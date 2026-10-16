//! Cloud/radar fetching, refresh policy and loaded imagery snapshots.
use earthcraft_core::language::Lang;
use image::RgbaImage;
mod download;
use download::{download_clouds, download_radar};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
const CLOUD_INTERVAL: Duration = Duration::from_secs(3 * 60 * 60);
const RADAR_INTERVAL: Duration = Duration::from_secs(10 * 60);
const RETRY_INTERVAL: Duration = Duration::from_secs(60);

struct RadarFrame {
    image: RgbaImage,
    time: i64,
}

#[derive(Default)]
pub struct LiveMaps {
    cloud: Option<RgbaImage>,
    radar: Option<RgbaImage>,
    cloud_rx: Option<mpsc::Receiver<Result<RgbaImage, crate::Error>>>,
    radar_rx: Option<mpsc::Receiver<Result<RadarFrame, crate::Error>>>,
    cloud_attempt: Option<Instant>,
    radar_attempt: Option<Instant>,
    cloud_error: Option<String>,
    radar_error: Option<String>,
    radar_frame_time: Option<i64>,
    generation: u64,
}
impl LiveMaps {
    pub fn update(
        &mut self,
        runtime: &crate::runtime::ServiceRuntime,
        clouds: bool,
        radar: bool,
        repaint: impl Fn() + Send + Sync + 'static,
    ) {
        let repaint = std::sync::Arc::new(repaint);
        if let Some(rx) = &self.cloud_rx {
            match rx.try_recv() {
                Ok(Ok(image)) => {
                    self.cloud = Some(image);
                    self.cloud_error = None;
                    self.cloud_rx = None;
                    self.generation += 1;
                    repaint();
                }
                Ok(Err(err)) => {
                    self.cloud_error = Some(err.to_string());
                    self.cloud_rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.cloud_error = Some("Cloud download interrupted".into());
                    self.cloud_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(rx) = &self.radar_rx {
            match rx.try_recv() {
                Ok(Ok(frame)) => {
                    self.radar_frame_time = Some(frame.time);
                    self.radar = Some(frame.image);
                    self.radar_error = None;
                    self.radar_rx = None;
                    self.generation += 1;
                    repaint();
                }
                Ok(Err(err)) => {
                    self.radar_error = Some(err.to_string());
                    self.radar_rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.radar_error = Some("Radar download interrupted".into());
                    self.radar_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if clouds
            && self.cloud_rx.is_none()
            && refresh_due(
                self.cloud_attempt,
                self.cloud.is_some() && self.cloud_error.is_none(),
                CLOUD_INTERVAL,
            )
        {
            let (tx, rx) = mpsc::channel();
            self.cloud_rx = Some(rx);
            self.cloud_attempt = Some(Instant::now());
            let repaint = repaint.clone();
            runtime.spawn(async move {
                let result = download_clouds().await;
                let _ = tx.send(result);
                repaint();
            });
        }
        if radar
            && self.radar_rx.is_none()
            && refresh_due(
                self.radar_attempt,
                self.radar.is_some() && self.radar_error.is_none(),
                RADAR_INTERVAL,
            )
        {
            let (tx, rx) = mpsc::channel();
            self.radar_rx = Some(rx);
            self.radar_attempt = Some(Instant::now());
            let repaint = repaint.clone();
            runtime.spawn(async move {
                let result = download_radar().await;
                let _ = tx.send(result);
                repaint();
            });
        }
    }

    pub fn cloud(&self) -> Option<&RgbaImage> {
        self.cloud.as_ref()
    }
    pub fn radar(&self) -> Option<&RgbaImage> {
        self.radar.as_ref()
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn status(&self, clouds: bool, radar: bool, lang: Lang) -> String {
        let mut lines = Vec::new();
        if clouds {
            if let Some(err) = &self.cloud_error {
                lines.push(
                    Lang::En.format(
                        "live-clouds-error",
                        &[
                            (
                                "cached",
                                if self.cloud.is_some() {
                                    "true"
                                } else {
                                    "false"
                                }
                                .into(),
                            ),
                            ("error", err.as_str().into()),
                        ],
                    ),
                );
            } else if self.cloud.is_some() {
                lines.push(lang.text("live-clouds-ok").to_owned());
            } else {
                lines.push(lang.text("live-clouds-loading").to_owned());
            }
        }
        if radar {
            if let Some(err) = &self.radar_error {
                lines.push(
                    Lang::En.format(
                        "live-radar-error",
                        &[
                            (
                                "cached",
                                if self.radar.is_some() {
                                    "true"
                                } else {
                                    "false"
                                }
                                .into(),
                            ),
                            ("error", err.as_str().into()),
                        ],
                    ),
                );
            } else if self.radar.is_some() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                let age = (now - self.radar_frame_time.unwrap_or(now)).max(0) / 60;
                lines.push(lang.format("live-radar-age", &[("minutes", age.to_string().into())]));
            } else {
                lines.push(lang.text("live-radar-loading").to_owned());
            }
        }
        lines.join("\n")
    }
}
fn refresh_due(last: Option<Instant>, loaded: bool, interval: Duration) -> bool {
    last.is_none_or(|t| t.elapsed() >= if loaded { interval } else { RETRY_INTERVAL })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn error_statuses_are_whole_english_messages_in_both_ui_languages() {
        let maps = LiveMaps {
            cloud_error: Some("cloud network failure".into()),
            radar_error: Some("radar network failure".into()),
            cloud: Some(RgbaImage::new(1, 1)),
            ..Default::default()
        };
        let status = maps.status(true, true, Lang::En);
        assert_eq!(maps.status(true, true, Lang::PtBr), status);
        assert!(
            status.contains("cloud network failure") && status.contains("radar network failure")
        );
        assert!(status.contains("showing cached image"));
        assert!(maps.status(false, false, Lang::PtBr).is_empty());
    }
}
