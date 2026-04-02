# quake-movement-gdext

> **⚠️ No longer maintained.** The architecture has changed and this crate will not be supported going forward. The movement code has been transferred directly into the game.

Quake-style first-person movement for Godot 4, written in Rust with [gdext](https://github.com/godot-rust/gdext).

## Quick start

Add the dependency:

```toml
[dependencies]
quake-movement-gdext = { git = "https://github.com/SoraDimichi/quake-movement-gdext.git" }
```

Force-link the types so gdext registers them with Godot:

```rust
type _QC = quake_movement_gdext::QuakeController;
type _QCam = quake_movement_gdext::QuakeCamera;
```

Build your cdylib crate. The classes appear in the Godot editor.

## Scene setup

```
QuakeController (CharacterBody3D)
  +-- CollisionShape3D "Collision"   # auto-discovered if named "Collision"
  +-- QuakeCamera (Camera3D)         # child reads parent state via call() API
  +-- [your game logic nodes]
```

1. Add a `QuakeController` node as your player root
2. Add a `CollisionShape3D` child named `Collision` with a `CapsuleShape3D`
3. Add a `QuakeCamera` child — it handles mouse look, view bob, strafe roll, landing tilt, FOV scaling
4. Configure via inspector. Set up input actions: `move_forward`, `move_backward`, `move_left`, `move_right`, `jump`, `crouch`

## Camera decoupling

`QuakeCamera` does **not** depend on `QuakeController` at compile time. It reads parent state via Godot's `call()` API. Any node (GDScript, C#, Rust) works as a parent if it exposes these methods:

| Method | Return type |
|--------|-------------|
| `get_horizontal_speed()` | `float` |
| `get_current_velocity()` | `Vector3` |
| `get_is_grounded()` | `bool` |
| `get_is_crouching()` | `bool` |
| `get_just_landed()` | `bool` |

## Signals

`QuakeController` emits:

| Signal | When |
|--------|------|
| `jumped` | Ground jump |
| `double_jumped` | Air jump |
| `landed` | Transitioned from air to floor |
| `crouch_started` | Entered crouch |
| `crouch_ended` | Left crouch (not blocked) |

## Pick-and-mix: use physics functions directly

```rust
use quake_movement_gdext::{accelerate, air_accelerate, apply_friction, jump_velocity};

// Ground
vel = apply_friction(vel, friction, stop_speed, dt);
vel = accelerate(vel, wishdir, ground_accel, max_vel, dt);

// Air
vel = air_accelerate(vel, wishdir, max_vel, air_accel, air_cap, dt);

// Jump (additive, like Quake)
vel.y += jump_velocity(jump_force, gravity);
```

All functions are pure math — no Godot runtime needed, testable with `cargo test`.

## Parameters

### QuakeController

| Parameter | Default | Description |
|-----------|---------|-------------|
| `gravity` | 30.0 | Downward acceleration |
| `ground_accelerate` | 250.0 | Ground acceleration rate |
| `air_accelerate` | 85.0 | Air acceleration rate |
| `max_ground_velocity` | 10.0 | Max ground speed (bhop can exceed this) |
| `air_cap` | 1.5 | Air speed clamp (enables air strafing) |
| `jump_force` | 1.0 | Jump height multiplier |
| `friction` | 6.0 | Ground friction |
| `stop_speed` | 1.5 | Low-speed friction threshold |
| `bhop_increment` | 0.2 | Speed bonus per consecutive jump |
| `bhop_max` | 0.8 | Max bhop speed multiplier |
| `bhop_decay` | 2.0 | Bhop multiplier decay rate on ground |
| `stand_height` | 1.8 | Standing capsule height |
| `crouch_height` | 0.9 | Crouching capsule height |
| `crouch_speed_factor` | 0.5 | Speed multiplier while crouched |
| `double_jump_force` | 0.8 | Double jump height multiplier |
| `double_jump_boost` | 3.0 | Horizontal boost on double jump |

### QuakeCamera

| Parameter | Default | Description |
|-----------|---------|-------------|
| `sensitivity` | (0.3, 0.3) | Mouse sensitivity (pitch, yaw) |
| `base_fov` | 75.0 | Base field of view |
| `max_fov_increase` | 15.0 | Extra FOV at max speed |
| `fov_max_speed` | 15.0 | Speed for full FOV effect |
| `bob_amount` | 0.02 | Walk bob amplitude |
| `bob_cycle` | 0.6 | Bob cycle duration (seconds) |
| `bob_up` | 0.5 | Fraction of cycle spent going up |
| `roll_angle` | 2.0 | Max strafe roll (degrees) |
| `roll_speed` | 10.0 | Side speed for full roll |
| `landing_tilt_degrees` | 2.0 | Camera dip on landing |
| `stand_camera_height` | 1.5 | Camera Y offset standing |
| `crouch_camera_height` | 0.75 | Camera Y offset crouching |

Input action names are configurable exports (default: `move_forward`, `move_backward`, `move_left`, `move_right`, `jump`, `crouch`).

## Testing

```bash
make ci    # fmt + clippy (pedantic+nursery) + 35 tests
make test  # unit tests only
```

## Credits

- **id Software** -- Quake source (GPL)
- **[BirDt/bhop3d](https://github.com/BirDt/bhop3d)** -- GDScript reference
- **[modcommunity/dot-fps-controller](https://github.com/modcommunity/dot-fps-controller)** -- Crouch/friction reference
- **[Flafla2](https://adrianb.io/2015/02/14/bunnyhop.html)** -- Bunny hop writeup

## License

MIT
