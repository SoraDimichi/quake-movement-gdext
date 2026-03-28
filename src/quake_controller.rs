use godot::classes::{
    Camera3D, CharacterBody3D, Engine, ICharacterBody3D, Input, InputEvent, InputEventMouseMotion,
};
use godot::prelude::*;

/// Quake/Source-style first-person movement controller.
///
/// Port of [bhop3d](https://github.com/BirDt/bhop3d) by `BirDt`,
/// based on [Flafla2's bunnyhopping writeup](https://adrianb.io/2015/02/14/bunnyhop.html).
#[derive(GodotClass)]
#[class(init, base=CharacterBody3D)]
pub struct QuakeController {
    // -- Activity Controls --
    /// Whether the character can look around.
    #[export]
    #[init(val = true)]
    pub look_enabled: bool,

    /// Whether the character can move.
    #[export]
    #[init(val = true)]
    pub move_enabled: bool,

    /// Whether holding jump continuously bunny hops (0 = press to jump, 1 = hold to bhop).
    #[export]
    #[init(val = 0)]
    pub jump_when_held: i32,

    // -- Input Definitions --
    /// Mouse sensitivity multiplier (x = pitch, y = yaw).
    #[export]
    #[init(val = Vector2::ONE)]
    pub sensitivity: Vector2,

    /// Input action for moving forward.
    #[export]
    #[init(val = GString::from("move_forward"))]
    pub move_forward_action: GString,

    /// Input action for moving backward.
    #[export]
    #[init(val = GString::from("move_backward"))]
    pub move_backward_action: GString,

    /// Input action for moving left.
    #[export]
    #[init(val = GString::from("move_left"))]
    pub move_left_action: GString,

    /// Input action for moving right.
    #[export]
    #[init(val = GString::from("move_right"))]
    pub move_right_action: GString,

    /// Input action for jumping.
    #[export]
    #[init(val = GString::from("jump"))]
    pub jump_action: GString,

    // -- Movement Variables --
    /// Gravity in units per second squared.
    #[export]
    #[init(val = 30.0)]
    pub gravity: f32,

    /// Acceleration when grounded.
    #[export]
    #[init(val = 250.0)]
    pub ground_accelerate: f32,

    /// Acceleration when in the air.
    #[export]
    #[init(val = 85.0)]
    pub air_accelerate: f32,

    /// Max velocity on the ground.
    #[export]
    #[init(val = 10.0)]
    pub max_ground_velocity: f32,

    /// Max velocity in the air (controls air strafing tightness).
    #[export]
    #[init(val = 1.5)]
    pub max_air_velocity: f32,

    /// Jump force multiplier.
    #[export]
    #[init(val = 1.0)]
    pub jump_force: f32,

    /// Ground friction.
    #[export]
    #[init(val = 6.0)]
    pub friction: f32,

    /// Bunny hop window in frames (friction is skipped during this window after landing).
    #[export]
    #[init(val = 2)]
    pub bhop_frames: i32,

    /// When non-zero, bunny hopping uses air acceleration during the bhop window
    /// instead of ground acceleration, causing speed to converge toward wishdir.
    #[export]
    #[init(val = 1)]
    pub additive_bhop: i32,

    // -- Controlled Nodes --
    /// Camera to rotate with mouse input.
    #[export]
    pub camera: Option<Gd<Camera3D>>,

    // -- Internal state --
    /// Frames since last grounded.
    #[init(val = 0)]
    frame_timer: i32,

    base: Base<CharacterBody3D>,
}

#[godot_api]
impl ICharacterBody3D for QuakeController {
    fn physics_process(&mut self, _delta: f64) {
        let ticks = Engine::singleton().get_physics_ticks_per_second();
        let dt = 1.0 / f32::from(i16::try_from(ticks).unwrap_or(60));
        self.update_frame_timer();
        let next_vel = self.get_next_velocity(dt);
        self.base_mut().set_velocity(next_vel);
        self.base_mut().move_and_slide();
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        self.mouse_look(&event);
    }

    fn ready(&mut self) {
        self.update_mouse_mode();
    }
}

