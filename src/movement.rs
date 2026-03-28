//! Quake-style movement controller for `CharacterBody3D`.
//!
//! Faithful to id Software's Quake source (`sv_user.c`).
//! Handles ground/air acceleration, friction, jump, and crouch.
//! Does NOT handle camera — see [`crate::camera::QuakeCamera`].

use crate::quake_physics;
use godot::classes::{
    CapsuleShape3D, CharacterBody3D, CollisionShape3D, Engine, ICharacterBody3D, Input,
};
use godot::prelude::*;

/// Quake-style first-person movement controller.
///
/// Faithful port of Quake's `SV_Accelerate`, `SV_AirAccelerate`, and `SV_UserFriction`.
#[derive(GodotClass)]
#[class(init, base=CharacterBody3D)]
pub struct QuakeController {
    /// Whether the character can move.
    #[export]
    #[init(val = true)]
    move_enabled: bool,

    /// Whether holding jump continuously hops.
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

    /// Ground acceleration (Quake `sv_accelerate`, default 10).
    #[export]
    #[init(val = 10.0)]
    ground_accelerate: f32,

    /// Air acceleration (Quake `sv_accelerate` used in `SV_AirAccelerate`).
    #[export]
    #[init(val = 10.0)]
    air_accelerate: f32,

    /// Max ground speed (Quake `sv_maxspeed`).
    #[export]
    #[init(val = 10.0)]
    max_speed: f32,

    /// Air speed cap for the addspeed check (Quake hardcodes 30, scaled for our units).
    #[export]
    #[init(val = 1.5)]
    air_cap: f32,

    /// Jump force (height in units the jump should reach).
    #[export]
    #[init(val = 1.2)]
    jump_force: f32,

    /// Ground friction (Quake `sv_friction`, default 4).
    #[export]
    #[init(val = 4.0)]
    friction: f32,

    /// Minimum control value for friction. Prevents infinite sliding.
    #[export]
    #[init(val = 1.5)]
    stop_speed: f32,

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

    // -- Collision Shape Reference --
    /// `CollisionShape3D` to resize during crouch (must use `CapsuleShape3D`).
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

    base: Base<CharacterBody3D>,
}

#[godot_api]
impl ICharacterBody3D for QuakeController {
    fn physics_process(&mut self, _delta: f64) {
        let ticks = Engine::singleton().get_physics_ticks_per_second();
        let dt = 1.0 / f32::from(i16::try_from(ticks).unwrap_or(60));

        let on_floor = self.base().is_on_floor();
        self.just_landed_flag = !self.was_on_floor && on_floor;

        // Crouch input.
        let input = Input::singleton();
        let crouch_name = StringName::from(&self.crouch_action);
        self.is_crouching = input.is_action_pressed(&crouch_name);
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

#[godot_api]
impl QuakeController {
    /// Ground acceleration (delegates to [`quake_physics::accelerate`]).
    #[must_use]
    pub fn accelerate(
        prev_velocity: Vector3,
        wish_dir: Vector3,
        wish_speed: f32,
        accel: f32,
        dt: f32,
    ) -> Vector3 {
        quake_physics::accelerate(prev_velocity, wish_dir, wish_speed, accel, dt)
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

impl QuakeController {
    fn compute_velocity(&self, dt: f32, on_floor: bool, input: &Gd<Input>) -> Vector3 {
        let mut vel = self.base().get_velocity();

        let wishdir = self.get_wishdir(input);
        let mut wish_speed = self.max_speed;
        if self.is_crouching && on_floor {
            wish_speed *= self.crouch_speed_factor;
        }

        // Jump check BEFORE friction (Quake execution order — enables bhop).
        // If jumping this frame, skip friction so speed is preserved.
        let jump_name = StringName::from(&self.jump_action);
        let jump_pressed = if self.jump_when_held {
            input.is_action_pressed(&jump_name)
        } else {
            input.is_action_just_pressed(&jump_name)
        };
        let jumping = jump_pressed && self.move_enabled && on_floor;

        if on_floor && !jumping {
            // Ground: friction then accelerate.
            vel = quake_physics::apply_friction(vel, self.friction, self.stop_speed, dt);
            vel = quake_physics::accelerate(vel, wishdir, wish_speed, self.ground_accelerate, dt);
        } else if on_floor {
            // Jumping frame: accelerate but skip friction (bhop).
            vel = quake_physics::accelerate(vel, wishdir, wish_speed, self.ground_accelerate, dt);
        } else {
            // Air: air accelerate (no friction).
            vel = quake_physics::air_accelerate(
                vel,
                wishdir,
                wish_speed,
                self.air_accelerate,
                self.air_cap,
                dt,
            );
        }

        // Gravity.
        vel += Vector3::DOWN * self.gravity * dt;

        // Jump (additive, like Quake).
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
