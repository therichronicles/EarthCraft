#[derive(Clone, Debug, PartialEq)]
pub struct Placemark {
    pub name: String,
    pub desc: String,
    pub lat: f64,
    pub lon: f64,
}

mod tree;
pub use tree::{PlaceItem, PlaceNode, Places};
