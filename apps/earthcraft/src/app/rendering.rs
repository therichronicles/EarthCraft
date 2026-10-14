use super::App;
use earthcraft_core::{
    camera::{Basis, Cam},
    math::{ll_to_v3, now_unix, subsolar},
};
use earthcraft_data::tiles::Provider;
use earthcraft_render::RenderCfg;
use earthcraft_render::worker::{RenderRequest, RenderWorker};
use eframe::egui;
use std::sync::Arc;

struct PreparedFrame {
    image: egui::ColorImage,
    overlays: Option<Arc<earthcraft_render::overlays::OverlayGeometry>>,
    aircraft: Option<Arc<earthcraft_render::aircraft::AircraftGeometry>>,
}

pub(super) struct Renderer {
    worker: RenderWorker<PreparedFrame>,
    displayed: Option<(Cam, Basis)>,
    displayed_rect: Option<egui::Rect>,
    displayed_grid_step: Option<f64>,
    waiting: Vec<earthcraft_data::tiles::TileKey>,
    required: Vec<earthcraft_data::tiles::TileKey>,
    retry_at: Option<std::time::Instant>,
}

impl Renderer {
    pub(super) fn new(cache: Arc<earthcraft_data::tiles::TileCache>, ctx: egui::Context) -> Self {
        let admission_cache = cache.clone();
        let mut backend = earthcraft_render::backend::FrameRenderer::new(
            earthcraft_render::backend::BackendPreference::from_environment(),
        );
        let mut overlay_preparer = earthcraft_render::overlays::OverlayPreparer::default();
        let mut aircraft_preparer = earthcraft_render::aircraft::AircraftPreparer::default();
        Self {
            worker: RenderWorker::with_prepare(
                move |request, buffer| {
                    #[cfg(feature = "profiling")]
                    let mut profile = earthcraft_data::profiling::Span::new("frame.raster");
                    #[cfg(feature = "profiling")]
                    {
                        profile.values.width = request.width;
                        profile.values.height = request.height;
                        profile.values.inflight = cache.inflight();
                    }
                    backend.render(request, &cache, buffer)
                },
                move || ctx.request_repaint(),
                move |frame| {
                    admission_cache.prioritize_view(&frame.required_tiles());
                },
                move |request, pixels| {
                    #[cfg(feature = "profiling")]
                    let conversion = earthcraft_data::profiling::Span::new("frame.convert");
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [request.width, request.height],
                        pixels,
                    );
                    #[cfg(feature = "profiling")]
                    drop(conversion);
                    let geometry = request.overlays.as_ref().map(|input| {
                        overlay_preparer.prepare(earthcraft_render::overlays::OverlayInput {
                            view: input.view,
                            data: input.data.clone(),
                        })
                    });
                    PreparedFrame {
                        image,
                        overlays: geometry,
                        aircraft: request.aircraft.clone().map(|mut input| {
                            input.unix_secs = now_unix();
                            aircraft_preparer.prepare(input)
                        }),
                    }
                },
            )
            .expect("start frame renderer"),
            displayed: None,
            displayed_rect: None,
            displayed_grid_step: None,
            waiting: Vec::new(),
            required: Vec::new(),
            retry_at: None,
        }
    }
    pub(super) fn tiles_changed(&self, cache: &earthcraft_data::tiles::TileCache) -> bool {
        self.waiting
            .iter()
            .any(|&key| matches!(cache.lookup(key), earthcraft_data::tiles::Lookup::Ready(_)))
    }
    pub(super) fn refill_requests(
        &mut self,
        cache: &earthcraft_data::tiles::TileCache,
        changed: bool,
        ctx: &egui::Context,
    ) {
        let now = std::time::Instant::now();
        if changed || self.retry_at.is_some_and(|deadline| now >= deadline) {
            cache.prioritize_view(&self.required);
            self.retry_at = cache.retry_delay(&self.required).map(|delay| now + delay);
        }
        if let Some(deadline) = self.retry_at {
            ctx.request_repaint_after(deadline.saturating_duration_since(now));
        }
    }
    pub(super) fn is_idle(&self) -> bool {
        self.worker.is_idle()
    }
    pub(super) fn picking_basis(&self, fallback: Basis) -> Basis {
        displayed_basis(self.displayed, fallback)
    }
    pub(super) fn displayed_grid_step(&self) -> Option<f64> {
        self.displayed_grid_step
    }
    pub(super) fn displayed_rect(&self) -> Option<egui::Rect> {
        self.displayed_rect
    }
    pub(super) fn displayed_view(&self) -> Option<(Cam, Basis)> {
        self.displayed
    }
}

