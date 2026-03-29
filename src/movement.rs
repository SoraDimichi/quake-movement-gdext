//! Quake-style movement controller for `CharacterBody3D`.
//!
//! Crouch uses Half-Life 1 `PM_Duck`/`PM_UnDuck` pattern: instant hull switch.
//! Jump uses `JumpState` for double jump support.
//! Bhop uses DUSK-style per-jump speed multiplier.

use crate::jump::{JumpAction, JumpState};
use crate::quake_physics;
use godot::classes::{CapsuleShape3D, CharacterBody3D, CollisionShape3D, ICharacterBody3D, Input};
use godot::prelude::*;

/// Quake-style first-person movement controller.
#[derive(GodotClass)]
#[class(init, base=CharacterBody3D)]
pub struct QuakeController {
    #[export]
    #[init(val = true)]
    move_enabled: bool,

    // -- Input Actions (GString for editor, cached as StringName at runtime) --
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

    // -- Movement --
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

    // -- Crouch --
    #[export]
    #[init(val = 1.8)]
    stand_height: f32,

    #[export]
    #[init(val = 0.9)]
    crouch_height: f32,

    #[export]
    #[init(val = 0.5)]
    crouch_speed_factor: f32,

    /// Collision shape to resize on crouch. Wire in editor.
    #[export]
    collision_shape: Option<Gd<CollisionShape3D>>,

    // -- Double Jump --
    #[export]
    #[init(val = 0.8)]
    double_jump_force: f32,

    #[export]
    #[init(val = 3.0)]
    double_jump_boost: f32,

    // -- Internal --
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

    // Cached StringNames (populated in ready).
    #[init(val = StringName::default())]
    sn_jump: StringName,

    #[init(val = StringName::default())]
    sn_crouch: StringName,

    #[init(val = StringName::default())]
    sn_fwd: StringName,

    #[init(val = StringName::default())]
    sn_back: StringName,

    #[init(val = StringName::default())]
    sn_left: StringName,

    #[init(val = StringName::default())]
    sn_right: StringName,

    base: Base<CharacterBody3D>,
}

#[godot_api]
impl ICharacterBody3D for QuakeController {
    fn ready(&mut self) {
        if self.collision_shape.is_none() {
            self.collision_shape = self.base().try_get_node_as::<CollisionShape3D>("Collision");
        }
        // Cache StringNames from GString exports.
        self.sn_fwd = StringName::from(&self.move_forward_action);
        self.sn_back = StringName::from(&self.move_backward_action);
        self.sn_left = StringName::from(&self.move_left_action);
        self.sn_right = StringName::from(&self.move_right_action);
        self.sn_jump = StringName::from(&self.jump_action);
        self.sn_crouch = StringName::from(&self.crouch_action);
    }

    fn physics_process(&mut self, delta: f64) {
        let dt = delta as f32;
        let on_floor = self.base().is_on_floor();
        self.just_landed_flag = !self.was_on_floor && on_floor;

        if self.just_landed_flag {
            self.signals().landed().emit();
        }

        let input = Input::singleton();
        let wants_crouch = input.is_action_pressed(&self.sn_crouch);
        if wants_crouch && !self.is_crouching {
            self.duck();
            self.signals().crouch_started().emit();
        } else if !wants_crouch && self.is_crouching {
            self.try_unduck();
            if !self.is_crouching {
                self.signals().crouch_ended().emit();
            }
        }

        let (velocity, jump_action) = self.compute_velocity(dt, on_floor, &input);
        self.base_mut().set_velocity(velocity);
        self.base_mut().move_and_slide();

        match jump_action {
            JumpAction::Jump => self.signals().jumped().emit(),
            JumpAction::DoubleJump => self.signals().double_jumped().emit(),
            JumpAction::None => {}
        }

        self.was_on_floor = on_floor;
    }
}

// -- Public API --
#[godot_api]
impl QuakeController {
    #[signal]
    fn jumped();

    #[signal]
    fn double_jumped();

    #[signal]
    fn landed();

    #[signal]
    fn crouch_started();

    #[signal]
    fn crouch_ended();

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

    #[func]
    #[must_use]
    pub fn get_current_velocity(&self) -> Vector3 {
        self.base().get_velocity()
    }
}

// -- Private implementation --
impl QuakeController {
    fn duck(&mut self) {
        self.is_crouching = true;
        self.set_hull_height(self.crouch_height);
    }

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
        let Some(ref shape_node) = self.collision_shape else {
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
        self.collision_shape
            .as_ref()
            .and_then(|n| n.get_shape())
            .and_then(|s| s.try_cast::<CapsuleShape3D>().ok())
            .map_or(self.stand_height, |c| c.get_height())
    }

    fn compute_velocity(
        &mut self,
        dt: f32,
        on_floor: bool,
        input: &Gd<Input>,
    ) -> (Vector3, JumpAction) {
        let mut vel = self.base().get_velocity();
        let wishdir = self.get_wishdir(input);

        let space_held = input.is_action_pressed(&self.sn_jump);
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

        (vel, jump_action)
    }

    fn get_wishdir(&self, input: &Gd<Input>) -> Vector3 {
        if !self.move_enabled {
            return Vector3::ZERO;
        }
        let basis = self.base().get_transform().basis;
        let forward_axis = input.get_axis(&self.sn_fwd, &self.sn_back);
        let side_axis = input.get_axis(&self.sn_left, &self.sn_right);

        basis.col_c() * forward_axis + basis.col_a() * side_axis
    }
}
