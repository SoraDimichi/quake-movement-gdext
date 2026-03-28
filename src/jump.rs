/// Jump state machine. Handles ground jump, double jump, and input buffering.
/// No Godot dependencies — pure logic.
pub struct JumpState {
    consumed: bool,
    can_double_jump: bool,
}

impl Default for JumpState {
    fn default() -> Self {
        Self::new()
    }
}

impl JumpState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            consumed: false,
            can_double_jump: false,
        }
    }
}

/// What the jump state decided this frame.
pub enum JumpAction {
    None,
    Jump,
    DoubleJump,
}

impl JumpState {
    /// Call once per frame with current input and floor state.
    /// Returns the jump action to perform.
    pub fn update(&mut self, space_held: bool, on_floor: bool) -> JumpAction {
        let mut action = JumpAction::None;

        if space_held && !self.consumed {
            if on_floor {
                action = JumpAction::Jump;
                self.can_double_jump = true;
            } else if self.can_double_jump {
                action = JumpAction::DoubleJump;
                self.can_double_jump = false;
            }
            self.consumed = true;
        }

        if !space_held {
            self.consumed = false;
        }

        if on_floor && !matches!(action, JumpAction::Jump) {
            self.can_double_jump = false;
        }

        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_jump() {
        let mut state = JumpState::default();
        assert!(matches!(state.update(true, true), JumpAction::Jump));
    }

    #[test]
    fn must_release_between_jumps() {
        let mut state = JumpState::default();
        assert!(matches!(state.update(true, true), JumpAction::Jump));
        // Still holding — no second jump.
        assert!(matches!(state.update(true, true), JumpAction::None));
        // Release.
        assert!(matches!(state.update(false, true), JumpAction::None));
        // Press again.
        assert!(matches!(state.update(true, true), JumpAction::Jump));
    }

    #[test]
    fn double_jump_in_air() {
        let mut state = JumpState::default();
        // Ground jump.
        assert!(matches!(state.update(true, true), JumpAction::Jump));
        // Release in air.
        assert!(matches!(state.update(false, false), JumpAction::None));
        // Press in air — double jump.
        assert!(matches!(state.update(true, false), JumpAction::DoubleJump));
    }

    #[test]
    fn no_triple_jump() {
        let mut state = JumpState::default();
        assert!(matches!(state.update(true, true), JumpAction::Jump));
        state.update(false, false);
        assert!(matches!(state.update(true, false), JumpAction::DoubleJump));
        state.update(false, false);
        // Third press in air — nothing.
        assert!(matches!(state.update(true, false), JumpAction::None));
    }

    #[test]
    fn double_jump_resets_on_ground() {
        let mut state = JumpState::default();
        assert!(matches!(state.update(true, true), JumpAction::Jump));
        state.update(false, false);
        // Land without jumping.
        state.update(false, true);
        // Jump again from ground.
        assert!(matches!(state.update(true, true), JumpAction::Jump));
        state.update(false, false);
        // Double jump available again.
        assert!(matches!(state.update(true, false), JumpAction::DoubleJump));
    }

    #[test]
    fn hold_before_landing_works() {
        let mut state = JumpState::default();
        // First jump.
        assert!(matches!(state.update(true, true), JumpAction::Jump));
        // Release in air.
        state.update(false, false);
        // Hold space before landing (still in air).
        state.update(true, false); // double jump
        state.update(false, false); // release
                                    // Hold before landing.
        assert!(matches!(state.update(true, true), JumpAction::Jump));
    }
}
