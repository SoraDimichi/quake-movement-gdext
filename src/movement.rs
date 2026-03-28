//! Quake-style movement controller for `CharacterBody3D`.
//!
//! Handles ground/air acceleration, friction, jump, crouch, and DUSK-style bhop.
//! Does NOT handle camera — see [`crate::camera::QuakeCamera`].

use crate::quake_physics;
use godot::classes::{
    CapsuleShape3D, CharacterBody3D, CollisionShape3D, Engine, ICharacterBody3D, Input,
};
use godot::prelude::*;

/// Quake-style first-person movement controller with DUSK-style bhop.
///
/// Movement physics from Quake's `SV_Accelerate`, `SV_AirAccelerate`, `SV_UserFriction`.
/// Bhop system inspired by DUSK: consecutive jumps build a speed multiplier.
#[derive(GodotClass)]
#[class(init, base=CharacterBody3D)]
pub struct QuakeController {
    /// Whether the character can move.
    #[export]
    #[init(val = true)]
    move_enabled: bool,

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

    /// Ground acceleration.
    #[export]
    #[init(val = 250.0)]
    ground_accelerate: f32,

    /// Air acceleration.
    #[export]
    #[init(val = 85.0)]
    air_accelerate: f32,

    /// Max ground velocity.
    #[export]
    #[init(val = 10.0)]
    max_ground_velocity: f32,

    /// Air speed cap for the addspeed check.
    #[export]
    #[init(val = 1.5)]
    air_cap: f32,

    /// Jump force (height in units).
    #[export]
    #[init(val = 1.0)]
    jump_force: f32,

    /// Ground friction.
    #[export]
    #[init(val = 6.0)]
    friction: f32,

    /// Minimum control value for friction.
    #[export]
    #[init(val = 1.5)]
    stop_speed: f32,

    // -- Bhop (DUSK-style) --
    /// Speed multiplier gained per consecutive jump.
    #[export]
    #[init(val = 0.2)]
    bhop_increment: f32,

    /// Max bhop multiplier (1.0 + this = max speed factor).
    #[export]
    #[init(val = 0.8)]
    bhop_max: f32,

    /// How fast the multiplier decays on ground (per second).
    #[export]
    #[init(val = 2.0)]
    bhop_decay: f32,

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

    /// Crouch height transition speed.
    #[export]
    #[init(val = 10.0)]
    crouch_lerp_speed: f32,

    // -- Collision Shape Reference --
    #[export]
    collision_shape: Option<Gd<CollisionShape3D>>,

    // -- Internal State --
    #[init(val = false)]
    is_crouching: bool,

    #[init(val = 1.8)]
    current_height: f32,

    #[init(val = false)]
    was_on_floor: bool,

    #[init(val = false)]
    just_landed_flag: bool,

    #[init(val = false)]
    jump_consumed: bool,

    #[init(val = 0.0)]
    bhop_multiplier_val: f32,

    base: Base<CharacterBody3D>,
}

#[godot_api]
impl ICharacterBody3D for QuakeController {
    fn physics_process(&mut self, _delta: f64) {
        let ticks = Engine::singleton().get_physics_ticks_per_second();
        let dt = 1.0 / f32::from(i16::try_from(ticks).unwrap_or(60));

        let on_floor = self.base().is_on_floor();
        self.just_landed_flag = !self.was_on_floor && on_floor;

        // Crouch.
        let input = Input::singleton();
        let crouch_name = StringName::from(&self.crouch_action);
        self.is_crouching = input.is_action_pressed(&crouch_name);
        self.update_collision_height(dt);

        // Velocity.
        let velocity = self.compute_velocity(dt, on_floor, &input);
        self.base_mut().set_velocity(velocity);
        self.base_mut().move_and_slide();

        self.was_on_floor = on_floor;
    }

    fn ready(&mut self) {
        self.current_height = self.stand_height;
    }
}

#[godot_api]
impl QuakeController {
    /// Ground acceleration (delegates to [`quake_physics::accelerate`]).
    #[must_use]
    pub fn accelerate(
        prev_velocity: Vector3,
        accel_dir: Vector3,
        accel: f32,
        max_vel: f32,
        dt: f32,
    ) -> Vector3 {
        quake_physics::accelerate(prev_velocity, accel_dir, accel, max_vel, dt)
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

    #[func]
    #[must_use]
    pub fn get_bhop_multiplier(&self) -> f32 {
        self.bhop_multiplier_val
    }
}

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

    #[must_use]
    pub const fn bhop_multiplier(&self) -> f32 {
        self.bhop_multiplier_val
    }
}

impl QuakeController {
    fn compute_velocity(&mut self, dt: f32, on_floor: bool, input: &Gd<Input>) -> Vector3 {
        let mut vel = self.base().get_velocity();
        let wishdir = self.get_wishdir(input);

        // Jump: press to jump, must release between jumps (no auto-hop).
        // Hold space before landing — fires on touchdown.
        let jump_name = StringName::from(&self.jump_action);
        let space_held = input.is_action_pressed(&jump_name);
        let jumping = if on_floor && space_held && !self.jump_consumed && self.move_enabled {
            self.jump_consumed = true;
            true
        } else {
            if !space_held {
                self.jump_consumed = false;
            }
            false
        };

        // Bhop multiplier: grows on jump, decays on ground.
        if jumping {
            self.bhop_multiplier_val =
                (self.bhop_multiplier_val + self.bhop_increment).min(self.bhop_max);
        } else if on_floor {
            self.bhop_multiplier_val = self
                .bhop_decay
                .mul_add(-dt, self.bhop_multiplier_val)
                .max(0.0);
        }

        // Max velocity with bhop and crouch applied.
        let mut max_vel = self.max_ground_velocity * (1.0 + self.bhop_multiplier_val);
        if self.is_crouching && on_floor {
            max_vel *= self.crouch_speed_factor;
        }

        if on_floor && !jumping {
            vel = quake_physics::apply_friction(vel, self.friction, self.stop_speed, dt);
            vel = quake_physics::accelerate(vel, wishdir, self.ground_accelerate, max_vel, dt);
        } else if on_floor {
            // Jumping frame: skip friction (bhop).
            vel = quake_physics::accelerate(vel, wishdir, self.ground_accelerate, max_vel, dt);
        } else {
            vel = quake_physics::air_accelerate(
                vel,
                wishdir,
                max_vel,
                self.air_accelerate,
                self.air_cap,
                dt,
            );
        }

        vel += Vector3::DOWN * self.gravity * dt;

        if jumping {
            vel.y += quake_physics::jump_velocity(self.jump_force, self.gravity);
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
