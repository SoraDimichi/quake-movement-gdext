//! Shared utilities for the quake-movement-gdext crate.

use godot::classes::Engine;
use godot::obj::Singleton;

/// Get physics delta time from Godot's physics tick rate.
/// Avoids the `f64 as f32` cast that clippy flags.
#[must_use]
pub fn physics_dt() -> f32 {
    let ticks = Engine::singleton().get_physics_ticks_per_second();
    1.0 / f32::from(i16::try_from(ticks).unwrap_or(60))
}
