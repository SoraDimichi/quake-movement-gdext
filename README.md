# quake-movement-gdext

Quake-style first-person movement controller for Godot 4, written in Rust using [gdext](https://github.com/godot-rust/gdext).

Faithful port of id Software's Quake movement physics and camera effects.

## What it ports

| Quake source | Our module | Function |
|-------------|-----------|----------|
| `SV_Accelerate` (`sv_user.c`) | `quake_physics::accelerate` | Ground acceleration with `wishspeed` multiplier |
| `SV_AirAccelerate` (`sv_user.c`) | `quake_physics::air_accelerate` | Air acceleration with `air_cap` clamp (enables bunny hopping + air strafing) |
| `SV_UserFriction` (`sv_user.c`) | `quake_physics::apply_friction` | Ground friction with `stop_speed` threshold |
| `V_CalcBob` (`view.c`) | `quake_physics::calc_bob` | Asymmetric walk bob proportional to speed |
| `V_CalcRoll` (`view.c`) | `quake_physics::calc_roll` | Strafe roll proportional to side velocity |

## Architecture

```
src/
  quake_physics.rs  — Pure math. No Godot runtime needed for tests.
  movement.rs       — QuakeController (CharacterBody3D). Uses quake_physics.
  camera.rs         — QuakeCamera (Camera3D). Reads QuakeController state.
```

Data flow is one-way: `quake_physics` <- `movement` <- `camera` reads state.

```
QuakeController (CharacterBody3D)
  └── QuakeCamera (Camera3D)
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
quake-movement-gdext = { git = "https://github.com/SoraDimichi/quake-movement-gdext.git" }
```

### Option A: Use QuakeController + QuakeCamera directly

Place a `QuakeController` node in your scene with a `QuakeCamera` child. Configure via the Godot inspector.

### Option B: Use physics functions in your own controller

```rust
use quake_movement_gdext::{accelerate, air_accelerate, apply_friction, jump_velocity};

// Ground movement
vel = apply_friction(vel, friction, stop_speed, dt);
vel = accelerate(vel, wishdir, wish_speed, ground_accel, dt);

// Air movement
vel = air_accelerate(vel, wishdir, wish_speed, air_accel, air_cap, dt);

// Jump (additive, like Quake)
vel.y += jump_velocity(jump_force, gravity);
```

## Parameters

### QuakeController (movement.rs)

| Parameter | Default | Quake equivalent |
|-----------|---------|-----------------|
| `gravity` | 30.0 | `sv_gravity` |
| `ground_accelerate` | 10.0 | `sv_accelerate` |
| `air_accelerate` | 10.0 | `sv_accelerate` (in `SV_AirAccelerate`) |
| `max_speed` | 10.0 | `sv_maxspeed` |
| `air_cap` | 1.5 | Hardcoded 30 in Quake (scaled) |
| `jump_force` | 1.2 | Jump height in units |
| `friction` | 4.0 | `sv_friction` |
| `stop_speed` | 1.5 | `sv_stopspeed` |
| `stand_height` | 1.8 | Collision capsule height |
| `crouch_height` | 0.9 | Crouched capsule height |
| `crouch_speed_factor` | 0.5 | Speed multiplier while crouched |

### QuakeCamera (camera.rs)

| Parameter | Default | Quake equivalent |
|-----------|---------|-----------------|
| `sensitivity` | (0.3, 0.3) | Mouse sensitivity (pitch, yaw) |
| `bob_amount` | 0.02 | `cl_bob` |
| `bob_cycle` | 0.6 | `cl_bobcycle` |
| `bob_up` | 0.5 | `cl_bobup` |
| `roll_angle` | 2.0 | `cl_rollangle` |
| `roll_speed` | 10.0 | `cl_rollspeed` (scaled) |
| `base_fov` | 75.0 | Base field of view |
| `max_fov_increase` | 15.0 | FOV increase at max speed |
| `landing_tilt_degrees` | 2.0 | Camera dip on landing |
| `stand_camera_height` | 1.5 | Camera Y offset standing |
| `crouch_camera_height` | 0.75 | Camera Y offset crouched |

## Testing

```bash
make ci    # fmt-check + clippy + tests
make test  # unit tests only (24 tests, pure math)
```

## Credits

- **id Software** — Original Quake source code (GPL)
- **[BirDt/bhop3d](https://github.com/BirDt/bhop3d)** — Initial GDScript reference
- **[modcommunity/dot-fps-controller](https://github.com/modcommunity/dot-fps-controller)** — Crouch and friction reference
- **[Flafla2](https://adrianb.io/2015/02/14/bunnyhop.html)** — Bunny hopping technical writeup

## License

MIT
