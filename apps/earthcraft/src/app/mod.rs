//! Desktop state and orchestration; widgets live in focused child modules.
#[cfg(test)]
mod test_ui;
use earthcraft_core::{camera::Cam, language::Lang, placemark::Places};
use earthcraft_data::{
    boundaries::{BoundaryData, BoundaryUpdates, StatusLog, load_boundaries_async},
    search::{GeoResult, Suggestion},
    settings::{Settings, load_lang},
    tiles::{Provider, TileCache, TileWarmup},
    wayback::WaybackRelease,
};
use eframe::egui::{self, Rect};
use std::{
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};
mod cache;
mod explorer;
mod files;
mod flat_earth;
mod frame_timing;
mod globe;
mod live_layers;
mod live_maps;
mod menu;
mod navigation;
mod overlays;
mod rendering;
mod roads;
mod ruler_history;
mod search;
mod traffic;
mod ui;
mod wayback;
const INITIAL_CAMERA: Cam = Cam {
    lat: -15.0,
    lon: -50.0,
    heading: 0.0,
    alt: 14_000.0,
};
const PAN_SPEED_POINTS_PER_SECOND: f64 = 350.0;
const BOUNDARY_POLL_INTERVAL: Duration = Duration::from_millis(300);

type SuggestionResponse = (String, Result<Vec<Suggestion>, earthcraft_data::Error>);

pub(crate) struct App {
    #[cfg(target_os = "macos")]
    native_menu: Option<menu::NativeMenu>,
    runtime: earthcraft_data::runtime::ServiceRuntime,
    explorer: explorer::Explorer,
    flat_earth: flat_earth::FlatEarth,
    boundary_loader: earthcraft_data::boundaries::BoundaryLoader,
    lang: Lang,
    cam: Cam,
    fly: Option<earthcraft_core::flight::Flight>,
    cache: Arc<TileCache>,
    cache_controls: cache::CacheControls,
    tile_warmup: TileWarmup,
    provider: Provider,
    show_grid: bool,
    show_country_boundaries: bool,
    show_state_boundaries: bool,
    show_city_boundaries: bool,
    show_metro_boundaries: bool,
    show_place_labels: bool,
    boundaries: Arc<BoundaryData>,
    overlay_cache: overlays::OverlayCache,
    boundaries_rx: BoundaryUpdates,
    boundary_log: StatusLog,
    show_atmo: bool,
    show_sun: bool,
    auto_rotate: bool,
    show_flights: bool,
    show_vessels: bool,
    show_satellites: bool,
    show_traffic: bool,
    show_roads: bool,
    tomtom_key: String,
    live_vessels: earthcraft_data::vessels::LiveVessels,
    live_satellites: earthcraft_data::satellites::LiveSatellites,
    traffic: traffic::TrafficOverlay,
    roads: roads::RoadOverlay,
    show_weather: bool,
    show_clouds: bool,
    live_maps: live_maps::LiveMaps,
    live_aircraft: earthcraft_data::aircraft::LiveAircraft,
    aircraft_overlay: live_layers::AircraftOverlay,

    renderer: rendering::Renderer,
    scale_bar_cache: earthcraft_render::scale_bar::ScaleBarCache,
    texture: Option<egui::TextureHandle>,
    image: Option<(usize, usize, Arc<Vec<u8>>)>,
    last_state: u64,
    last_gen: u64,
    last_render: Instant,
    last_interact: Instant,

    local_search: earthcraft_data::search::LocalSearch,
    query: String,
    results: Vec<GeoResult>,
    search_rx: Option<mpsc::Receiver<Result<Vec<GeoResult>, earthcraft_data::Error>>>,
    status: String,
    status_file: Option<files::FileStatus>,
    system_units: earthcraft_core::units::DistanceUnits,
    us_locale: bool,
    altitude_editor: Option<ui::altitude::AltitudeEditor>,

    file_jobs: earthcraft_data::files::FileJobs,
    file_results: mpsc::Receiver<earthcraft_data::files::FileCompletion>,
    placemarks: Places,
    place_selection: Option<u64>,
    new_name: String,
    kml_path: String,
    kml_session: files::KmlSession,
    menu_pick: Option<(f64, f64)>,