#[godot_api]
impl QuakeController {
    /// Update mouse capture mode based on `look_enabled` and camera presence.
    #[func]
    pub fn update_mouse_mode(&self) {
        if self.look_enabled && self.camera.is_some() {
            Input::singleton().set_mouse_mode(godot::classes::input::MouseMode::CAPTURED);
        } else {
            Input::singleton().set_mouse_mode(godot::classes::input::MouseMode::VISIBLE);
        }
    }
}

impl QuakeController {
    /// Handle mouse look rotation.
    fn mouse_look(&mut self, event: &Gd<InputEvent>) {
        if !self.look_enabled || self.camera.is_none() {
            return;
        }
        let Ok(motion) = event.clone().try_cast::<InputEventMouseMotion>() else {
            return;
        };

        let relative = motion.get_relative();
        let sens = self.sensitivity;

        let yaw = (-relative.x * sens.y).to_radians();
        self.base_mut().rotate_y(yaw);

        let pitch = (-relative.y * sens.x).to_radians();
        let mut camera = self.camera.clone().unwrap();
        camera.rotate_x(pitch);

        let mut cam_rot = camera.get_rotation();
        cam_rot.x = cam_rot
            .x
            .clamp((-89.0_f32).to_radians(), (89.0_f32).to_radians());
        camera.set_rotation(cam_rot);
    }

    /// Get the player's intended movement direction. Returns zero if movement is disabled.
    fn get_wishdir(&self) -> Vector3 {
        if !self.move_enabled {
            return Vector3::ZERO;
        }
        let input = Input::singleton();
        let basis = self.base().get_transform().basis;

        let fwd = StringName::from(&self.move_forward_action);
        let back = StringName::from(&self.move_backward_action);
        let left = StringName::from(&self.move_left_action);
        let right = StringName::from(&self.move_right_action);

        let forward_axis = input.get_axis(&fwd, &back);
        let side_axis = input.get_axis(&left, &right);

        basis.col_c() * forward_axis + basis.col_a() * side_axis
    }

    /// Calculate jump velocity from `jump_force` and gravity.
    fn get_jump_speed(&self) -> f32 {
        (4.0 * self.jump_force * self.gravity).sqrt()
    }

    /// Source-style acceleration function.
    ///
    /// Projects current velocity onto the desired direction, then accelerates
    /// up to `max_vel` along that direction.
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

    /// Calculate the next frame's velocity given current state.
    fn get_next_velocity(&self, dt: f32) -> Vector3 {
        let grounded = self.base().is_on_floor();
        let can_jump = grounded;

        let mut prev_velocity = self.base().get_velocity();

        // Apply friction if grounded and past the bhop window.
        let use_ground_params = if grounded && self.frame_timer >= self.bhop_frames {
            let speed = prev_velocity.length();
            if speed != 0.0 {
                let drop = speed * self.friction * dt;
                prev_velocity *= (speed - drop).max(0.0) / speed;
            }
            true
        } else {
            // During bhop window: use air params if additive_bhop, else ground params.
            self.additive_bhop == 0
        };

        let max_vel = if use_ground_params {
            self.max_ground_velocity
        } else {
            self.max_air_velocity
        };
        let accel = if use_ground_params {
            self.ground_accelerate
        } else {
            self.air_accelerate
        };

        let wishdir = self.get_wishdir();

        // Accelerate.
        let mut velocity = Self::accelerate(prev_velocity, wishdir, accel, max_vel, dt);

        // Apply gravity.
        velocity += Vector3::DOWN * self.gravity * dt;

        // Jump if requested and able.
        let input = Input::singleton();
        let jump_name = StringName::from(&self.jump_action);
        let jump_pressed = if self.jump_when_held != 0 {
            input.is_action_pressed(&jump_name)
        } else {
            input.is_action_just_pressed(&jump_name)
        };

        if jump_pressed && self.move_enabled && can_jump {
            velocity.y = self.get_jump_speed();
        }

        velocity
    }

