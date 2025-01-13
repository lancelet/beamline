#[macro_use]
pub mod compare;
mod interval;
mod line;
mod polygon;
mod types;

pub use types::P2;
pub use types::V2;

pub use line::Line;
pub use polygon::Polygon;
pub use polygon::WindingDirection;
