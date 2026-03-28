//! First-person camera with Quake-style effects.
//!
//! Expects to be a child of [`crate::movement::QuakeController`].
//! Reads parent state (velocity, grounded, crouching) each frame — never modifies movement.

use crate::movement::QuakeController;
use crate::quake_physics;
use godot::classes::{
    Camera3D, ICamera3D, Input, InputEvent, InputEventMouseButton, InputEventMouseMotion,
};
use godot::prelude::*;

/// First-person camera with mouse look and Quake-style visual effects.
///
/// Place as a child of [`QuakeController`]. Effects include FOV scaling with speed,
/// head bob, landing tilt, and smooth crouch camera height transitions.
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

    // -- Head Bob --
    /// Whether head bob is enabled.
    #[export]
    #[init(val = true)]
    head_bob_enabled: bool,

    /// Head bob amplitude in units.
    #[export]
    #[init(val = 0.04)]
    head_bob_amplitude: f32,

    /// Head bob frequency (full cycles per second).
    #[export]
    #[init(val = 12.0)]
    head_bob_frequency: f32,

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
    bob_timer: f32,

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
        let ticks = godot::classes::Engine::singleton().get_physics_ticks_per_second();
        let dt = 1.0 / f32::from(i16::try_from(ticks).unwrap_or(60));

        // Read parent controller state.
        let (speed, grounded, crouching, just_landed) =
            self.get_controller()
                .map_or((0.0, true, false, false), |controller| {
                    let ctrl = controller.bind();
                    (
                        ctrl.horizontal_speed(),
                        ctrl.is_grounded(),
                        ctrl.crouching(),
                        ctrl.just_landed(),
                    )
                });

        self.update_fov(speed, dt);
        self.update_head_bob(speed, grounded, dt);
        self.update_landing_tilt(just_landed, dt);
        self.update_crouch_height(crouching, dt);
        self.apply_camera_transform();
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

    fn update_head_bob(&mut self, speed: f32, grounded: bool, dt: f32) {
        if !self.head_bob_enabled || !grounded || speed < 0.5 {
            self.bob_timer = 0.0;
            return;
        }
        self.bob_timer += dt * self.head_bob_frequency;
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

    fn apply_camera_transform(&mut self) {
        // Head bob Y offset.
        let bob_offset = if self.head_bob_enabled {
            (self.bob_timer * std::f32::consts::TAU).sin() * self.head_bob_amplitude
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

        // Pitch = mouse pitch + landing tilt.
        let pitch = self.mouse_pitch + self.current_landing_tilt;
        let mut rot = self.base().get_rotation();
        rot.x = pitch;
        self.base_mut().set_rotation(rot);
    }
}
