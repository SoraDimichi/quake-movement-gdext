mod camera;
mod movement;
pub mod quake_physics;

pub use camera::QuakeCamera;
pub use movement::QuakeController;
pub use quake_physics::{accelerate, apply_friction, jump_velocity, lerp_f32};