    ruler: bool,
    ruler_drag: Option<earthcraft_core::ruler::RulerPointId>,
    ruler_paths: earthcraft_core::ruler::Ruler,
    ruler_point_editor: Option<ui::ruler::PointEditor>,
    cursor: Option<(f64, f64)>,
    sidebar_open: bool,
    sidebar_animating: bool,
    show_help: bool,

    settings: Settings,
    saved_settings: Settings,
    settings_writer: earthcraft_data::settings::SettingsWriter,
    applied_ui_scale: f32,
    ui_scale_edit: f32, // slider value while dragging (committed on release)
    show_settings: bool,

    wayback_list: Vec<WaybackRelease>,
    wayback_idx: usize,
    wayback_rx: Option<mpsc::Receiver<Result<Vec<WaybackRelease>, earthcraft_data::Error>>>,
    wayback_started: bool,
    wayback_err: Option<String>,

    suggestions: Vec<Suggestion>,
    remote_sugg: Vec<Suggestion>,
    sugg_open: bool,
    sugg_sel: Option<usize>,
    sugg_last_edit: Instant,
    sugg_sent: String,
    sugg_rx: Option<mpsc::Receiver<SuggestionResponse>>,

    kml_drop_rect: Rect,
    ring_drag: bool,
}

impl App {
    fn distance_units(&self) -> earthcraft_core::units::DistanceUnits {
        self.settings.distance_units.resolve(self.system_units)
    }

