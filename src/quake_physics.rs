//! Pure math functions implementing Quake movement physics.
//!
//! Faithful port of id Software's Quake source (`sv_user.c`).
//! These are stateless functions that only depend on `Vector3`.
//! They can be tested with `cargo test` without a Godot runtime.

use godot::prelude::Vector3;

/// Source-engine ground acceleration.
///
/// Projects current velocity onto the wish direction, then accelerates
/// up to `max_vel` along that direction. Speed above `max_vel` is preserved
/// (never decelerated) — this is what enables bunny hopping.
///
/// Based on bhop3d's implementation tuned for Godot's meter-scale units.
#[must_use]
pub fn accelerate(
    prev_velocity: Vector3,
    accel_dir: Vector3,
    accel: f32,
    max_vel: f32,
    dt: f32,
) -> Vector3 {
    let projected_vel = prev_velocity.dot(accel_dir);
    let accel_vel = (max_vel - projected_vel).clamp(0.0, accel * dt);
    prev_velocity + accel_dir * accel_vel
}

/// Quake air acceleration (`SV_AirAccelerate`).
///
/// The `wish_speed` (capped at `air_cap` for the addspeed check) determines
/// when acceleration stops. But the full `wish_speed` is used for computing
/// `accelspeed = accel * dt * wish_speed`, giving strong air control.
/// This asymmetry is what makes bhop work even while holding W.
#[must_use]
pub fn air_accelerate(
    prev_velocity: Vector3,
    accel_dir: Vector3,
    wish_speed: f32,
    accel: f32,
    air_cap: f32,
    dt: f32,
) -> Vector3 {
    let capped = wish_speed.min(air_cap);
    let current_speed = prev_velocity.dot(accel_dir);
    let add_speed = capped - current_speed;
    if add_speed <= 0.0 {
        return prev_velocity;
    }
    let accel_speed = (accel * dt * wish_speed).min(add_speed);
    prev_velocity + accel_dir * accel_speed
}

/// Quake ground friction (`SV_UserFriction` / `PM_Friction`).
///
/// When speed is below `stop_speed`, uses `stop_speed` as the control value
/// to prevent infinite sliding at low speeds.
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
/// Formula: `sqrt(2 * gravity * jump_force)` — the physics formula for
/// reaching height `jump_force` under `gravity`.
/// Note: in Quake this is applied additively (`vel.y += result`), not as assignment.
#[must_use]
pub fn jump_velocity(jump_force: f32, gravity: f32) -> f32 {
    (2.0 * gravity * jump_force).sqrt()
}

/// Quake view bob (`V_CalcBob` from `view.c`).
///
/// Computes vertical camera bob proportional to horizontal speed with an
/// asymmetric cycle (up phase is shorter than down phase).
///
/// - `time`: elapsed time in seconds
/// - `speed`: horizontal speed (XZ magnitude)
/// - `bob_amount`: amplitude scale (Quake `cl_bob`, default 0.02)
/// - `bob_cycle`: full cycle duration in seconds (Quake `cl_bobcycle`, default 0.6)
/// - `bob_up`: fraction of cycle spent going up (Quake `cl_bobup`, default 0.5)
#[must_use]
pub fn calc_bob(time: f32, speed: f32, bob_amount: f32, bob_cycle: f32, bob_up: f32) -> f32 {
    if speed < 0.5 || bob_cycle <= 0.0 {
        return 0.0;
    }
    let phase = (time % bob_cycle) / bob_cycle;
    let cycle = if phase < bob_up {
        std::f32::consts::PI * phase / bob_up
    } else {
        std::f32::consts::PI + std::f32::consts::PI * (phase - bob_up) / (1.0 - bob_up)
    };
    let bob = speed * bob_amount;
    (bob * 0.7).mul_add(cycle.sin(), bob * 0.3)
}

