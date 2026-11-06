mod path;
pub use path::GeoPath;

/// A named place. `lat`/`lon` is its pin position (the first coordinate of
/// its geometry); `paths` holds any line or polygon outlines it also draws.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Placemark {
    pub name: String,
    pub desc: String,
    pub lat: f64,
    pub lon: f64,
    pub paths: Vec<GeoPath>,
}

mod tree;
pub use tree::{PlaceItem, PlaceNode, Places};
