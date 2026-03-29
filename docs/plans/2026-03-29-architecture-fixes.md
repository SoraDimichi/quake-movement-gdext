# Architecture Fixes: Smooth Godot Integration

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make quake-movement-gdext a smoothly integrable Godot module — no custom instance coupling, clean signals, no dead code.

**Architecture:** The camera must read parent state via Godot's `call()` API (any `CharacterBody3D` exposing the right `#[func]` methods works), not via Rust `try_cast` to a concrete type. The controller emits signals for game-relevant events. Dead code and duplicate getters are removed.

**Tech Stack:** Rust + gdext (godot-rust), Godot 4.6

---

### Task 1: Remove dead `StringName` cache fields and cache properly in `ready()`

`movement.rs` declares 6 `sn_*` fields that are never populated or used. Meanwhile `get_wishdir()` and `physics_process()` create `StringName::from(&GString)` every physics frame.

**Files:**
- Modify: `src/movement.rs`

**Step 1: Remove unused `sn_*` fields and populate/use cached StringNames**

In `movement.rs`, the fields `sn_jump`, `sn_crouch`, `sn_fwd`, `sn_back`, `sn_left`, `sn_right` exist but are never written in `ready()` and never read anywhere. Fix by:

1. Populate them in `ready()`:
```rust
fn ready(&mut self) {
    if self.collision_shape.is_none() {
        self.collision_shape = self.base().try_get_node_as::<CollisionShape3D>("Collision");
    }
    self.sn_fwd = StringName::from(&self.move_forward_action);
    self.sn_back = StringName::from(&self.move_backward_action);
    self.sn_left = StringName::from(&self.move_left_action);
    self.sn_right = StringName::from(&self.move_right_action);
    self.sn_jump = StringName::from(&self.jump_action);
    self.sn_crouch = StringName::from(&self.crouch_action);
}
```

2. Use cached `sn_*` in `physics_process()` and `get_wishdir()`:
```rust
// physics_process:
let wants_crouch = input.is_action_pressed(&self.sn_crouch);

// get_wishdir:
let forward_axis = input.get_axis(&self.sn_fwd, &self.sn_back);
let side_axis = input.get_axis(&self.sn_left, &self.sn_right);

// compute_velocity:
let space_held = input.is_action_pressed(&self.sn_jump);
```

**Step 2: Run `make ci`**

Run: `make ci`
Expected: All checks pass (fmt, lint, test)

**Step 3: Commit**

```bash
git add src/movement.rs
git commit -m "fix: populate and use cached StringName fields in ready()"
```

---

### Task 2: Remove duplicate Rust-only getters — unify on `#[func]` getters

`movement.rs` has duplicate getter pairs: Rust-only (`horizontal_speed()`, `is_grounded()`, etc.) and `#[func]` (`get_horizontal_speed()`, `get_is_grounded()`, etc.) doing the same computation independently.

**Files:**
- Modify: `src/movement.rs`
- Modify: `src/camera.rs` (update calls to use unified getters)

**Step 1: Remove the Rust-only `impl QuakeController` block (lines 232-263)**

Delete the entire block:
```rust
// -- Rust-only getters (for QuakeCamera) --
impl QuakeController { ... }
```

These methods are: `horizontal_speed()`, `is_grounded()`, `crouching()`, `just_landed()`, `current_velocity()`, `bhop_multiplier()`.

**Step 2: Add the missing `current_velocity` as a `#[func]` getter**

In the `#[godot_api]` impl block, add:
```rust
#[func]
#[must_use]
pub fn get_current_velocity(&self) -> Vector3 {
    self.base().get_velocity()
}
```

**Step 3: Update `camera.rs` to use the `#[func]` getters**

In `camera.rs`, the `get_controller()` call returns `Gd<QuakeController>` and calls `.bind()` to access Rust-only methods. After removing those methods, update camera to call via Godot API (this prepares for Task 3 decoupling):

