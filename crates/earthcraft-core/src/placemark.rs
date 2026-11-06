mod path;
pub use path::GeoPath;

/// Color and width for drawing a placemark's lines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineStyle {
    /// Straight (non-premultiplied) red, green, blue, alpha.
    pub rgba: [u8; 4],
    /// Width in KML pixels.
    pub width: f32,
}

/// A named place. `lat`/`lon` is its pin position (the first coordinate of
/// its geometry); `paths` holds any line or polygon outlines it also draws.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Placemark {
    pub name: String,
    pub desc: String,
    pub lat: f64,
    pub lon: f64,
    pub paths: Vec<GeoPath>,
    /// Named fields from the file (KML `ExtendedData`), in file order.
    pub attributes: Vec<(String, String)>,
    /// Style for `paths`; `None` draws them with the default.
    pub line_style: Option<LineStyle>,
}

mod tree;
pub use tree::{PlaceItem, PlaceNode, Places};
