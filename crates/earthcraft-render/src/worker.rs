//! One active raster job, one replaceable pending view, and one completed frame.
use crate::RenderCfg;
use earthcraft_core::camera::{Basis, Cam};
use std::sync::{Arc, Condvar, Mutex};

pub struct RenderRequest {
    pub width: usize,
    pub height: usize,
    pub cam: Cam,
    pub basis: Basis,
    pub cfg: RenderCfg,
    pub overlays: Option<crate::overlays::OverlayInput>,
    pub aircraft: Option<crate::aircraft::ProjectionInput>,
    pub live_layers: Option<crate::live_layers::LiveInput>,
    // Camera motion may lag by one job; provider/style/size changes must not.
    pub presentation: u64,
}

pub struct RenderedFrame<T = ()> {
    pub prepared: T,
    #[cfg(feature = "profiling")]
    pub submitted: std::time::Instant,
    pub request: RenderRequest,
    pub pixels: Vec<u8>,
    pub waiting: Vec<earthcraft_data::tiles::TileKey>,
    pub required: Vec<earthcraft_data::tiles::TileKey>,
}

struct PendingRequest {
    request: RenderRequest,
    #[cfg(feature = "profiling")]
    submitted: std::time::Instant,
}

struct Mailbox<T> {
    pending: Option<PendingRequest>,
    completed: Option<RenderedFrame<T>>,
    active: bool,
    stopped: bool,
    spare: Option<Vec<u8>>,
}

struct Shared<T> {
    mailbox: Mutex<Mailbox<T>>,
    wake: Condvar,
}

pub struct RenderWorker<T = ()> {
    shared: Arc<Shared<T>>,
}

impl RenderWorker {
    pub fn new(
        render: impl FnMut(&RenderRequest, Vec<u8>) -> crate::RenderedView + Send + 'static,
        repaint: impl Fn() + Send + 'static,
        admit: impl Fn(&crate::RenderedView) + Send + 'static,
    ) -> std::io::Result<Self> {
        Self::with_prepare(render, repaint, admit, |_, _| ())
    }
}

impl<T: Send + 'static> RenderWorker<T> {
    /// Prepare backend-specific CPU resources on the coordinator, before publication.
    pub fn with_prepare(
        mut render: impl FnMut(&RenderRequest, Vec<u8>) -> crate::RenderedView + Send + 'static,
        repaint: impl Fn() + Send + 'static,
        admit: impl Fn(&crate::RenderedView) + Send + 'static,
        mut prepare: impl FnMut(&RenderRequest, &[u8]) -> T + Send + 'static,
    ) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            mailbox: Mutex::new(Mailbox {
                pending: None,
                completed: None,
                active: false,
                stopped: false,
                spare: None,
            }),
            wake: Condvar::new(),
        });
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("earthcraft-frame".into())
            .spawn(move || {
                loop {
                    let (pending, buffer) = {
                        let mut mailbox = worker.mailbox.lock().unwrap();
                        while !mailbox.stopped && mailbox.pending.is_none() {
                            mailbox = worker.wake.wait(mailbox).unwrap();
                        }
                        if mailbox.stopped {
                            return;
                        }
                        mailbox.active = true;
                        (
                            mailbox.pending.take().unwrap(),
                            mailbox.spare.take().unwrap_or_default(),
                        )
                    };
                    #[cfg(feature = "profiling")]
                    earthcraft_data::profiling::Span::elapsed(
                        "frame.queue",
                        pending.submitted,
                        Default::default(),
                    );
                    let request = pending.request;
                    // Rasterization holds no mailbox lock. Admission is synchronized below.
                    let frame = render(&request, buffer);
                    {
                        let mut mailbox = worker.mailbox.lock().unwrap();
                        if mailbox.stopped {
                            return;
                        }
                        if mailbox
                            .pending
                            .as_ref()
                            .is_some_and(|next| next.request.presentation != request.presentation)
                        {
                            mailbox.active = false;
                            mailbox.spare = Some(frame.pixels);
                            drop(mailbox);
                            repaint();
                            continue;
                        }
                    }
                    let prepared = prepare(&request, &frame.pixels);
                    let mut mailbox = worker.mailbox.lock().unwrap();
                    mailbox.active = false;
                    if mailbox.stopped {
                        return;
                    }
                    let obsolete = mailbox
                        .pending
                        .as_ref()
                        .is_some_and(|next| next.request.presentation != request.presentation);
                    if !obsolete {
                        // Admission is short and bounded; synchronize it with pending
                        // presentation changes so incompatible rasters cannot fetch tiles.
                        admit(&frame);
                        if let Some(previous) = mailbox.completed.take() {
                            mailbox.spare = Some(previous.pixels);
                        }
                        let required = frame.required_tiles();
                        mailbox.completed = Some(RenderedFrame {
                            prepared,
                            #[cfg(feature = "profiling")]
                            submitted: pending.submitted,
                            request,
                            pixels: frame.pixels,
                            waiting: frame.waiting,
                            required,
                        });
                    } else {
                        mailbox.spare = Some(frame.pixels);
                    }
                    drop(mailbox);
                    repaint();
                }
            })?;
        Ok(Self { shared })
    }

    pub fn recycle(&self, buffer: Vec<u8>) {
        self.shared.mailbox.lock().unwrap().spare = Some(buffer);
    }

    pub fn submit(&self, request: RenderRequest) {
        self.shared.mailbox.lock().unwrap().pending = Some(PendingRequest {
            request,
            #[cfg(feature = "profiling")]
            submitted: std::time::Instant::now(),
        });
        self.shared.wake.notify_one();
    }

    pub fn take_completed(&self, presentation: u64) -> Option<RenderedFrame<T>> {
        let mut mailbox = self.shared.mailbox.lock().unwrap();
        let frame = mailbox.completed.take()?;
        if frame.request.presentation == presentation {
            Some(frame)
        } else {
            mailbox.spare = Some(frame.pixels);
            None
        }
    }

    pub fn is_idle(&self) -> bool {
        let mailbox = self.shared.mailbox.lock().unwrap();
        !mailbox.active && mailbox.pending.is_none()
    }
}

impl<T> Drop for RenderWorker<T> {
    fn drop(&mut self) {
        let mut mailbox = self.shared.mailbox.lock().unwrap();
        mailbox.stopped = true;
        mailbox.pending = None;
        mailbox.completed = None;
        drop(mailbox);
        self.shared.wake.notify_one();
        // Closing the window never waits for a raster job. The coordinator exits
        // after the current job, releasing its cache and repaint callback.
    }
}

#[cfg(test)]
mod tests;
