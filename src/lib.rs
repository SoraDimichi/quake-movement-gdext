mod camera;
pub mod jump;
mod movement;
pub mod quake_physics;
pub mod util;

pub use camera::QuakeCamera;
pub use jump::{JumpAction, JumpState};
pub use movement::QuakeController;
pub use quake_physics::{
    accelerate, air_accelerate, apply_friction, calc_bob, calc_roll, jump_velocity, lerp_f32,
};
pub use util::physics_dt;