impl App {
    pub(super) fn request_render(
        &mut self,
        w: usize,
        h: usize,
        aspect: f64,
        presentation: u64,
        rect: egui::Rect,
        pixels_per_point: f32,
    ) {
        let basis = self.cam.basis_for_model(aspect, self.active_earth_model());
        let grid_step = earthcraft_render::grid_step(self.cam.alt, w, h);
        let sun = if self.show_sun {
            let (la, lo) = subsolar(now_unix());
            Some(ll_to_v3(la, lo))
        } else {
            None
        };
        let prov = self.effective_provider();
        let rel = if prov == Provider::Wayback {
            self.wayback_idx as u16
        } else {
            0
        };
        let cfg = RenderCfg {
            provider: prov,
            rel,
            grid: self.show_grid,
            grid_step,
            atmo: self.show_atmo,
            sun,
        };
        self.renderer.worker.submit(RenderRequest {
            width: w,
            height: h,
            cam: self.cam,
            basis,
            cfg,
            presentation,
            aircraft: (self.show_flights
                && self.explorer.active == earthcraft_core::body::Body::Earth)
                .then(|| self.aircraft_input(basis, rect, self.cam.alt, pixels_per_point)),
            overlays: Some(earthcraft_render::overlays::OverlayInput {
                view: earthcraft_render::overlays::OverlayView {
                    basis,
                    rect,
                    altitude_km: self.cam.alt,
                    grid_step_deg: (self.show_grid && self.settings.grid_numbers)
                        .then_some(grid_step),
                    layers: self.overlay_layers(),
                },
                data: self.boundaries.clone(),
            }),
        });
    }

    pub(super) fn poll_render(&mut self, presentation: u64, ctx: &egui::Context) {
        let Some(frame) = self.renderer.worker.take_completed(presentation) else {
            return;
        };
        #[cfg(feature = "profiling")]
        earthcraft_data::profiling::Span::elapsed(
            "frame.delivery",
            frame.submitted,
            earthcraft_data::profiling::Values {
                width: frame.request.width,
                height: frame.request.height,
                ..Default::default()
            },
        );
        #[cfg(feature = "profiling")]
        let _upload = earthcraft_data::profiling::Span::new("ui.texture_submit");
        let (w, h, buf) = (frame.request.width, frame.request.height, frame.pixels);

        let PreparedFrame {
            image: img,
            overlays,
            aircraft,
        } = frame.prepared;
        if let Some(geometry) = aircraft
            && Arc::ptr_eq(&geometry.data, &self.live_aircraft.snapshot())
            && geometry.view.map_scale == self.settings.map_scale
            && geometry.view.pixels_per_point == ctx.pixels_per_point()
        {
            self.aircraft_overlay.accept(geometry, now_unix());
        }
        if let Some(geometry) = overlays {
            self.overlay_cache.accept(geometry);
        }
        match &mut self.texture {
            Some(t) if t.size() == [w, h] => {
                t.set_partial([0, 0], img, egui::TextureOptions::LINEAR)
            }
            Some(t) => t.set(img, egui::TextureOptions::LINEAR),
            None => {
                self.texture = Some(ctx.load_texture("globe", img, egui::TextureOptions::LINEAR))
            }
        }
        if let Some((_, _, previous)) = self.image.replace((w, h, Arc::new(buf)))
            && let Ok(previous) = Arc::try_unwrap(previous)
        {
            self.renderer.worker.recycle(previous);
        }
        self.renderer.waiting = frame.waiting;
        self.renderer.required = frame.required;
        self.renderer.retry_at = None;
        // Recheck dependencies even if a tile finished before frame publication.
        self.last_gen = u64::MAX;
        self.renderer.displayed_rect = frame.request.overlays.as_ref().map(|input| input.view.rect);
        self.renderer.displayed_grid_step = frame
            .request
            .cfg
            .grid
            .then_some(frame.request.cfg.grid_step);
        self.renderer.displayed = Some((frame.request.cam, frame.request.basis));
    }
}

// Input must describe the same globe as the frame being presented.
fn displayed_basis(displayed: Option<(Cam, Basis)>, fallback: Basis) -> Basis {
    displayed.map_or(fallback, |(_, basis)| basis)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn picking_tracks_displayed_camera_while_new_view_is_pending() {
        let shown = Cam {
            lat: 20.0,
            lon: 30.0,
            heading: 0.0,
            alt: 14000.0,
        };
        let requested = Cam { lon: 90.0, ..shown };
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let shown_basis = shown.basis(4.0 / 3.0);
        let requested_basis =
            requested.basis_for_model(4.0 / 3.0, earthcraft_core::earth_model::EarthModel::Wgs84);
        assert!(displayed_basis(Some((shown, shown_basis)), requested_basis) == shown_basis);
        let pick = displayed_basis(Some((shown, shown_basis)), requested_basis)
            .pick(rect, rect.center())
            .unwrap();
        assert!((pick.0 - shown.lat).abs() < 1e-8);
        assert!((pick.1 - shown.lon).abs() < 1e-8);
        assert_eq!(
            displayed_basis(None, requested_basis).pick(rect, rect.center()),
            requested_basis.pick(rect, rect.center())
        );
    }
}
