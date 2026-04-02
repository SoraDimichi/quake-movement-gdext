//! First-person camera with Quake-style effects.
//!
//! Expects to be a child of [`crate::movement::QuakeController`].
//! Reads parent state (velocity, grounded, crouching) each frame — never modifies movement.
//!
//! Camera effects ported from Quake's `view.c`:
//! - `V_CalcBob` — asymmetric walk bob proportional to speed
//! - `V_CalcRoll` — strafe roll proportional to side velocity
//! - Landing tilt and crouch camera lerp (modern additions)
//! - FOV scaling with speed (modern addition)

use crate::movement::QuakeController;
use crate::quake_physics;
use godot::classes::{
    Camera3D, ICamera3D, Input, InputEvent, InputEventMouseButton, InputEventMouseMotion,
};
use godot::prelude::*;

/// First-person camera with mouse look and Quake-style visual effects.
///
/// Place as a child of [`QuakeController`].
#[derive(GodotClass)]
#[class(init, base=Camera3D)]
pub struct QuakeCamera {
    // -- Mouse Look --
    /// Whether the camera responds to mouse input.
    #[export]
    #[init(val = true)]
    look_enabled: bool,

    /// Mouse sensitivity (x = pitch, y = yaw).
    #[export]
    #[init(val = Vector2::new(0.3, 0.3))]
    sensitivity: Vector2,

    // -- FOV Scaling --
    /// Whether FOV changes with speed.
    #[export]
    #[init(val = true)]
    fov_scaling_enabled: bool,

    /// Base FOV in degrees.
    #[export]
    #[init(val = 75.0)]
    base_fov: f32,

    /// Additional FOV degrees at max display speed.
    #[export]
    #[init(val = 15.0)]
    max_fov_increase: f32,

    /// Speed at which max FOV increase is reached.
    #[export]
    #[init(val = 15.0)]
    fov_max_speed: f32,

    /// FOV interpolation speed (lerp rate per second).
    #[export]
    #[init(val = 8.0)]
    fov_lerp_speed: f32,

    // -- View Bob (Quake V_CalcBob) --
    /// Whether view bob is enabled.
    #[export]
    #[init(val = true)]
    bob_enabled: bool,

    /// Bob amplitude scale (Quake `cl_bob`, default 0.02).
    #[export]
    #[init(val = 0.02)]
    bob_amount: f32,

    /// Full bob cycle duration in seconds (Quake `cl_bobcycle`, default 0.6).
    #[export]
    #[init(val = 0.6)]
    bob_cycle: f32,

    /// Fraction of cycle spent going up (Quake `cl_bobup`, default 0.5).
    #[export]
    #[init(val = 0.5)]
    bob_up: f32,

    // -- Strafe Roll (Quake V_CalcRoll) --
    /// Whether strafe roll is enabled.
    #[export]
    #[init(val = true)]
    roll_enabled: bool,

    /// Max roll angle in degrees (Quake `cl_rollangle`, default 2.0).
    #[export]
    #[init(val = 2.0)]
    roll_angle: f32,

    /// Side speed at which max roll is reached (Quake `cl_rollspeed`, default 10.0).
    #[export]
    #[init(val = 10.0)]
    roll_speed: f32,

    // -- Landing Tilt --
    /// Whether landing camera dip is enabled.
    #[export]
    #[init(val = true)]
    landing_tilt_enabled: bool,

    /// Landing tilt angle in degrees.
    #[export]
    #[init(val = 2.0)]
    landing_tilt_degrees: f32,

    /// Landing tilt recovery speed (lerp rate per second).
    #[export]
    #[init(val = 6.0)]
    landing_tilt_recovery: f32,

    // -- Crouch Camera --
    /// Standing camera Y offset from parent origin.
    #[export]
    #[init(val = 1.5)]
    stand_camera_height: f32,

    /// Crouching camera Y offset from parent origin.
    #[export]
    #[init(val = 0.75)]
    crouch_camera_height: f32,

    /// Camera height lerp speed.
    #[export]
    #[init(val = 10.0)]
    crouch_camera_lerp_speed: f32,

    // -- Internal State --
    #[init(val = 75.0)]
    current_fov: f32,

    #[init(val = 0.0)]
    elapsed_time: f32,

    #[init(val = 0.0)]
    current_landing_tilt: f32,

    #[init(val = 1.5)]
    current_camera_y: f32,

    #[init(val = 0.0)]
    mouse_pitch: f32,

    base: Base<Camera3D>,
}

#[godot_api]
impl ICamera3D for QuakeCamera {
    fn ready(&mut self) {
        self.current_fov = self.base_fov;
        self.current_camera_y = self.stand_camera_height;
        Input::singleton().set_mouse_mode(godot::classes::input::MouseMode::CAPTURED);
    }

