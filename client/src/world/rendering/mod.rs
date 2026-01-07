pub mod materials;
pub mod meshing;
pub mod render;
pub mod render_distance;
pub mod voxel;
pub mod water;

pub use materials::*;
pub use render::*;
pub use render_distance::*;
// Note: water module types are imported directly where needed (game.rs)
// to avoid polluting the rendering namespace
