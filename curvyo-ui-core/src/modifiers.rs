//! The Shift, Ctrl and Alt state of one input event, as the host reports it.

/// Which of Shift, Ctrl and Alt are down for an event (Ctrl is Cmd on macOS;
/// the host folds it in). A tool reads the state of the event it is handling,
/// not a remembered one, so a result is a pure function of the pointer
/// position and these flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Shift is down.
    pub shift: bool,
    /// Ctrl (or Cmd) is down.
    pub ctrl: bool,
    /// Alt (or Option) is down. Only the Select tool reads it
    /// (`specs/advanced-selection/`): the shape tools ignore it.
    pub alt: bool,
}

impl Modifiers {
    /// No key is down.
    pub const NONE: Self = Self {
        shift: false,
        ctrl: false,
        alt: false,
    };

    /// The given state of Shift and Ctrl, Alt up.
    #[must_use]
    pub const fn new(shift: bool, ctrl: bool) -> Self {
        Self {
            shift,
            ctrl,
            alt: false,
        }
    }

    /// This state with Alt set as given.
    #[must_use]
    pub const fn with_alt(self, alt: bool) -> Self {
        Self { alt, ..self }
    }
}