    fn input(&mut self, event: Gd<InputEvent>) {
        // Capture mouse on click.
        if event.clone().try_cast::<InputEventMouseButton>().is_ok() {
            Input::singleton().set_mouse_mode(godot::classes::input::MouseMode::CAPTURED);
        }

        if !self.look_enabled {
            return;
        }
        if Input::singleton().get_mouse_mode() != godot::classes::input::MouseMode::CAPTURED {
            return;
        }
        let Ok(motion) = event.try_cast::<InputEventMouseMotion>() else {
            return;
        };

        let relative = motion.get_relative();
        let sens = self.sensitivity;

        // Yaw: rotate parent body.
        let yaw = (-relative.x * sens.y).to_radians();
        if let Some(parent) = self.base().get_parent() {
            if let Ok(mut node3d) = parent.try_cast::<Node3D>() {
                node3d.rotate_y(yaw);
            }
        }

        // Pitch: track separately to prevent landing tilt drift.
        self.mouse_pitch += (-relative.y * sens.x).to_radians();
        self.mouse_pitch = self
            .mouse_pitch
            .clamp((-89.0_f32).to_radians(), 89.0_f32.to_radians());
    }

    fn process(&mut self, _delta: f64) {
        let dt = crate::util::physics_dt();
        self.elapsed_time += dt;

        // Read parent controller state.
        let (speed, velocity, grounded, crouching, just_landed) =
            self.get_controller()
                .map_or((0.0, Vector3::ZERO, true, false, false), |controller| {
                    let ctrl = controller.bind();
                    (
                        ctrl.horizontal_speed(),
                        ctrl.current_velocity(),
                        ctrl.is_grounded(),
                        ctrl.crouching(),
                        ctrl.just_landed(),
                    )
                });

        self.update_fov(speed, dt);
        self.update_landing_tilt(just_landed, dt);
        self.update_crouch_height(crouching, dt);
        self.apply_camera_transform(speed, velocity, grounded);
    }
}

impl QuakeCamera {
    fn get_controller(&self) -> Option<Gd<QuakeController>> {
        self.base().get_parent()?.try_cast::<QuakeController>().ok()
    }

    fn update_fov(&mut self, speed: f32, dt: f32) {
        if !self.fov_scaling_enabled {
            return;
        }
        let t = (speed / self.fov_max_speed).clamp(0.0, 1.0);
        let target_fov = t.mul_add(self.max_fov_increase, self.base_fov);
        self.current_fov =
            quake_physics::lerp_f32(self.current_fov, target_fov, self.fov_lerp_speed * dt);
    }

    fn update_landing_tilt(&mut self, just_landed: bool, dt: f32) {
        if !self.landing_tilt_enabled {
            return;
        }
        if just_landed {
            self.current_landing_tilt = self.landing_tilt_degrees.to_radians();
        }
        self.current_landing_tilt = quake_physics::lerp_f32(
            self.current_landing_tilt,
            0.0,
            self.landing_tilt_recovery * dt,
        );
    }

    fn update_crouch_height(&mut self, crouching: bool, dt: f32) {
        let target = if crouching {
            self.crouch_camera_height
        } else {
            self.stand_camera_height
        };
        self.current_camera_y = quake_physics::lerp_f32(
            self.current_camera_y,
            target,
            self.crouch_camera_lerp_speed * dt,
        );
    }

    fn apply_camera_transform(&mut self, speed: f32, velocity: Vector3, grounded: bool) {
        // Quake V_CalcBob — asymmetric walk bob proportional to speed.
        let bob_offset = if self.bob_enabled && grounded {
            quake_physics::calc_bob(
                self.elapsed_time,
                speed,
                self.bob_amount,
                self.bob_cycle,
                self.bob_up,
            )
        } else {
            0.0
        };

        // Quake V_CalcRoll — strafe roll.
        let roll = if self.roll_enabled {
            let right = self.base().get_global_transform().basis.col_a();
            quake_physics::calc_roll(velocity, right, self.roll_angle, self.roll_speed)
        } else {
            0.0
        };

        // Camera position (height + bob).
        let mut pos = self.base().get_position();
        pos.y = self.current_camera_y + bob_offset;
        self.base_mut().set_position(pos);

        // FOV.
        let fov = self.current_fov;
        self.base_mut().set_fov(fov);

        // Rotation: pitch (mouse + landing tilt) + roll (strafe).
        let pitch = self.mouse_pitch + self.current_landing_tilt;
        let mut rot = self.base().get_rotation();
        rot.x = pitch;
        rot.z = roll.to_radians();
        self.base_mut().set_rotation(rot);
    }
}
