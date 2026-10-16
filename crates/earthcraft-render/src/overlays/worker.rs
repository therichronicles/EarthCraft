//! Bounded background geometry preparation; font metrics remain backend-owned.
use super::{
    BorderKind, BoundaryIndex, OverlayLayers, OverlayProjection, project_borders, project_labels,
};
use earthcraft_core::camera::Basis;
use earthcraft_data::boundaries::BoundaryData;
use emath::{Pos2, Rect};
use std::sync::{Arc, Condvar, Mutex};

#[derive(Clone, Copy, PartialEq)]
pub struct OverlayView {
    pub basis: Basis,
    pub rect: Rect,
    pub altitude_km: f64,
    pub grid_step_deg: Option<f64>,
    pub layers: OverlayLayers,
}
pub struct PreparedLabel {
    pub position: Pos2,
    pub source_index: usize,
}
pub struct OverlayGeometry {
    pub view: OverlayView,
    pub data: Arc<BoundaryData>,
    pub borders: Vec<(BorderKind, [Pos2; 2])>,
    pub labels: Vec<PreparedLabel>,
    pub grid_labels: Vec<super::grid::GridLabel>,
}
impl OverlayGeometry {
    pub fn matches(&self, view: OverlayView, data: &Arc<BoundaryData>) -> bool {
        self.view == view && Arc::ptr_eq(&self.data, data)
    }
}
pub struct OverlayInput {
    pub view: OverlayView,
    pub data: Arc<BoundaryData>,
}

#[derive(Default)]
pub struct OverlayPreparer {
    indexed: Option<(Arc<BoundaryData>, BoundaryIndex)>,
    latest: Option<Arc<OverlayGeometry>>,
}
impl OverlayPreparer {
    pub fn prepare(&mut self, request: OverlayInput) -> Arc<OverlayGeometry> {
        #[cfg(feature = "profiling")]
        let _profile = earthcraft_data::profiling::Span::new("overlay.prepare");
        if let Some(geometry) = self
            .latest
            .as_ref()
            .filter(|geometry| geometry.matches(request.view, &request.data))
        {
            return geometry.clone();
        }
        if self
            .indexed
            .as_ref()
            .is_none_or(|(data, _)| !Arc::ptr_eq(data, &request.data))
        {
            #[cfg(feature = "profiling")]
            let _profile = earthcraft_data::profiling::Span::new("overlay.index");
            self.indexed = Some((request.data.clone(), BoundaryIndex::new(&request.data)));
        }
        let index = &self.indexed.as_ref().unwrap().1;
        let visible = index.visible(&request.view.basis, request.view.rect);
        let projection = OverlayProjection {
            basis: &request.view.basis,
            rect: request.view.rect,
            altitude_km: request.view.altitude_km,
            layers: request.view.layers,
        };
        let mut borders = Vec::new();
        project_borders(
            &projection,
            &request.data,
            Some(&visible),
            |kind, segment| borders.push((kind, segment)),
        );
        let labels = project_labels(&projection, &request.data.labels)
            .into_iter()
            .map(|label| PreparedLabel {
                position: label.position,
                source_index: label.source_index,
            })
            .collect();
        let grid_labels = request.view.grid_step_deg.map_or_else(Vec::new, |step| {
            super::grid::project_grid_labels(&request.view.basis, request.view.rect, step)
        });
        let geometry = Arc::new(OverlayGeometry {
            view: request.view,
            data: request.data,
            borders,
            labels,
            grid_labels,
        });
        self.latest = Some(geometry.clone());
        geometry
    }
}

#[derive(Default)]
struct Mailbox {
    pending: Option<OverlayInput>,
    completed: Option<Arc<OverlayGeometry>>,
    stopped: bool,
}
#[derive(Default)]
struct Shared {
    mailbox: Mutex<Mailbox>,
    wake: Condvar,
}
pub struct OverlayWorker {
    shared: Arc<Shared>,
}

