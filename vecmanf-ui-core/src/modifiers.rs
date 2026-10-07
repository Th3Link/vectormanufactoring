//! The Shift and Ctrl state of one input event, as the host reports it.

/// Which of Shift and Ctrl are down for an event (Ctrl is Cmd on macOS; the
/// host folds it in). A tool reads the state of the event it is handling, not
/// a remembered one, so a result is a pure function of the pointer position
/// and these two flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Shift is down.
    pub shift: bool,
    /// Ctrl (or Cmd) is down.
    pub ctrl: bool,
}

impl Modifiers {
    /// Neither key is down.
    pub const NONE: Self = Self {
        shift: false,
        ctrl: false,
    };

    /// The given state of the two keys.
    #[must_use]
    pub const fn new(shift: bool, ctrl: bool) -> Self {
        Self { shift, ctrl }
    }
}