Replace the tuple extraction in `process()`:
```rust
let ctrl = controller.bind();
(
    ctrl.horizontal_speed(),
    ctrl.current_velocity(),
    ctrl.is_grounded(),
    ctrl.crouching(),
    ctrl.just_landed(),
)
```
With:
```rust
let ctrl = controller.bind();
(
    ctrl.get_horizontal_speed(),
    ctrl.get_current_velocity(),
    ctrl.get_is_grounded(),
    ctrl.get_is_crouching(),
    ctrl.get_just_landed(),
)
```

**Step 4: Run `make ci`**

Run: `make ci`
Expected: All checks pass

**Step 5: Commit**

```bash
git add src/movement.rs src/camera.rs
git commit -m "refactor: unify duplicate getters into single #[func] API"
```

---

### Task 3: Decouple camera from `QuakeController` — use Godot `call()` API

This is the biggest integrability fix. `QuakeCamera` currently does `get_parent()?.try_cast::<QuakeController>()` which means it ONLY works as a child of `QuakeController`. Any `CharacterBody3D` exposing the right methods should work.

**Files:**
- Modify: `src/camera.rs`

**Step 1: Replace `get_controller()` with generic parent reading via `call()`**

Remove:
```rust
use crate::movement::QuakeController;

fn get_controller(&self) -> Option<Gd<QuakeController>> {
    self.base().get_parent()?.try_cast::<QuakeController>().ok()
}
```

Replace the state reading in `process()` with Godot `call()` on the parent node. The camera expects the parent to be any `CharacterBody3D` that has these methods: `get_horizontal_speed() -> f32`, `get_current_velocity() -> Vector3`, `get_is_grounded() -> bool`, `get_is_crouching() -> bool`, `get_just_landed() -> bool`.

New helper:
```rust
fn read_parent_state(&self) -> (f32, Vector3, bool, bool, bool) {
    let Some(parent) = self.base().get_parent() else {
        return (0.0, Vector3::ZERO, true, false, false);
    };

    let speed = f32::try_from_variant(&parent.call("get_horizontal_speed", &[]))
        .unwrap_or(0.0);
    let velocity = Vector3::try_from_variant(&parent.call("get_current_velocity", &[]))
        .unwrap_or(Vector3::ZERO);
    let grounded = bool::try_from_variant(&parent.call("get_is_grounded", &[]))
        .unwrap_or(true);
    let crouching = bool::try_from_variant(&parent.call("get_is_crouching", &[]))
        .unwrap_or(false);
    let just_landed = bool::try_from_variant(&parent.call("get_just_landed", &[]))
        .unwrap_or(false);

    (speed, velocity, grounded, crouching, just_landed)
}
```

Update `process()` to call `self.read_parent_state()` instead of `self.get_controller().map_or(...)`.

Also remove `use crate::movement::QuakeController;` from camera.rs imports.

**Step 2: Run `make ci`**

Run: `make ci`
Expected: All checks pass

**Step 3: Commit**

```bash
git add src/camera.rs
git commit -m "refactor: decouple camera from QuakeController — use Godot call() API"
```

---

### Task 4: Add signals to `QuakeController` for game-relevant events

The controller should emit signals for `jumped`, `landed`, `crouch_started`, `crouch_ended` so game code can react without polling.

**Files:**
- Modify: `src/movement.rs`

**Step 1: Add signal definitions**

In the `#[godot_api]` impl block, add:
```rust
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
```

**Step 2: Emit signals at the right points**

In `physics_process()`:
- After detecting `just_landed_flag = true`: emit `landed`
- After `duck()`: emit `crouch_started`
- After successful `try_unduck()` (when `is_crouching` becomes false): emit `crouch_ended`

In `compute_velocity()`:
- After `JumpAction::Jump`: emit `jumped`
- After `JumpAction::DoubleJump`: emit `double_jumped`