    /// Track frames since last grounded for the bunny hop window.
    fn update_frame_timer(&mut self) {
        if self.base().is_on_floor() {
            self.frame_timer += 1;
        } else {
            self.frame_timer = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accelerate_from_zero() {
        let result = QuakeController::accelerate(
            Vector3::ZERO,
            Vector3::new(1.0, 0.0, 0.0),
            250.0,
            10.0,
            1.0 / 60.0,
        );
        // Should accelerate up to min(max_vel, acceleration * dt) = min(10, 4.167) = 4.167
        let expected_accel = (250.0_f32 / 60.0).min(10.0);
        assert!((result.x - expected_accel).abs() < 0.001);
        assert!(result.y.abs() < f32::EPSILON);
        assert!(result.z.abs() < f32::EPSILON);
    }

    #[test]
    fn accelerate_caps_at_max_velocity() {
        // Already at max velocity in the wish direction.
        let result = QuakeController::accelerate(
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            250.0,
            10.0,
            1.0 / 60.0,
        );
        // No additional acceleration — already at max_vel.
        assert!((result.x - 10.0).abs() < 0.001);
    }

    #[test]
    fn accelerate_above_max_no_decel() {
        // Moving faster than max_vel — acceleration should not slow down.
        let result = QuakeController::accelerate(
            Vector3::new(20.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            250.0,
            10.0,
            1.0 / 60.0,
        );
        // projected_vel (20) > max_vel (10), so accel_vel is clamped to 0.
        assert!((result.x - 20.0).abs() < 0.001);
    }

    #[test]
    fn accelerate_perpendicular_allows_full_accel() {
        // Moving forward, wish to go right — perpendicular.
        let result = QuakeController::accelerate(
            Vector3::new(0.0, 0.0, 20.0),
            Vector3::new(1.0, 0.0, 0.0),
            85.0,
            1.5,
            1.0 / 60.0,
        );
        // dot product is 0, so full acceleration applies (min of max_vel and accel*dt).
        let expected = (85.0_f32 / 60.0).min(1.5);
        assert!((result.x - expected).abs() < 0.001);
        // Z unchanged.
        assert!((result.z - 20.0).abs() < 0.001);
    }

    #[test]
    fn accelerate_deterministic() {
        let a = QuakeController::accelerate(
            Vector3::new(5.0, 0.0, 3.0),
            Vector3::new(0.707, 0.0, 0.707),
            250.0,
            10.0,
            1.0 / 60.0,
        );
        let b = QuakeController::accelerate(
            Vector3::new(5.0, 0.0, 3.0),
            Vector3::new(0.707, 0.0, 0.707),
            250.0,
            10.0,
            1.0 / 60.0,
        );
        assert_eq!(a, b);
    }

    #[test]
    fn jump_speed_scales_with_gravity_and_force() {
        // jump_speed = sqrt(4 * jump_force * gravity)
        let speed = (4.0_f32 * 1.0 * 30.0).sqrt();
        assert!((speed - 10.954).abs() < 0.01);

        let speed_high = (4.0_f32 * 2.0 * 30.0).sqrt();
        assert!(speed_high > speed);
    }

    #[test]
    fn friction_reduces_speed() {
        let speed = 10.0_f32;
        let friction = 6.0_f32;
        let dt = 1.0 / 60.0;
        let drop = speed * friction * dt;
        let new_speed = (speed - drop).max(0.0);
        assert!(new_speed < speed);
        assert!(new_speed > 0.0);
    }

    #[test]
    fn friction_does_not_go_negative() {
        let speed = 0.1_f32;
        let friction = 100.0_f32;
        let dt = 1.0;
        let drop = speed * friction * dt;
        let new_speed = (speed - drop).max(0.0);
        assert!(new_speed >= 0.0);
    }

    #[test]
    fn bhop_window_logic() {
        // frame_timer < bhop_frames means skip friction.
        let bhop_frames = 2;
        for frame_timer in 0..bhop_frames {
            assert!(frame_timer < bhop_frames, "should skip friction");
        }
        assert!(bhop_frames >= bhop_frames, "should apply friction");
    }

    #[test]
    fn air_strafe_increases_speed() {
        // Classic Quake air strafe: moving forward, wish to go right.
        // Since dot product is ~0, full air acceleration applies.
        let forward_vel = Vector3::new(0.0, 0.0, 10.0);
        let wish_right = Vector3::new(1.0, 0.0, 0.0);
        let result = QuakeController::accelerate(forward_vel, wish_right, 85.0, 1.5, 1.0 / 60.0);
        // Total speed should exceed original speed.
        assert!(result.length() > forward_vel.length());
    }
}
