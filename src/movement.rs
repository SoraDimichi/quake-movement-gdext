//! Quake/Source-style movement controller for `CharacterBody3D`.
//!
//! Handles ground/air acceleration, friction, bunny hopping, crouch, and coyote time.
//! Does NOT handle camera — see [`crate::camera::QuakeCamera`] for that.

use crate::quake_physics;
use godot::classes::{
    CapsuleShape3D, CharacterBody3D, CollisionShape3D, Engine, ICharacterBody3D, Input,
};
use godot::prelude::*;

/// Quake/Source-style first-person movement controller.
///
/// Port of [bhop3d](https://github.com/BirDt/bhop3d) by `BirDt` with additions from
/// [dot-fps-controller](https://github.com/modcommunity/dot-fps-controller).
/// Based on [Flafla2's bunnyhopping writeup](https://adrianb.io/2015/02/14/bunnyhop.html).
#[derive(GodotClass)]
#[class(init, base=CharacterBody3D)]
pub struct QuakeController {
    // -- Activity Controls --
    /// Whether the character can move.
    #[export]
    #[init(val = true)]
    move_enabled: bool,

    /// Whether holding jump continuously bunny hops.
    #[export]
    #[init(val = false)]
    jump_when_held: bool,

    // -- Input Action Names --
    #[export]
    #[init(val = GString::from("move_forward"))]
    move_forward_action: GString,

    #[export]
    #[init(val = GString::from("move_backward"))]
    move_backward_action: GString,

    #[export]
    #[init(val = GString::from("move_left"))]
    move_left_action: GString,

    #[export]
    #[init(val = GString::from("move_right"))]
    move_right_action: GString,

    #[export]
    #[init(val = GString::from("jump"))]
    jump_action: GString,

    #[export]
    #[init(val = GString::from("crouch"))]
    crouch_action: GString,

    // -- Movement Parameters --
    /// Gravity in units per second squared.
    #[export]
    #[init(val = 30.0)]
    gravity: f32,

    /// Acceleration when grounded.
    #[export]
    #[init(val = 250.0)]
    ground_accelerate: f32,

    /// Acceleration when in the air.
    #[export]
    #[init(val = 85.0)]
    air_accelerate: f32,

    /// Max velocity on the ground.
    #[export]
    #[init(val = 10.0)]
    max_ground_velocity: f32,

    /// Max velocity in the air (controls air strafing tightness).
    #[export]
    #[init(val = 1.5)]
    max_air_velocity: f32,

    /// Jump force multiplier.
    #[export]
    #[init(val = 1.0)]
    jump_force: f32,

    /// Ground friction.
    #[export]
    #[init(val = 6.0)]
    friction: f32,

    /// Minimum control value for friction. Prevents infinite sliding at low speeds.
    #[export]
    #[init(val = 1.5)]
    stop_speed: f32,

    /// Bunny hop window in frames (friction skipped during this window after landing).
    #[export]
    #[init(val = 2)]
    bhop_frames: i32,

    /// When true, bhop window uses air acceleration instead of ground.
    #[export]
    #[init(val = true)]
    additive_bhop: bool,

    // -- Crouch Parameters --
    /// Standing collision capsule height.
    #[export]
    #[init(val = 1.8)]
    stand_height: f32,

    /// Crouching collision capsule height.
    #[export]
    #[init(val = 0.9)]
    crouch_height: f32,

    /// Speed multiplier while crouching.
    #[export]
    #[init(val = 0.5)]
    crouch_speed_factor: f32,

    /// Crouch height transition speed (lerp rate per second).
    #[export]
    #[init(val = 10.0)]
    crouch_lerp_speed: f32,

    // -- Coyote Time --
    /// Frames after leaving an edge where jump is still allowed.
    #[export]
    #[init(val = 5)]
    coyote_frames: i32,

    // -- Collision Shape Reference --
    /// `CollisionShape3D` to resize during crouch (must use `CapsuleShape3D`).
    #[export]
    collision_shape: Option<Gd<CollisionShape3D>>,

    // -- Internal State --
    #[init(val = 0)]
    frame_timer: i32,

    #[init(val = 0)]
    coyote_timer: i32,

    #[init(val = false)]
    is_crouching: bool,

    #[init(val = 1.8)]
    current_height: f32,

    #[init(val = false)]
    was_on_floor: bool,

    #[init(val = false)]
    just_landed_flag: bool,

    base: Base<CharacterBody3D>,
}

#[godot_api]
impl ICharacterBody3D for QuakeController {
    fn physics_process(&mut self, _delta: f64) {
        let ticks = Engine::singleton().get_physics_ticks_per_second();
        let dt = 1.0 / f32::from(i16::try_from(ticks).unwrap_or(60));

        let on_floor = self.base().is_on_floor();

        // Detect landing.
        self.just_landed_flag = !self.was_on_floor && on_floor;

        // Update bhop frame timer.
        if on_floor {
            self.frame_timer += 1;
        } else {
            self.frame_timer = 0;
        }

        // Update coyote timer.
        if on_floor {
            self.coyote_timer = self.coyote_frames;
        } else if self.coyote_timer > 0 {
            self.coyote_timer -= 1;
        }

        // Crouch input.
        let input = Input::singleton();
        let crouch_name = StringName::from(&self.crouch_action);
        self.is_crouching = input.is_action_pressed(&crouch_name);

        // Lerp collision height.
        self.update_collision_height(dt);

        // Compute velocity.
        let velocity = self.compute_velocity(dt, on_floor, &input);
        self.base_mut().set_velocity(velocity);
        self.base_mut().move_and_slide();

        self.was_on_floor = on_floor;
    }