impl OverlayWorker {
    pub fn new(repaint: impl Fn() + Send + 'static) -> std::io::Result<Self> {
        let shared = Arc::new(Shared::default());
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("earthcraft-overlays".into())
            .spawn(move || {
                let mut preparer = OverlayPreparer::default();
                loop {
                    let request = {
                        let mut state = worker.mailbox.lock().unwrap();
                        while state.pending.is_none() && !state.stopped {
                            state = worker.wake.wait(state).unwrap();
                        }
                        if state.stopped {
                            return;
                        }
                        state.pending.take().unwrap()
                    };
                    let geometry = preparer.prepare(request);
                    let mut state = worker.mailbox.lock().unwrap();
                    if state.stopped {
                        return;
                    }
                    state.completed = Some(geometry);
                    drop(state);
                    repaint();
                }
            })?;
        Ok(Self { shared })
    }
    pub fn submit(&self, view: OverlayView, data: Arc<BoundaryData>) {
        self.shared.mailbox.lock().unwrap().pending = Some(OverlayInput { view, data });
        self.shared.wake.notify_one();
    }
    pub fn take_completed(&self) -> Option<Arc<OverlayGeometry>> {
        self.shared.mailbox.lock().unwrap().completed.take()
    }
}
impl Drop for OverlayWorker {
    fn drop(&mut self) {
        let mut state = self.shared.mailbox.lock().unwrap();
        state.stopped = true;
        state.pending = None;
        state.completed = None;
        drop(state);
        self.shared.wake.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthcraft_core::camera::Cam;
    use earthcraft_data::boundaries::{LabelKind, MapLabel};
    use std::{sync::mpsc, time::Duration};
    #[test]
    fn background_geometry_matches_direct_projection_and_dataset_identity() {
        let data = Arc::new(BoundaryData {
            countries: vec![Arc::new(vec![(0.0, 0.0), (1.0, 1.0)])],
            labels: vec![MapLabel {
                lat: 0.0,
                lon: 0.0,
                en: "Center".into(),
                pt: "Centro".into(),
                kind: LabelKind::Country,
                rank: 0,
                big: false,
            }],
            ..Default::default()
        });
        let view = OverlayView {
            basis: Cam {
                tilt: 0.0,
                lat: 0.0,
                lon: 0.0,
                heading: 0.0,
                alt: 1000.0,
            }
            .basis(1.0),
            rect: Rect::from_min_size(Pos2::ZERO, emath::vec2(600.0, 600.0)),
            altitude_km: 1000.0,
            grid_step_deg: None,
            layers: OverlayLayers {
                countries: true,
                states: true,
                cities: false,
                metro: false,
                labels: true,
            },
        };
        let (tx, rx) = mpsc::channel();
        let ui_thread = std::thread::current().id();
        let worker = OverlayWorker::new(move || {
            assert_ne!(std::thread::current().id(), ui_thread);
            let _ = tx.send(());
        })
        .unwrap();
        worker.submit(view, data.clone());
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let geometry = worker.take_completed().unwrap();
        assert!(geometry.matches(view, &data));
        assert!(!geometry.matches(view, &Arc::new((*data).clone())));
        let projection = OverlayProjection {
            basis: &view.basis,
            rect: view.rect,
            altitude_km: view.altitude_km,
            layers: view.layers,
        };
        let mut direct = Vec::new();
        project_borders(&projection, &data, None, |_, points| direct.push(points));
        assert_eq!(
            geometry
                .borders
                .iter()
                .map(|(_, points)| *points)
                .collect::<Vec<_>>(),
            direct
        );
        let labels = project_labels(&projection, &data.labels);
        assert_eq!(geometry.labels[0].position, labels[0].position);
        assert_eq!(geometry.labels[0].source_index, labels[0].source_index);

        let mut preparer = OverlayPreparer::default();
        let first = preparer.prepare(OverlayInput {
            view,
            data: data.clone(),
        });
        let same = preparer.prepare(OverlayInput {
            view,
            data: data.clone(),
        });
        assert!(
            Arc::ptr_eq(&first, &same),
            "tile-only changes reuse geometry"
        );
        let hidden = OverlayView {
            layers: OverlayLayers {
                labels: false,
                ..view.layers
            },
            ..view
        };
        let next = preparer.prepare(OverlayInput {
            view: hidden,
            data: data.clone(),
        });
        assert!(!Arc::ptr_eq(&first, &next));
        assert!(next.labels.is_empty());
        let numbered = OverlayView {
            grid_step_deg: Some(2.0),
            ..view
        };
        worker.submit(numbered, data.clone());
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let numbered_geometry = worker.take_completed().unwrap();
        assert!(!numbered_geometry.matches(view, &data));
        assert!(!numbered_geometry.grid_labels.is_empty());
        assert!(first.grid_labels.is_empty());
        assert!(!numbered_geometry.matches(
            OverlayView {
                grid_step_deg: Some(1.0),
                ..numbered
            },
            &data
        ));
        worker.submit(view, data.clone());
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(worker.take_completed().unwrap().grid_labels.is_empty());
    }

    #[test]
    fn pending_views_coalesce_while_completed_work_is_busy() {
        let (wake, wakes) = mpsc::channel();
        let (release, releases) = mpsc::channel();
        let worker = OverlayWorker::new(move || {
            let _ = wake.send(());
            let _ = releases.recv_timeout(Duration::from_secs(10));
        })
        .unwrap();
        let data = Arc::new(BoundaryData::default());
        let mut view = OverlayView {
            basis: Cam {
                tilt: 0.0,
                lat: 0.0,
                lon: 0.0,
                heading: 0.0,
                alt: 1000.0,
            }
            .basis(1.0),
            rect: Rect::from_min_size(Pos2::ZERO, emath::vec2(600.0, 600.0)),
            altitude_km: 1000.0,
            grid_step_deg: None,
            layers: OverlayLayers {
                countries: true,
                states: true,
                cities: false,
                metro: false,
                labels: true,
            },
        };
        worker.submit(view, data.clone());
        wakes.recv_timeout(Duration::from_secs(10)).unwrap();
        for altitude in 1..=100 {
            view.altitude_km = altitude as f64;
            worker.submit(view, data.clone());
        }
        release.send(()).unwrap();
        wakes.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(worker.take_completed().unwrap().matches(view, &data));
        release.send(()).unwrap();
    }
}
