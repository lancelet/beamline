#[macro_use]
pub mod compare;
#[allow(unused)] // TODO: For development only.
mod bbox;
#[allow(unused)] // TODO: For development only.
mod interval;
mod line;
mod polygon;
pub mod style;
mod types;

pub use types::P2;
pub use types::V2;

pub use line::Line;
pub use polygon::Polygon;
pub use polygon::WindingDirection;
