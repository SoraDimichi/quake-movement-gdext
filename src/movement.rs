//! Quake-style movement controller for `CharacterBody3D`.
//!
//! Handles ground/air acceleration, friction, jump, crouch, and DUSK-style bhop.
//! Crouch uses Half-Life 1 `PM_Duck`/`PM_UnDuck` pattern: instant hull switch.
//! Does NOT handle camera — see [`crate::camera::QuakeCamera`].

use crate::jump::{JumpAction, JumpState};
use crate::quake_physics;
use godot::classes::{CapsuleShape3D, CharacterBody3D, CollisionShape3D, ICharacterBody3D, Input};
use godot::prelude::*;

/// Quake-style first-person movement controller.
///
/// Movement: Quake `SV_Accelerate`/`SV_AirAccelerate`/`SV_UserFriction`.
/// Bhop: DUSK-style per-jump speed multiplier.
/// Crouch: Half-Life 1 instant hull switch with `test_move` anti-stuck.
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
    #[export]
    #[init(val = 30.0)]
    gravity: f32,

    #[export]
    #[init(val = 250.0)]
    ground_accelerate: f32,

    #[export]
    #[init(val = 85.0)]
    air_accelerate: f32,

    #[export]
    #[init(val = 10.0)]
    max_ground_velocity: f32,

    #[export]
    #[init(val = 1.5)]
    air_cap: f32,

    #[export]
    #[init(val = 1.0)]
    jump_force: f32,

    #[export]
    #[init(val = 6.0)]
    friction: f32,

    #[export]
    #[init(val = 1.5)]
    stop_speed: f32,

    // -- Bhop --
    #[export]
    #[init(val = 0.2)]
    bhop_increment: f32,

    #[export]
    #[init(val = 0.8)]
    bhop_max: f32,

    #[export]
    #[init(val = 2.0)]
    bhop_decay: f32,

    // -- Crouch (Half-Life 1 instant switch) --
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

    // -- Double Jump --
    /// Double jump vertical force multiplier (relative to normal jump).
    #[export]
    #[init(val = 0.8)]
    double_jump_force: f32,

    /// Horizontal speed boost on double jump.
    #[export]
    #[init(val = 3.0)]
    double_jump_boost: f32,

    // -- Internal State --
    #[init(val = false)]
    is_crouching: bool,

    #[init(val = false)]
    was_on_floor: bool,

    #[init(val = false)]
    just_landed_flag: bool,

    #[init(val = JumpState::new())]
    jump_state: JumpState,

    #[init(val = 0.0)]
    bhop_multiplier_val: f32,

    base: Base<CharacterBody3D>,
}

#[godot_api]
impl ICharacterBody3D for QuakeController {
    fn physics_process(&mut self, delta: f64) {
        let dt = delta as f32;

        let on_floor = self.base().is_on_floor();
        self.just_landed_flag = !self.was_on_floor && on_floor;

        // Half-Life 1 crouch: instant hull switch.
        let input = Input::singleton();
        let crouch_name = StringName::from(&self.crouch_action);
        let wants_crouch = input.is_action_pressed(&crouch_name);
        if wants_crouch && !self.is_crouching {
            self.duck();
        } else if !wants_crouch && self.is_crouching {
            self.try_unduck();
        }

        let velocity = self.compute_velocity(dt, on_floor, &input);
        self.base_mut().set_velocity(velocity);
        self.base_mut().move_and_slide();

        self.was_on_floor = on_floor;
    }
}

#[godot_api]
impl QuakeController {
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
    /// Instantly switch to crouch hull (Half-Life `PM_Duck`).
    fn duck(&mut self) {
        self.is_crouching = true;
        self.set_hull_height(self.crouch_height);
    }

    /// Try to uncrouch. If standing hull overlaps, stay ducked (Half-Life `PM_UnDuck`).
    fn try_unduck(&mut self) {
        let prev = self.get_hull_height();
        self.set_hull_height(self.stand_height);

        let transform = self.base().get_global_transform();
        if self.base_mut().test_move(transform, Vector3::ZERO) {
            self.set_hull_height(prev);
        } else {
            self.is_crouching = false;
        }
    }

    fn set_hull_height(&self, height: f32) {
        let Some(shape_node) = self.base().try_get_node_as::<CollisionShape3D>("Collision") else {
            return;
        };
        let Some(shape_res) = shape_node.get_shape() else {
            return;
        };
        if let Ok(mut capsule) = shape_res.try_cast::<CapsuleShape3D>() {
            capsule.set_height(height);
        }
    }

    fn get_hull_height(&self) -> f32 {
        self.base()
            .try_get_node_as::<CollisionShape3D>("Collision")
            .and_then(|n| n.get_shape())
            .and_then(|s| s.try_cast::<CapsuleShape3D>().ok())
            .map_or(self.stand_height, |c| c.get_height())
    }

    fn compute_velocity(&mut self, dt: f32, on_floor: bool, input: &Gd<Input>) -> Vector3 {
        let mut vel = self.base().get_velocity();
        let wishdir = self.get_wishdir(input);

        let jump_name = StringName::from(&self.jump_action);
        let space_held = input.is_action_pressed(&jump_name);
        let jump_action = self.jump_state.update(space_held, on_floor);
        let jumping = matches!(jump_action, JumpAction::Jump);

        if jumping {
            self.bhop_multiplier_val =
                (self.bhop_multiplier_val + self.bhop_increment).min(self.bhop_max);
        } else if on_floor {
            self.bhop_multiplier_val = self
                .bhop_decay
                .mul_add(-dt, self.bhop_multiplier_val)
                .max(0.0);
        }

        let mut max_vel = self.max_ground_velocity * (1.0 + self.bhop_multiplier_val);
        if self.is_crouching && on_floor {
            max_vel *= self.crouch_speed_factor;
        }

        if on_floor && !jumping {
            vel = quake_physics::apply_friction(vel, self.friction, self.stop_speed, dt);
            vel = quake_physics::accelerate(vel, wishdir, self.ground_accelerate, max_vel, dt);
        } else if on_floor {
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

        match jump_action {
            JumpAction::Jump => {
                vel.y += quake_physics::jump_velocity(self.jump_force, self.gravity);
            }
            JumpAction::DoubleJump => {
                vel.y = quake_physics::jump_velocity(
                    self.jump_force * self.double_jump_force,
                    self.gravity,
                );
                if wishdir.length() > 0.1 {
                    let boost = wishdir.normalized() * self.double_jump_boost;
                    vel.x += boost.x;
                    vel.z += boost.z;
                }
            }
            JumpAction::None => {}
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
}
