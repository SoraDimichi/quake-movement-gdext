mod camera;
mod movement;
pub mod quake_physics;

pub use camera::QuakeCamera;
pub use movement::QuakeController;
pub use quake_physics::{
    accelerate, air_accelerate, apply_friction, calc_bob, calc_roll, jump_velocity, lerp_f32,
};

// Re-export QuakeController::accelerate for backward compat
// (same signature as quake_physics::accelerate)