    fn ready(&mut self) {
        self.current_height = self.stand_height;
    }
}

// -- State getters (pub for QuakeCamera, #[func] for GDScript) --
#[godot_api]
impl QuakeController {
    /// Backward-compatible static acceleration function.
    /// Delegates to [`quake_physics::accelerate`].
    #[must_use]
    pub fn accelerate(
        prev_velocity: Vector3,
        accel_dir: Vector3,
        acceleration: f32,
        max_vel: f32,
        dt: f32,
    ) -> Vector3 {
        quake_physics::accelerate(prev_velocity, accel_dir, acceleration, max_vel, dt)
    }

    #[func]
    #[must_use]
    pub fn get_horizontal_speed(&self) -> f32 {
        let vel = self.base().get_velocity();
        Vector3::new(vel.x, 0.0, vel.z).length()
    }

    #[func]
    #[must_use]
    pub fn get_is_grounded(&self) -> bool {
        self.base().is_on_floor()
    }

    #[func]
    #[must_use]
    pub fn get_is_crouching(&self) -> bool {
        self.is_crouching
    }

    #[func]
    #[must_use]
    pub fn get_just_landed(&self) -> bool {
        self.just_landed_flag
    }
}

// -- Public Rust-only getters (for QuakeCamera) --
impl QuakeController {
    #[must_use]
    pub fn horizontal_speed(&self) -> f32 {
        let vel = self.base().get_velocity();
        Vector3::new(vel.x, 0.0, vel.z).length()
    }

    #[must_use]
    pub fn is_grounded(&self) -> bool {
        self.base().is_on_floor()
    }

    #[must_use]
    pub const fn crouching(&self) -> bool {
        self.is_crouching
    }

    #[must_use]
    pub const fn just_landed(&self) -> bool {
        self.just_landed_flag
    }

    #[must_use]
    pub fn current_velocity(&self) -> Vector3 {
        self.base().get_velocity()
    }
}

// -- Private implementation --
impl QuakeController {
    fn compute_velocity(&self, dt: f32, on_floor: bool, input: &Gd<Input>) -> Vector3 {
        let mut vel = self.base().get_velocity();
        let can_jump = on_floor || self.coyote_timer > 0;

        // Apply friction if grounded and past the bhop window.
        let use_ground_params = if on_floor && self.frame_timer >= self.bhop_frames {
            vel = quake_physics::apply_friction(vel, self.friction, self.stop_speed, dt);
            true
        } else {
            // During bhop window: use air params if additive_bhop is on.
            !self.additive_bhop
        };

        // Determine params (apply crouch speed factor if crouching and grounded).
        let mut max_vel = if use_ground_params {
            self.max_ground_velocity
        } else {
            self.max_air_velocity
        };
        if self.is_crouching && on_floor {
            max_vel *= self.crouch_speed_factor;
        }
        let accel = if use_ground_params {
            self.ground_accelerate
        } else {
            self.air_accelerate
        };

        // Wishdir from input.
        let wishdir = self.get_wishdir(input);

        // Accelerate.
        vel = quake_physics::accelerate(vel, wishdir, accel, max_vel, dt);

        // Gravity.
        vel += Vector3::DOWN * self.gravity * dt;

        // Jump.
        let jump_name = StringName::from(&self.jump_action);
        let jump_pressed = if self.jump_when_held {
            input.is_action_pressed(&jump_name)
        } else {
            input.is_action_just_pressed(&jump_name)
        };
        if jump_pressed && self.move_enabled && can_jump {
            vel.y = quake_physics::jump_velocity(self.jump_force, self.gravity);
        }

        vel
    }

    fn get_wishdir(&self, input: &Gd<Input>) -> Vector3 {
        if !self.move_enabled {
            return Vector3::ZERO;
        }
        let basis = self.base().get_transform().basis;
        let fwd = StringName::from(&self.move_forward_action);
        let back = StringName::from(&self.move_backward_action);
        let left = StringName::from(&self.move_left_action);
        let right = StringName::from(&self.move_right_action);

        let forward_axis = input.get_axis(&fwd, &back);
        let side_axis = input.get_axis(&left, &right);

        basis.col_c() * forward_axis + basis.col_a() * side_axis
    }

    fn update_collision_height(&mut self, dt: f32) {
        let target = if self.is_crouching {
            self.crouch_height
        } else {
            self.stand_height
        };
        self.current_height =
            quake_physics::lerp_f32(self.current_height, target, self.crouch_lerp_speed * dt);

        let Some(ref mut shape_node) = self.collision_shape else {
            return;
        };
        let Some(shape_res) = shape_node.get_shape() else {
            return;
        };
        if let Ok(mut capsule) = shape_res.try_cast::<CapsuleShape3D>() {
            capsule.set_height(self.current_height);
        }
    }
}
