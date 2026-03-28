//! Pure math functions implementing Quake/Source movement physics.
//!
//! These are stateless functions that only depend on `Vector3`.
//! They can be tested with `cargo test` without a Godot runtime.

use godot::prelude::Vector3;

/// Source-engine acceleration: projects velocity onto wish direction,
/// accelerates up to `max_vel` along that axis without decelerating.
///
/// This is the core formula from id Software's Quake source.
/// If the player is already moving faster than `max_vel` in the wish direction,
/// no acceleration is applied — but the speed is NOT reduced. This property
/// is what enables bunny hopping and air strafing.
#[must_use]
pub fn accelerate(
    prev_velocity: Vector3,
    accel_dir: Vector3,
    acceleration: f32,
    max_vel: f32,
    dt: f32,
) -> Vector3 {
    let projected_vel = prev_velocity.dot(accel_dir);
    let accel_vel = (max_vel - projected_vel).clamp(0.0, acceleration * dt);
    prev_velocity + accel_dir * accel_vel
}

/// Quake-style ground friction with `stop_speed` threshold.
///
/// Ported from dot-fps-controller `states/run.gd`.
/// When speed is below `stop_speed`, uses `stop_speed` as the control value
/// instead of the current speed. This prevents the "infinite sliding" problem
/// where very low speeds produce proportionally tiny friction drops.
#[must_use]
pub fn apply_friction(velocity: Vector3, friction: f32, stop_speed: f32, dt: f32) -> Vector3 {
    let speed = velocity.length();
    if speed < 0.001 {
        return Vector3::ZERO;
    }
    let control = speed.max(stop_speed);
    let drop = control * friction * dt;
    let new_speed = (speed - drop).max(0.0);
    velocity * (new_speed / speed)
}

/// Calculate jump velocity from `jump_force` multiplier and gravity.
///
/// Formula: `sqrt(4 * jump_force * gravity)`
#[must_use]
pub fn jump_velocity(jump_force: f32, gravity: f32) -> f32 {
    (4.0 * jump_force * gravity).sqrt()
}

