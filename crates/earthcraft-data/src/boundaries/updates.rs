//! Ordered patch preparation; the UI swaps one latest immutable dataset.
use super::{BoundaryData, Layer, MapLabel, Patch, dedupe_city_labels};
use std::sync::{Arc, Mutex, mpsc};

pub struct BoundarySnapshot {
    pub data: Arc<BoundaryData>,
    pub names: Arc<Vec<MapLabel>>,
}

pub struct BoundaryUpdates {
    latest: Arc<Mutex<Option<BoundarySnapshot>>>,
}

fn apply(data: &mut BoundaryData, patch: Patch) {
    match patch.layer {
        Layer::Countries => data.countries.extend(patch.lines),
        Layer::States => {
            data.states.extend(patch.lines);
            data.states_big.extend(patch.lines_big);
        }
        Layer::Cities => data.cities.extend(patch.lines),
        Layer::WorldCities => data.world_cities = patch.lines,
        Layer::Metro => data.metro.extend(patch.lines),
    }
    data.labels.extend(patch.labels);
    dedupe_city_labels(&mut data.labels);
}

impl BoundaryUpdates {
    pub fn new(
        patches: mpsc::Receiver<Patch>,
        repaint: impl Fn() + Send + 'static,
    ) -> std::io::Result<Self> {
        let latest = Arc::new(Mutex::new(None));
        let output = Arc::downgrade(&latest);
        std::thread::Builder::new()
            .name("earthcraft-boundaries".into())
            .spawn(move || {
                let mut data = Arc::new(BoundaryData::default());
                while let Ok(patch) = patches.recv() {
                    if output.strong_count() == 0 {
                        return;
                    }
                    apply(Arc::make_mut(&mut data), patch);
                    // Drain already completed patches in order, then publish one snapshot.
                    for patch in patches.try_iter() {
                        apply(Arc::make_mut(&mut data), patch);
                    }
                    let snapshot = BoundarySnapshot {
                        data: data.clone(),
                        names: Arc::new(data.labels.clone()),
                    };
                    let Some(output) = output.upgrade() else {
                        return;
                    };
                    let previous = output.lock().unwrap().replace(snapshot);
                    drop(previous);
                    repaint();
                }
            })?;
        Ok(Self { latest })
    }

    pub fn take_latest(&self) -> Option<BoundarySnapshot> {
        self.latest.lock().unwrap().take()
    }
}

#[cfg(test)]
mod tests {
    use super::super::LabelKind;
    use super::*;
    use std::time::Duration;

    #[test]
    fn patch_preparation_preserves_order_deduplication_and_previous_snapshots() {
        let (tx, rx) = mpsc::channel();
        let (wake, wakes) = mpsc::channel();
        let updates = BoundaryUpdates::new(rx, move || {
            let _ = wake.send(());
        })
        .unwrap();
        let patch = |name: &str, rank| Patch {
            layer: Layer::Countries,
            lines: vec![Arc::new(vec![(0.0, 0.0), (1.0, 1.0)])],
            lines_big: vec![],
            labels: vec![MapLabel {
                lat: 0.0,
                lon: 0.0,
                en: name.into(),
                pt: "São Paulo".into(),
                kind: LabelKind::City,
                rank,
                big: false,
            }],
        };
        tx.send(patch("First", 5)).unwrap();
        wakes.recv_timeout(Duration::from_secs(10)).unwrap();
        let first = updates.take_latest().unwrap();
        tx.send(patch("Second", 1)).unwrap();
        wakes.recv_timeout(Duration::from_secs(10)).unwrap();
        let second = updates.take_latest().unwrap();
        assert_eq!(first.data.labels[0].en, "First");
        assert_eq!(second.data.labels.len(), 1);
        assert_eq!(second.names[0].en, "Second");
        assert_eq!(second.data.countries.len(), 2);
        assert!(Arc::ptr_eq(
            &first.data.countries[0],
            &second.data.countries[0]
        ));
    }
}