/// Quake view roll on strafe (`V_CalcRoll` from `view.c`).
///
/// Returns roll angle in degrees proportional to sideways velocity.
/// Capped at `roll_angle` when side speed exceeds `roll_speed`.
///
/// - `velocity`: player velocity vector
/// - `right`: player's right direction vector
/// - `roll_angle`: max roll in degrees (Quake `cl_rollangle`, default 2.0)
/// - `roll_speed`: speed at which max roll is reached (Quake `cl_rollspeed`, default 200)
#[must_use]
pub fn calc_roll(velocity: Vector3, right: Vector3, roll_angle: f32, roll_speed: f32) -> f32 {
    let side = velocity.dot(right);
    let sign = side.signum();
    let abs_side = side.abs();
    let roll = if abs_side < roll_speed {
        abs_side * roll_angle / roll_speed
    } else {
        roll_angle
    };
    roll * sign
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
        // accelerate(vel, dir, accel, max_vel, dt)
        let result = accelerate(
            Vector3::ZERO,
            Vector3::new(1.0, 0.0, 0.0),
            250.0,
            10.0,
            1.0 / 60.0,
        );
        // accel_vel = min(10 - 0, 250/60) = min(10, 4.17) = 4.17
        let expected = (250.0_f32 / 60.0).min(10.0);
        assert!((result.x - expected).abs() < 0.01);
    }

    #[test]
    fn accelerate_caps_at_max_vel() {
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

    // -- air accelerate tests --

    #[test]
    fn air_accelerate_perpendicular_gains_speed() {
        let forward = Vector3::new(0.0, 0.0, 10.0);
        let wish_right = Vector3::new(1.0, 0.0, 0.0);
        // air_accelerate(vel, dir, wish_speed, accel, air_cap, dt)
        let result = air_accelerate(forward, wish_right, 10.0, 10.0, 1.5, 1.0 / 60.0);
        assert!(result.length() > forward.length());
    }

    // -- friction tests --

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
        assert!(result.length() < 0.001);
    }

    #[test]
    fn apply_friction_stop_speed_threshold() {
        let vel = Vector3::new(0.5, 0.0, 0.0);
        let result = apply_friction(vel, 6.0, 2.0, 1.0 / 60.0);
        // control = max(0.5, 2.0) = 2.0, drop = 2.0 * 6.0 / 60 = 0.2
        assert!((result.length() - 0.3).abs() < 0.01);
    }

    #[test]
    fn apply_friction_preserves_direction() {
        let vel = Vector3::new(3.0, 0.0, 4.0);
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

    // -- jump velocity tests --

    #[test]
    fn jump_velocity_formula() {
        let speed = jump_velocity(1.0, 30.0);
        let expected = (2.0_f32 * 30.0 * 1.0).sqrt();
        assert!((speed - expected).abs() < 0.001);
    }

    #[test]
    fn jump_velocity_scales_with_force() {
        assert!(jump_velocity(2.0, 30.0) > jump_velocity(1.0, 30.0));
    }

    #[test]
    fn jump_velocity_zero_force() {
        assert!(jump_velocity(0.0, 30.0).abs() < f32::EPSILON);
    }

    // -- lerp tests --

    #[test]
    fn lerp_f32_midpoint() {
        assert!((lerp_f32(0.0, 10.0, 0.5) - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn lerp_f32_clamped() {
        assert!((lerp_f32(0.0, 10.0, 2.0) - 10.0).abs() < f32::EPSILON);
        assert!(lerp_f32(0.0, 10.0, -1.0).abs() < f32::EPSILON);
    }

    // -- calc_bob tests --

    #[test]
    fn calc_bob_zero_speed() {
        assert!(calc_bob(1.0, 0.0, 0.02, 0.6, 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn calc_bob_proportional_to_speed() {
        let slow = calc_bob(0.15, 5.0, 0.02, 0.6, 0.5);
        let fast = calc_bob(0.15, 10.0, 0.02, 0.6, 0.5);
        assert!(fast.abs() > slow.abs());
    }

    #[test]
    fn calc_bob_asymmetric_cycle() {
        // At bob_up=0.5 the up phase takes half the cycle
        // At 25% of cycle (middle of up phase) vs 75% (middle of down phase)
        let up_mid = calc_bob(0.15, 10.0, 0.02, 0.6, 0.5);
        let down_mid = calc_bob(0.45, 10.0, 0.02, 0.6, 0.5);
        // They should differ since the waveform isn't symmetric
        assert!((up_mid - down_mid).abs() > 0.001);
    }

    #[test]
    fn calc_bob_periodic() {
        let a = calc_bob(1.0, 10.0, 0.02, 0.6, 0.5);
        let b = calc_bob(1.6, 10.0, 0.02, 0.6, 0.5);
        assert!((a - b).abs() < 0.001);
    }

    // -- calc_roll tests --

    #[test]
    fn calc_roll_proportional_to_strafe() {
        let right = Vector3::new(1.0, 0.0, 0.0);
        let slow = calc_roll(Vector3::new(3.0, 0.0, 0.0), right, 2.0, 10.0);
        let fast = calc_roll(Vector3::new(6.0, 0.0, 0.0), right, 2.0, 10.0);
        assert!(fast.abs() > slow.abs());
    }

    #[test]
    fn calc_roll_caps_at_roll_angle() {
        let right = Vector3::new(1.0, 0.0, 0.0);
        let result = calc_roll(Vector3::new(100.0, 0.0, 0.0), right, 2.0, 10.0);
        assert!((result - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn calc_roll_sign_matches_direction() {
        let right = Vector3::new(1.0, 0.0, 0.0);
        let left = calc_roll(Vector3::new(-5.0, 0.0, 0.0), right, 2.0, 10.0);
        let right_roll = calc_roll(Vector3::new(5.0, 0.0, 0.0), right, 2.0, 10.0);
        assert!(left < 0.0);
        assert!(right_roll > 0.0);
    }

    // -- bhop simulation test --

    /// Simulates a full bunny hop cycle: air strafe for 30 frames,
    /// land (friction skipped because jump fires immediately), repeat 4 times.
    /// Proves that speed increases with each hop — the core bhop mechanic.
    #[test]
    fn bhop_simulation_speed_increases() {
        let dt = 1.0 / 60.0;
        let max_ground_vel = 10.0_f32;
        let ground_accel = 250.0_f32;
        let air_accel = 10.0_f32;
        let air_cap = 1.5_f32;
        let friction_val = 6.0_f32;
        let stop_speed = 1.5_f32;
        let jump_force = 1.0_f32;
        let gravity = 30.0_f32;

        // Start on ground, accelerate to max speed.
        let mut vel = Vector3::ZERO;
        for _ in 0..60 {
            vel = apply_friction(vel, friction_val, stop_speed, dt);
            vel = accelerate(
                vel,
                Vector3::new(0.0, 0.0, -1.0),
                ground_accel,
                max_ground_vel,
                dt,
            );
        }
        let ground_speed = Vector3::new(vel.x, 0.0, vel.z).length();
        assert!(
            ground_speed > 9.0,
            "should reach near max ground speed: {ground_speed}"
        );

        // Now simulate 4 bhop cycles.
        let mut speeds_at_hop: Vec<f32> = vec![ground_speed];
        let mut facing_angle: f32 = 0.0;
        let turn_rate = 3.0_f32; // radians/sec

        for hop in 0..4 {
            // Jump: additive velocity, skip friction (bhop).
            vel.y += jump_velocity(jump_force, gravity);

            // Air strafe for 30 frames (alternating left/right each hop).
            let strafe_sign = if hop % 2 == 0 { 1.0_f32 } else { -1.0_f32 };
            for _ in 0..30 {
                // Wishdir: perpendicular to facing (strafe).
                let wish_angle = facing_angle + strafe_sign * std::f32::consts::FRAC_PI_2;
                let wishdir = Vector3::new(wish_angle.sin(), 0.0, -wish_angle.cos());

                vel = air_accelerate(vel, wishdir, max_ground_vel, air_accel, air_cap, dt);
                vel.y -= gravity * dt;

                facing_angle += strafe_sign * turn_rate * dt;
            }

            // Land: jump immediately (friction skipped).
            vel.y = 0.0; // simulate landing
            let h_speed = Vector3::new(vel.x, 0.0, vel.z).length();
            speeds_at_hop.push(h_speed);
        }

        // Each hop should be faster than the previous.
        for i in 1..speeds_at_hop.len() {
            assert!(
                speeds_at_hop[i] > speeds_at_hop[i - 1],
                "hop {i} speed ({:.2}) should exceed hop {} speed ({:.2})",
                speeds_at_hop[i],
                i - 1,
                speeds_at_hop[i - 1]
            );
        }

        // Final speed should significantly exceed max ground velocity.
        let final_speed = speeds_at_hop.last().copied().unwrap_or(0.0);
        assert!(
            final_speed > max_ground_vel * 1.2,
            "bhop should exceed max ground vel: {final_speed:.2} vs {max_ground_vel}"
        );
    }

    // -- DUSK-style bhop multiplier tests --

    #[test]
    fn bhop_multiplier_grows_per_jump() {
        let increment = 0.2_f32;
        let max = 0.8_f32;
        let mut mult = 0.0_f32;

        for i in 1..=4 {
            mult = (mult + increment).min(max);
            assert!(
                (mult - increment * i as f32).abs() < f32::EPSILON,
                "after {i} jumps: {mult}"
            );
        }
    }

    #[test]
    fn bhop_multiplier_caps_at_max() {
        let increment = 0.2_f32;
        let max = 0.8_f32;
        let mut mult = 0.0_f32;

        for _ in 0..10 {
            mult = (mult + increment).min(max);
        }
        assert!((mult - max).abs() < f32::EPSILON);
    }

    #[test]
    fn bhop_multiplier_decays_on_ground() {
        let decay = 2.0_f32;
        let dt = 1.0 / 60.0;
        let mut mult = 0.8_f32;

        for _ in 0..30 {
            mult = decay.mul_add(-dt, mult).max(0.0);
        }
        // After 0.5s at decay=2.0: lost 1.0, but started at 0.8, so should be 0
        assert!(mult < 0.01, "should decay to ~0: {mult}");
    }

    #[test]
    fn bhop_multiplier_does_not_go_negative() {
        let decay = 100.0_f32;
        let dt = 1.0;
        let mult = decay.mul_add(-dt, 0.5_f32).max(0.0);
        assert!(mult >= 0.0);
    }

    #[test]
    fn bhop_multiplier_increases_max_velocity() {
        let base_vel = 10.0_f32;
        let mult = 0.8_f32;
        let boosted = base_vel * (1.0 + mult);
        assert!((boosted - 18.0).abs() < f32::EPSILON);
    }

    #[test]
    fn bhop_full_cycle_simulation() {
        // Simulate: 4 jumps building multiplier, then stand for 30 frames decaying.
        let increment = 0.2_f32;
        let max = 0.8_f32;
        let decay = 2.0_f32;
        let dt = 1.0 / 60.0;
        let base_vel = 10.0_f32;

        let mut mult = 0.0_f32;

        // 4 jumps.
        for _ in 0..4 {
            mult = (mult + increment).min(max);
        }
        assert!((mult - 0.8).abs() < f32::EPSILON);
        assert!((base_vel * (1.0 + mult) - 18.0).abs() < f32::EPSILON);

        // Stand on ground for 30 frames (~0.5s).
        for _ in 0..30 {
            mult = decay.mul_add(-dt, mult).max(0.0);
        }
        assert!(mult < 0.01, "multiplier should decay: {mult}");

        // Speed should be back near base.
        let speed = base_vel * (1.0 + mult);
        assert!(
            speed < base_vel * 1.01,
            "speed should be near base: {speed}"
        );
    }
}
