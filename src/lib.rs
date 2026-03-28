mod quake_controller;

use godot::prelude::*;

struct QuakeMovementGdext;

#[gdextension]
unsafe impl ExtensionLibrary for QuakeMovementGdext {}