Since `compute_velocity` takes `&mut self`, signals can be emitted there. However, signal emission needs `self.signals()` which requires the godot_api context. Move signal emission to `physics_process()` after `compute_velocity()` returns, by returning the `JumpAction` from `compute_velocity()`:

```rust
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
        let was_crouching = self.is_crouching;
        self.try_unduck();
        if was_crouching && !self.is_crouching {
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
```

Update `compute_velocity` to return `(Vector3, JumpAction)` instead of just `Vector3`.

**Step 3: Run `make ci`**

Run: `make ci`
Expected: All checks pass

**Step 4: Commit**

```bash
git add src/movement.rs
git commit -m "feat: add signals for jumped, double_jumped, landed, crouch_started, crouch_ended"
```

---

### Task 5: Remove unnecessary static method wrapper

`movement.rs` has `QuakeController::accelerate()` — a public static method that just delegates to `quake_physics::accelerate()`. It's not `#[func]`, so GDScript can't call it. It's dead code.

**Files:**
- Modify: `src/movement.rs`

**Step 1: Remove the method**

Delete:
```rust
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
```

**Step 2: Run `make ci`**

Run: `make ci`
Expected: All checks pass

**Step 3: Commit**

```bash
git add src/movement.rs
git commit -m "refactor: remove dead QuakeController::accelerate() wrapper"
```

---

### Task 6: Update `lib.rs` public API — export `get_current_velocity` rename

**Files:**
- Modify: `src/lib.rs`

**Step 1: Verify exports are correct**

The public API in `lib.rs` should still export:
- Classes: `QuakeCamera`, `QuakeController`, `JumpAction`, `JumpState`
- Functions: `accelerate`, `air_accelerate`, `apply_friction`, `calc_bob`, `calc_roll`, `jump_velocity`, `lerp_f32`, `physics_dt`

No changes needed here since the removed Rust-only methods were not re-exported.

**Step 2: Run full `make ci`**

Run: `make ci`
Expected: All checks pass

**Step 3: Commit (if any changes)**

Skip if no changes needed.

---

### Task 7: Update consumer project tests

After the changes, the consumer project (`first-game`) should still work. Key things to verify:

**Files:**
- Modify: `first-game/test/gut/test_player.gd` — add signal tests for new signals
- Modify: `first-game/rust/src/player.rs` — no changes needed (uses `quake_movement_gdext::accelerate()` directly, not the removed wrapper)

**Step 1: Add signal presence tests in test_player.gd**

```gdscript
func test_player_has_jumped_signal():
    assert_has_signal(_player, "jumped")

func test_player_has_landed_signal():
    assert_has_signal(_player, "landed")

func test_player_has_crouch_started_signal():
    assert_has_signal(_player, "crouch_started")

func test_player_has_crouch_ended_signal():
    assert_has_signal(_player, "crouch_ended")
```

**Step 2: Run consumer CI**

Run: `make ci` (from first-game directory)
Expected: All checks pass

**Step 3: Commit**

```bash
git add test/gut/test_player.gd
git commit -m "test: add signal presence tests for new QuakeController signals"
```

---

## Execution Order

Tasks 1-5 are in the `quake-movement-gdext` repo.
Task 7 is in the `first-game` repo.
Task 6 is verification only.

Tasks 1 and 2 are independent and can be parallelized.
Task 3 depends on Task 2 (needs unified `#[func]` getters).
Task 4 is independent of 1-3.
Task 5 is independent.
Task 7 depends on Tasks 4 (new signals to test).

## What was NOT changed (and why)

- **`quake_physics` depending on `godot::prelude::Vector3`**: Pragmatic choice for a Godot-specific crate. The target audience always has gdext.
- **Feature flags**: Premature for a crate this size. The cost of compiling 2 extra Godot classes is negligible.
- **`process_mode`**: Game-specific choice — some games want paused controllers, others don't. Not the module's decision.
- **CapsuleShape3D assumption in crouch**: This is the standard Godot FPS shape. Supporting box/cylinder adds complexity for a non-existent use case.
