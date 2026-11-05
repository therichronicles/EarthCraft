//! Ordered, bounded file jobs; native dialogs and applying results stay in app.
pub(crate) mod images;
mod screenshot;
mod session;
use earthcraft_core::{language::Lang, placemark::Places};
pub use images::{ImageSource, ImageTarget, LoadedImage, PlanetSave};
pub use session::RestoredKml;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
};
const PENDING_FILE_JOBS: usize = 4;

pub enum FileOutcome {
    KmlRestored {
        files: Vec<RestoredKml>,
    },
    KmlLoaded {
        path: PathBuf,
        placemarks: Places,
    },
    KmlSaved {
        path: PathBuf,
    },
    ScreenshotSaved {
        path: PathBuf,
    },
    ImageLoaded {
        target: ImageTarget,
        image: LoadedImage,
    },
    PlanetLoaded {
        planet: crate::planet::LoadedPlanet,
    },
    PlanetSaved {
        path: PathBuf,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    RestoreKml,
    OpenKml,
    SaveKml,
    Screenshot,
    Image(ImageTarget),
    OpenPlanet,
    SavePlanet,
}
pub struct FileCompletion {
    pub kind: FileKind,
    pub result: Result<FileOutcome, crate::Error>,
}
enum Job {
    Restore(PathBuf, Lang),
    Open(PathBuf, Lang),
    Save(PathBuf, Places),
    Screenshot(PathBuf, u32, u32, Arc<Vec<u8>>),
    ScreenshotWithCredits(PathBuf, u32, u32, Arc<Vec<u8>>, String),
    Image(ImageTarget, ImageSource, u32),
    OpenPlanet(PathBuf, u32),
    SavePlanet(PlanetSave),
}
impl Job {
    fn kind(&self) -> FileKind {
        match self {
            Self::Restore(..) => FileKind::RestoreKml,
            Self::Open(..) => FileKind::OpenKml,
            Self::Save(..) => FileKind::SaveKml,
            Self::Screenshot(..) | Self::ScreenshotWithCredits(..) => FileKind::Screenshot,
            Self::Image(target, ..) => FileKind::Image(*target),
            Self::OpenPlanet(..) => FileKind::OpenPlanet,
            Self::SavePlanet(..) => FileKind::SavePlanet,
        }
    }
    fn run(self) -> Result<FileOutcome, crate::Error> {
        match self {
            Self::Restore(path, lang) => session::restore(&path, lang),
            Self::Open(path, lang) => {
                let path = std::path::absolute(path)?;
                let placemarks = crate::kml::load_kml(&path.to_string_lossy(), lang)?;
                Ok(FileOutcome::KmlLoaded { path, placemarks })
            }
            Self::Save(path, marks) => {
                let path = std::path::absolute(path)?;
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                crate::kml::save_kml(&path.to_string_lossy(), &marks)?;
                Ok(FileOutcome::KmlSaved { path })
            }
            Self::Screenshot(path, width, height, pixels) => {
                screenshot::save(path, width, height, &pixels, None)
            }
            Self::ScreenshotWithCredits(path, width, height, pixels, credits) => {
                screenshot::save(path, width, height, &pixels, Some(&credits))
            }
            Self::Image(target, source, max_side) => {
                let encoded = source.read()?;
                let pixels = images::decode_texture(&encoded, max_side)?;
                Ok(FileOutcome::ImageLoaded {
                    target,
                    image: LoadedImage { pixels, encoded },
                })
            }
            Self::OpenPlanet(path, max_side) => Ok(FileOutcome::PlanetLoaded {
                planet: crate::planet::load_planet_with_limit(&path, max_side)?,
            }),
            Self::SavePlanet(PlanetSave {
                path,
                name,
                texture,
                sky,
            }) => {
                let texture = texture.read()?;
                let sky = sky.map(ImageSource::read).transpose()?.unwrap_or_default();
                let document = crate::planet::encode_planet(
                    &name,
                    texture.as_ref().clone(),
                    sky.as_ref().clone(),
                )?;
                std::fs::write(&path, document)?;
                Ok(FileOutcome::PlanetSaved { path })
            }
        }
    }
}
pub struct FileJobs {
    sender: tokio::sync::mpsc::Sender<Job>,
    outstanding: Arc<AtomicUsize>,
    texture_side_px: AtomicUsize,
}
impl FileJobs {
    pub fn new(
        runtime: &tokio::runtime::Handle,
        repaint: impl Fn() + Send + Sync + 'static,
    ) -> (Self, mpsc::Receiver<FileCompletion>) {
        let (sender, mut jobs) = tokio::sync::mpsc::channel::<Job>(PENDING_FILE_JOBS);
        let (results, receiver) = mpsc::channel();
        let repaint: Arc<dyn Fn() + Send + Sync> = Arc::new(repaint);
        let outstanding = Arc::new(AtomicUsize::new(0));
        let worker_outstanding = outstanding.clone();
        runtime.spawn(async move {
            while let Some(job) = jobs.recv().await {
                let kind = job.kind();
                let result = crate::cpu::run(crate::cpu::Service::Files, move || job.run()).await;
                let published = results.send(FileCompletion { kind, result });
                worker_outstanding.fetch_sub(1, Ordering::Release);
                if published.is_err() {
                    return;
                }
                repaint();
            }
        });
        (
            Self {
                sender,
                outstanding,
                texture_side_px: AtomicUsize::new(images::MAX_TEXTURE_SIDE_PX as usize),
            },
            receiver,
        )
    }
    fn reserve(&self) -> Result<tokio::sync::mpsc::Permit<'_, Job>, crate::Error> {
        self.sender.try_reserve().map_err(|error| match error {
            tokio::sync::mpsc::error::TrySendError::Full(_) => {
                crate::Error::from("file job queue is full")
            }
            tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                crate::Error::from("file service is unavailable")
            }
        })
    }
    /// Includes queued and active work; restart waits without blocking the UI.
    pub fn is_busy(&self) -> bool {
        self.outstanding.load(Ordering::Acquire) != 0
    }
    fn submit(&self, job: Job) -> Result<(), crate::Error> {
        let permit = self.reserve()?;
        self.outstanding.fetch_add(1, Ordering::Release);
        permit.send(job);
        Ok(())
    }
    /// Restore saved sources on the ordered file worker, without blocking the UI.
    pub fn restore_kml(&self, lang: Lang) -> Result<(), crate::Error> {
        self.submit(Job::Restore(
            crate::paths::cache_dir().join("open-kml.json"),
            lang,
        ))
    }
    pub fn open_kml(&self, path: PathBuf, lang: Lang) -> Result<(), crate::Error> {
        self.submit(Job::Open(path, lang))
    }
    pub fn save_kml(&self, path: PathBuf, marks: &Places) -> Result<(), crate::Error> {
        self.submit(Job::Save(path, marks.clone()))
    }
    pub fn screenshot(
        &self,
        path: PathBuf,
        width: u32,
        height: u32,
        pixels: &Arc<Vec<u8>>,
    ) -> Result<(), crate::Error> {
        self.submit(Job::Screenshot(path, width, height, pixels.clone()))
    }
    pub fn screenshot_with_credits(
        &self,
        path: PathBuf,
        width: u32,
        height: u32,
        pixels: &Arc<Vec<u8>>,
        credits: String,
    ) -> Result<(), crate::Error> {
        self.submit(Job::ScreenshotWithCredits(
            path,
            width,
            height,
            pixels.clone(),
            credits,
        ))
    }
    /// Capture the frontend device limit when subsequent image jobs are submitted.
    pub fn set_texture_side_limit(&self, max_side: usize) {
        self.texture_side_px.store(
            max_side.min(images::MAX_TEXTURE_SIDE_PX as usize),
            Ordering::Relaxed,
        );
    }
    pub fn load_image(&self, target: ImageTarget, source: ImageSource) -> Result<(), crate::Error> {
        self.submit(Job::Image(
            target,
            source,
            self.texture_side_px.load(Ordering::Relaxed) as u32,
        ))
    }
    pub fn open_planet(&self, path: PathBuf) -> Result<(), crate::Error> {
        self.submit(Job::OpenPlanet(
            path,
            self.texture_side_px.load(Ordering::Relaxed) as u32,
        ))
    }
    pub fn save_planet(&self, snapshot: PlanetSave) -> Result<(), crate::Error> {
        self.submit(Job::SavePlanet(snapshot))
    }
}

#[cfg(test)]
mod tests;
