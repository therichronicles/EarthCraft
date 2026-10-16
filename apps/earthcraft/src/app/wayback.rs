use super::App;
use earthcraft_core::math::now_unix;
use earthcraft_data::{
    tiles::Provider,
    wayback::{load_wayback_releases, set_wayback_releases},
};
use eframe::egui;
use std::sync::mpsc;
const UNIX_EPOCH_YEAR: i32 = 1970;
const GREGORIAN_YEAR_SECS: f64 = 31_556_952.0;
const DEFAULT_RELEASE_AGE_YEARS: i32 = 5;

impl App {
    pub(super) fn ensure_wayback(&mut self, ctx: &egui::Context) {
        if self.provider != Provider::Wayback || self.wayback_started {
            return;
        }
        self.wayback_started = true;
        self.wayback_err = None;
        let (tx, rx) = mpsc::channel();
        self.wayback_rx = Some(rx);
        let ctx = ctx.clone();
        self.runtime.spawn(async move {
            let _ = tx.send(load_wayback_releases().await);
            ctx.request_repaint();
        });
    }

    pub(super) fn poll_wayback(&mut self) {
        let Some(rx) = &self.wayback_rx else { return };
        match rx.try_recv() {
            Ok(Ok(list)) => {
                self.wayback_rx = None;
                set_wayback_releases(list.clone());
                // default: the latest release that is at least ~5 years old, so the difference is visible
                let year = UNIX_EPOCH_YEAR + (now_unix() / GREGORIAN_YEAR_SECS) as i32;
                let limit = format!("{}-12-31", year - DEFAULT_RELEASE_AGE_YEARS);
                self.wayback_idx = list.iter().position(|r| r.date <= limit).unwrap_or(0);
                self.wayback_list = list;
                self.last_state = 0;
                self.last_gen = u64::MAX;
            }
            Ok(Err(e)) => {
                self.wayback_rx = None;
                self.wayback_started = false;
                self.wayback_err = Some(e.to_string());
                self.provider = Provider::Satellite;
            }
            Err(mpsc::TryRecvError::Disconnected) => self.wayback_rx = None,
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    pub(super) fn set_wayback(&mut self, idx: usize) {
        if idx < self.wayback_list.len() && idx != self.wayback_idx {
            self.wayback_idx = idx;
            self.last_state = 0;
        }
    }

    pub(super) fn effective_provider(&self) -> Provider {
        if self.provider == Provider::Wayback && self.wayback_list.is_empty() {
            Provider::Satellite // until the list of dates is loaded
        } else {
            self.provider
        }
    }
}
