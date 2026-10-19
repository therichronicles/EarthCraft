//! Nonexclusive HID reads off the window thread, with bounded button edges and
//! coalesced stick samples. Only the Nintendo Pro Controller model is opened.
use super::protocol::{Decoder, Sample, calibration_requests};
use eframe::egui;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

pub(super) const VENDOR_ID: u16 = 0x057e;
pub(super) const PRODUCT_ID: u16 = 0x2009;
const RETRY_INTERVAL: Duration = Duration::from_secs(1);
const READ_TIMEOUT_MS: i32 = 100;
const REPORT_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_BUTTON_EDGES: usize = 128;
const REPORT_BYTES: usize = 128;

#[derive(Default)]
pub(super) struct Mailbox {
    pub sample: Option<Sample>,
    pub edges: Vec<(usize, bool)>,
    pub reset: bool,
    pub error: Option<String>,
}
impl Mailbox {
    fn publish(&mut self, sample: Sample) -> bool {
        let previous = self.sample.unwrap_or_default();
        let changed = self.sample != Some(sample);
        for (input, down) in sample.buttons.into_iter().enumerate() {
            if previous.buttons[input] != down && !self.reset {
                if self.edges.len() == MAX_BUTTON_EDGES {
                    self.reset = true;
                    self.edges.clear();
                    self.error = Some(
                        "Nintendo controller input overflow; release controls to resume".into(),
                    );
                    break;
                }
                self.edges.push((input, down));
            }
        }
        self.sample = Some(sample);
        changed
    }

    fn disconnect(&mut self, error: Option<String>) {
        self.sample = None;
        self.edges.clear();
        self.reset = true;
        self.error = error;
    }
}

pub(super) struct Reader {
    mailbox: Arc<Mutex<Mailbox>>,
    stop: Arc<AtomicBool>,
    thread: thread::Thread,
}
impl Reader {
    pub fn new(ctx: &egui::Context) -> Result<Self, String> {
        let mailbox = Arc::new(Mutex::new(Mailbox::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_mailbox = mailbox.clone();
        let worker_stop = stop.clone();
        let ctx = ctx.clone();
        let worker = thread::Builder::new()
            .name("nintendo-input".into())
            .spawn(move || {
                run(&worker_mailbox, &worker_stop, &ctx);
            })
            .map_err(|error| format!("Nintendo controller reader startup failed: {error}"))?;
        Ok(Self {
            mailbox,
            stop,
            thread: worker.thread().clone(),
        })
    }

    pub fn take(&self) -> Mailbox {
        let mut mailbox = self
            .mailbox
            .lock()
            .expect("Nintendo input mailbox poisoned");
        Mailbox {
            sample: mailbox.sample,
            edges: std::mem::take(&mut mailbox.edges),
            reset: std::mem::take(&mut mailbox.reset),
            error: mailbox.error.clone(),
        }
    }
}
impl Drop for Reader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.unpark();
        // Reads time out; never join a native device thread on the UI thread.
    }
}

fn run(mailbox: &Mutex<Mailbox>, stop: &AtomicBool, ctx: &egui::Context) {
    while !stop.load(Ordering::Relaxed) {
        let result = read_device(mailbox, stop, ctx);
        mailbox
            .lock()
            .expect("Nintendo input mailbox poisoned")
            .disconnect(result.err());
        ctx.request_repaint();
        if !stop.load(Ordering::Relaxed) {
            thread::park_timeout(RETRY_INTERVAL);
        }
    }
}

fn read_device(
    mailbox: &Mutex<Mailbox>,
    stop: &AtomicBool,
    ctx: &egui::Context,
) -> Result<(), String> {
    let api = hidapi::HidApi::new()
        .map_err(|error| format!("Nintendo HID initialization failed: {error}"))?;
    api.set_open_exclusive(false);
    let mut devices = api.device_list().filter(|device| {
        device.vendor_id() == VENDOR_ID
            && device.product_id() == PRODUCT_ID
            && device.usage_page() == 1
            && device.usage() == 5
    });
    let Some(info) = devices.next() else {
        return Ok(());
    };
    if devices.next().is_some() {
        return Err(
            "Nintendo full-report compatibility requires a single connected Pro Controller".into(),
        );
    }
    let device = info
        .open_device(&api)
        .map_err(|error| format!("Nintendo HID open failed: {error}"))?;
    // Both are SPI reads. Report mode and flash contents are left untouched.
    for (sequence, mut request) in calibration_requests().into_iter().enumerate() {
        request[1] = sequence as u8;
        if let Err(error) = device.write(&request) {
            mailbox
                .lock()
                .expect("Nintendo input mailbox poisoned")
                .error = Some(format!(
                "Nintendo calibration read failed; using full-range defaults: {error}"
            ));
        }
    }
    let mut decoder = Decoder::default();
    let mut report = [0; REPORT_BYTES];
    let mut last_report = Instant::now();
    let mut has_full_reports = false;
    while !stop.load(Ordering::Relaxed) {
        let length = device
            .read_timeout(&mut report, READ_TIMEOUT_MS)
            .map_err(|error| format!("Nintendo HID read failed: {error}"))?;
        if let Some(sample) = decoder.decode(&report[..length]) {
            has_full_reports = true;
            last_report = Instant::now();
            if mailbox
                .lock()
                .expect("Nintendo input mailbox poisoned")
                .publish(sample)
            {
                ctx.request_repaint();
            }
        } else if has_full_reports && last_report.elapsed() >= REPORT_TIMEOUT {
            return Err("Nintendo controller stopped sending full input reports; release controls after reconnecting".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coalesces_sticks_but_retains_press_and_release_between_ui_polls() {
        let mut mailbox = Mailbox::default();
        let mut sample = Sample::default();
        sample.buttons[0] = true;
        mailbox.publish(sample);
        sample.buttons[0] = false;
        for index in 0..1000 {
            sample.axes[0] = index as f32 / 1000.0;
            mailbox.publish(sample);
        }
        assert_eq!(mailbox.edges, [(0, true), (0, false)]);
        assert_eq!(mailbox.sample, Some(sample));
    }
    #[test]
    fn overflow_and_disconnect_discard_pending_commands_and_request_rearming() {
        let mut mailbox = Mailbox::default();
        let mut sample = Sample::default();
        for _ in 0..MAX_BUTTON_EDGES + 1 {
            sample.buttons[0] = !sample.buttons[0];
            mailbox.publish(sample);
        }
        assert!(mailbox.reset);
        assert!(mailbox.edges.is_empty());
        assert!(mailbox.error.is_some());
        mailbox.disconnect(None);
        assert!(mailbox.sample.is_none());
        assert!(mailbox.edges.is_empty());
    }
}