    pub(crate) fn new(cc: &eframe::CreationContext<'_>, settings: Settings) -> Self {
        let mut vis = egui::Visuals::dark();
        vis.handle_shape = egui::style::HandleShape::Circle;
        vis.window_corner_radius = 6.0.into();
        cc.egui_ctx.set_visuals(vis);
        let runtime =
            earthcraft_data::runtime::ServiceRuntime::new().expect("start network runtime");
        let lang = load_lang();
        let system_locale = sys_locale::get_locale().unwrap_or_default();
        let boundary_log: StatusLog = Arc::new(Mutex::new(Vec::new()));
        let (boundaries_rx, boundary_loader) =
            load_boundaries_async(boundary_log.clone(), runtime.handle(), {
                let ctx = cc.egui_ctx.clone();
                move || ctx.request_repaint()
            });
        let cache = TileCache::new(
            {
                let ctx = cc.egui_ctx.clone();
                move || ctx.request_repaint()
            },
            runtime.handle(),
        );
        let renderer = rendering::Renderer::new(cache.clone(), cc.egui_ctx.clone());
        let (file_jobs, file_results) = earthcraft_data::files::FileJobs::new(runtime.handle(), {
            let ctx = cc.egui_ctx.clone();
            move || ctx.request_repaint()
        });
        let local_search = earthcraft_data::search::LocalSearch::new(runtime.handle(), {
            let ctx = cc.egui_ctx.clone();
            move || ctx.request_repaint()
        });
        let mut app = App {
            #[cfg(target_os = "macos")]
            native_menu: None,
            explorer: explorer::Explorer::default(),
            flat_earth: flat_earth::FlatEarth::default(),
            lang,
            cam: INITIAL_CAMERA,
            fly: None,
            cache,
            renderer,
            scale_bar_cache: earthcraft_render::scale_bar::ScaleBarCache::default(),
            provider: Provider::Satellite,
            tile_warmup: TileWarmup::startup_satellite(),
            cache_controls: cache::CacheControls::default(),
            show_grid: false,
            show_country_boundaries: true,
            show_state_boundaries: true,
            show_city_boundaries: false,
            show_metro_boundaries: false,
            show_place_labels: true,
            boundaries: Arc::new(BoundaryData::default()),
            overlay_cache: overlays::OverlayCache::new(cc.egui_ctx.clone()),
            boundaries_rx: BoundaryUpdates::new(boundaries_rx, {
                let ctx = cc.egui_ctx.clone();
                move || ctx.request_repaint()
            })
            .expect("start boundary preparation"),
            boundary_loader,
            boundary_log,
            show_atmo: true,
            show_sun: false,
            auto_rotate: false,
            show_flights: false,
            show_vessels: false,
            show_satellites: false,
            show_traffic: false,
            show_roads: false,
            tomtom_key: std::env::var("EARTHCRAFT_TOMTOM_KEY").unwrap_or_default(),
            live_vessels: earthcraft_data::vessels::LiveVessels::default(),
            live_satellites: earthcraft_data::satellites::LiveSatellites::default(),
            traffic: traffic::TrafficOverlay::default(),
            roads: roads::RoadOverlay::default(),
            show_weather: false,
            show_clouds: false,
            live_maps: live_maps::LiveMaps::default(),
            live_aircraft: earthcraft_data::aircraft::LiveAircraft::new(runtime.handle(), {
                let ctx = cc.egui_ctx.clone();
                move || ctx.request_repaint()
            }),
            aircraft_overlay: live_layers::AircraftOverlay::new(cc.egui_ctx.clone()),
            texture: None,
            image: None,
            last_state: 0,
            last_gen: u64::MAX,
            last_render: Instant::now(),
            last_interact: Instant::now(),
            local_search,
            query: String::new(),
            results: vec![],
            search_rx: None,
            status: lang.text("app-ready-status").into(),
            status_file: None,
            system_units: earthcraft_core::units::DistanceUnits::for_locale(&system_locale),
            us_locale: earthcraft_core::units::is_us_locale(&system_locale),
            altitude_editor: None,
            placemarks: Places::default(),
            place_selection: None,
            file_jobs,
            file_results,
            new_name: String::new(),
            kml_path: "placemarks.kml".into(),
            kml_session: files::KmlSession::default(),
            menu_pick: None,
            ruler: false,
            ruler_drag: None,
            ruler_paths: earthcraft_core::ruler::Ruler::default(),
            ruler_point_editor: None,
            cursor: None,
            sidebar_open: true,
            sidebar_animating: false,
            show_help: false,
            ui_scale_edit: settings.ui_scale,
            settings: settings.clone(),
            saved_settings: settings,
            settings_writer: earthcraft_data::settings::SettingsWriter::new({
                let ctx = cc.egui_ctx.clone();
                move || ctx.request_repaint()
            })
            .expect("start settings writer"),
            applied_ui_scale: 0.0,
            show_settings: false,
            wayback_list: vec![],
            wayback_idx: 0,
            wayback_rx: None,
            wayback_started: false,
            wayback_err: None,
            suggestions: vec![],
            remote_sugg: vec![],
            sugg_open: false,
            sugg_sel: None,
            sugg_last_edit: Instant::now(),
            sugg_sent: String::new(),
            sugg_rx: None,
            kml_drop_rect: Rect::NOTHING,
            ring_drag: false,
            runtime,
        };
        app.restore_kml_session();
        #[cfg(target_os = "macos")]
        {
            app.with_native_menu(cc)
        }
        #[cfg(not(target_os = "macos"))]
        {
            app
        }
    }
}
impl eframe::App for App {
    #[cfg(feature = "profiling")]
    fn on_exit(&mut self) {
        earthcraft_data::profiling::flush();
    }
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let aircraft_enabled = self.show_flights
            && self.explorer.active == earthcraft_core::body::Body::Earth
            && !self.flat_earth.open;
        self.live_aircraft
            .update(earthcraft_data::aircraft::AircraftQuery {
                enabled: aircraft_enabled,
                latitude: self.cam.lat,
                longitude: self.cam.lon,
                altitude_km: self.cam.alt,
            });
        if aircraft_enabled {
            ctx.request_repaint_after(live_layers::FLIGHT_REPAINT_INTERVAL);
        }
        // Independent realtime layers, loaded only for the globe while enabled.
        let earth =
            self.explorer.active == earthcraft_core::body::Body::Earth && !self.flat_earth.open;
        let vessel_wake = ctx.clone();
        self.live_vessels
            .update(&self.runtime, earth && self.show_vessels, move || {
                vessel_wake.request_repaint()
            });
        let satellite_wake = ctx.clone();
        self.live_satellites
            .update(&self.runtime, earth && self.show_satellites, move || {
                satellite_wake.request_repaint()
            });
        let road_wake = ctx.clone();
        self.roads.service.update(
            &self.runtime,
            earth && self.show_roads,
            self.cam.lat,
            self.cam.lon,
            self.cam.alt,
            move || road_wake.request_repaint(),
        );
        let traffic_wake = ctx.clone();
        self.traffic.service.update(
            &self.runtime,
            earthcraft_data::traffic::TrafficQuery {
                enabled: earth && self.show_traffic,
                key: &self.tomtom_key,
                latitude: self.cam.lat,
                longitude: self.cam.lon,
                altitude_km: self.cam.alt,
            },
            move || traffic_wake.request_repaint(),
        );
        if earth
            && (self.show_vessels || self.show_satellites || self.show_roads || self.show_traffic)
        {
            ctx.request_repaint_after(Duration::from_secs(30));
        }
        self.poll_files(ctx);
        self.ensure_wayback(ctx);
        self.poll_wayback();
        self.poll_suggestions(ctx);
        if let Some(error) = self.local_search.take_error() {
            self.status = format!("Local prefix index failed: {error}");
        }
        self.poll_search();
        self.poll_cache_clear(ctx);
        if let Some(snapshot) = self.boundaries_rx.take_latest() {
            self.merge_snapshot(snapshot);
        }
        if self.boundary_loader.is_loading() {
            ctx.request_repaint_after(BOUNDARY_POLL_INTERVAL);
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(feature = "profiling")]
        let mut profile = earthcraft_data::profiling::Span::new("ui.update");
        let dt = frame_timing::animation_delta_secs(ui);
        if (self.settings.ui_scale - self.applied_ui_scale).abs() > 1e-4 {
            ui::apply_ui_scale(ui, self.settings.ui_scale);
            self.applied_ui_scale = self.settings.ui_scale;
        }
        // persist settings once a slider is released
        if self.settings != self.saved_settings && !ui.input(|i| i.pointer.any_down()) {
            self.settings_writer.settings(&self.settings);
            self.saved_settings = self.settings.clone();
        }
        if let Some(error) = self.settings_writer.take_error() {
            self.status = format!("Settings persistence failed: {error}");
        }
        #[cfg(target_os = "macos")]
        self.poll_native_menu(ui);
        #[cfg(not(target_os = "macos"))]
        self.file_shortcuts(ui);
        self.ruler_shortcuts(ui);
        self.flat_earth_shortcuts(ui.ctx());
        self.handle_dropped_files(ui);
        self.step_fly(dt);
        if self.fly.is_some() {
            ui.request_repaint();
        }
        self.top_bar(ui);
        self.bottom_bar(ui);
        self.side_panel(ui);
        self.settings_window(ui);
        self.explorer_window(ui);
        self.altitude_window(ui);
        self.ruler_points_window(ui);
        self.live_maps
            .update(ui, &self.runtime, self.show_clouds, self.show_weather);
        self.boundary_loader.request_world_cities(
            self.show_city_boundaries
                && self.explorer.active == earthcraft_core::body::Body::Earth
                && !self.flat_earth.open,
            self.cam.alt,
            self.cam.lat,
            self.cam.lon,
        );
        self.boundary_loader.request_visible_layers(
            self.show_city_boundaries,
            self.show_metro_boundaries,
            self.show_place_labels,
            self.cam.alt,
        );
        if self.boundary_loader.is_loading() {
            ui.request_repaint_after(BOUNDARY_POLL_INTERVAL);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                if self.flat_earth.open {
                    let ctx = ui.ctx().clone();
                    self.flat_earth_view(ui, &ctx);
                } else {
                    self.globe_view(ui);
                }
            });
        #[cfg(target_os = "macos")]
        self.refresh_native_menu(ui);
        #[cfg(feature = "profiling")]
        {
            profile.values.inflight = self.cache.inflight();
            profile.values.resident_bytes = self.cache.resident_bytes();
        }
    }
}