/// Linear interpolation between two `f32` values.
#[must_use]
pub fn lerp_f32(from: f32, to: f32, weight: f32) -> f32 {
    (to - from).mul_add(weight.clamp(0.0, 1.0), from)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- accelerate tests --

    #[test]
    fn accelerate_from_zero() {
        let result = accelerate(
            Vector3::ZERO,
            Vector3::new(1.0, 0.0, 0.0),
            250.0,
            10.0,
            1.0 / 60.0,
        );
        let expected = (250.0_f32 / 60.0).min(10.0);
        assert!((result.x - expected).abs() < 0.001);
        assert!(result.y.abs() < f32::EPSILON);
        assert!(result.z.abs() < f32::EPSILON);
    }

    #[test]
    fn accelerate_caps_at_max_velocity() {
        let result = accelerate(
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            250.0,
            10.0,
            1.0 / 60.0,
        );
        assert!((result.x - 10.0).abs() < 0.001);
    }

    #[test]
    fn accelerate_above_max_no_decel() {
        let result = accelerate(
            Vector3::new(20.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            250.0,
            10.0,
            1.0 / 60.0,
        );
        assert!((result.x - 20.0).abs() < 0.001);
    }

    #[test]
    fn accelerate_perpendicular_allows_full_accel() {
        let result = accelerate(
            Vector3::new(0.0, 0.0, 20.0),
            Vector3::new(1.0, 0.0, 0.0),
            85.0,
            1.5,
            1.0 / 60.0,
        );
        let expected = (85.0_f32 / 60.0).min(1.5);
        assert!((result.x - expected).abs() < 0.001);
        assert!((result.z - 20.0).abs() < 0.001);
    }

    #[test]
    fn accelerate_deterministic() {
        let a = accelerate(
            Vector3::new(5.0, 0.0, 3.0),
            Vector3::new(0.707, 0.0, 0.707),
            250.0,
            10.0,
            1.0 / 60.0,
        );
        let b = accelerate(
            Vector3::new(5.0, 0.0, 3.0),
            Vector3::new(0.707, 0.0, 0.707),
            250.0,
            10.0,
            1.0 / 60.0,
        );
        assert_eq!(a, b);
    }

    #[test]
    fn air_strafe_increases_speed() {
        let forward = Vector3::new(0.0, 0.0, 10.0);
        let wish_right = Vector3::new(1.0, 0.0, 0.0);
        let result = accelerate(forward, wish_right, 85.0, 1.5, 1.0 / 60.0);
        assert!(result.length() > forward.length());
    }

    // -- apply_friction tests --

    #[test]
    fn apply_friction_reduces_speed() {
        let vel = Vector3::new(10.0, 0.0, 0.0);
        let result = apply_friction(vel, 6.0, 1.5, 1.0 / 60.0);
        assert!(result.length() < vel.length());
        assert!(result.length() > 0.0);
    }

    #[test]
    fn apply_friction_does_not_go_negative() {
        let vel = Vector3::new(0.1, 0.0, 0.0);
        let result = apply_friction(vel, 100.0, 10.0, 1.0);
        assert!(result.length() >= 0.0);
        assert!(result.length() < 0.001);
    }

    #[test]
    fn apply_friction_stop_speed_threshold() {
        // With low speed, stop_speed should kick in as the control value
        let vel = Vector3::new(0.5, 0.0, 0.0);
        let stop_speed = 2.0;
        let result = apply_friction(vel, 6.0, stop_speed, 1.0 / 60.0);
        // Friction should be stronger than if we used speed as control
        let naive_drop = 0.5 * 6.0 / 60.0; // 0.05
        let stop_drop = 2.0 * 6.0 / 60.0; // 0.2
        assert!(
            naive_drop < stop_drop,
            "stop_speed should increase friction"
        );
        // With stop_speed: new_speed = 0.5 - 0.2 = 0.3
        assert!((result.length() - 0.3).abs() < 0.01);
    }

    #[test]
    fn apply_friction_preserves_direction() {
        let vel = Vector3::new(3.0, 0.0, 4.0); // length = 5
        let result = apply_friction(vel, 6.0, 1.5, 1.0 / 60.0);
        let dir_before = vel.normalized();
        let dir_after = result.normalized();
        assert!((dir_before.x - dir_after.x).abs() < 0.001);
        assert!((dir_before.z - dir_after.z).abs() < 0.001);
    }

    #[test]
    fn apply_friction_zero_velocity() {
        let result = apply_friction(Vector3::ZERO, 6.0, 1.5, 1.0 / 60.0);
        assert!(result.length() < 0.001);
    }

    // -- jump_velocity tests --

    #[test]
    fn jump_velocity_formula() {
        let speed = jump_velocity(1.0, 30.0);
        let expected = (4.0_f32 * 1.0 * 30.0).sqrt();
        assert!((speed - expected).abs() < 0.001);
    }

    #[test]
    fn jump_velocity_scales_with_force() {
        let low = jump_velocity(1.0, 30.0);
        let high = jump_velocity(2.0, 30.0);
        assert!(high > low);
    }

    #[test]
    fn jump_velocity_zero_force() {
        let speed = jump_velocity(0.0, 30.0);
        assert!(speed.abs() < f32::EPSILON);
    }

    // -- lerp_f32 tests --

    #[test]
    fn lerp_f32_midpoint() {
        assert!((lerp_f32(0.0, 10.0, 0.5) - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn lerp_f32_clamped() {
        assert!((lerp_f32(0.0, 10.0, 2.0) - 10.0).abs() < f32::EPSILON);
        assert!((lerp_f32(0.0, 10.0, -1.0)).abs() < f32::EPSILON);
    }
}
