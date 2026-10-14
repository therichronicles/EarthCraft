//! Map services and persistence. Background loaders report values to the desktop app.
#![forbid(unsafe_code)]
pub mod aircraft;
pub mod boundaries;
mod error;
mod http;
pub mod kml;
mod paths;
pub mod search;
pub mod settings;
pub mod tiles;
pub mod wayback;
pub use error::Error;
mod cpu;
mod features;
pub mod files;
mod json_fields;
pub mod runtime;

#[cfg(feature = "profiling")]
pub mod profiling;

pub mod links;
pub mod live_maps;
pub mod planet;
pub mod roads;
pub mod satellites;
pub mod traffic;
pub mod vessels;
