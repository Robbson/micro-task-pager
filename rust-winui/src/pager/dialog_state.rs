//! Lifecycle of the pager's secondary About/Settings window.
//!
//! The pager owns at most one secondary window. Reactor does not report a
//! reliable close notification, so this state machine tracks the window from
//! the open request until its HWND disappears:
//!
//! ```text
//! Closed --opened()--> Opening --window_seen()--> Open
//!   ^                     |                         |
//!   +--closed()-----------+---window_missing()------+
//! ```
//!
//! `Opening` knows the pager rectangle the Settings window should be placed
//! against and counts fast placement retries. `Open` only needs the missing
//! counter that clears a stale state after the window vanished without a close
//! event. About windows have no owner rectangle, so they skip placement.

use crate::dialogs::placement::Rect;

/// Ticks a window may be missing before it is considered gone while opening.
const MISSING_TICKS_BEFORE_SEEN: u32 = 25;
/// Ticks a window may be missing after it has been seen at least once.
const MISSING_TICKS_AFTER_SEEN: u32 = 2;
/// Fast placement retries while the Settings HWND has not appeared yet.
const MAX_POSITION_ATTEMPTS: u32 = 100;

/// One of the three dialog lifecycle phases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DialogState {
    /// No secondary window is open.
    #[default]
    Closed,
    /// A window was requested but its HWND has not been observed yet.
    Opening {
        /// Pager rectangle used for the Settings placement, if any.
        owner: Option<Rect>,
        missing_ticks: u32,
        position_attempts: u32,
    },
    /// The window has been observed at least once.
    Open { missing_ticks: u32 },
}

impl DialogState {
    /// A successful open request, before the HWND is observed.
    pub(crate) fn opened(owner: Option<Rect>) -> Self {
        Self::Opening {
            owner,
            missing_ticks: 0,
            position_attempts: 0,
        }
    }

    /// Whether a secondary window is open or still opening.
    pub(crate) fn is_open(self) -> bool {
        !matches!(self, Self::Closed)
    }

    /// The Settings placement owner, available only while `Opening`.
    pub(crate) fn owner(self) -> Option<Rect> {
        match self {
            Self::Opening { owner, .. } => owner,
            Self::Open { .. } | Self::Closed => None,
        }
    }

    /// Records that the dialog HWND was found.
    ///
    /// Returns `true` for the first sighting, which is when the window must be
    /// cloaked, placed, and revealed.
    pub(crate) fn window_seen(&mut self) -> bool {
        match self {
            Self::Opening { .. } => {
                *self = Self::Open { missing_ticks: 0 };
                true
            }
            Self::Open { missing_ticks } => {
                *missing_ticks = 0;
                false
            }
            Self::Closed => false,
        }
    }

    /// Records that the dialog HWND was not found.
    ///
    /// Returns `true` when the state gave up waiting and returned to `Closed`.
    pub(crate) fn window_missing(&mut self) -> bool {
        match self {
            Self::Opening { missing_ticks, .. } => {
                *missing_ticks = missing_ticks.saturating_add(1);
                if *missing_ticks > MISSING_TICKS_BEFORE_SEEN {
                    *self = Self::Closed;
                    true
                } else {
                    false
                }
            }
            Self::Open { missing_ticks } => {
                *missing_ticks = missing_ticks.saturating_add(1);
                if *missing_ticks > MISSING_TICKS_AFTER_SEEN {
                    *self = Self::Closed;
                    true
                } else {
                    false
                }
            }
            Self::Closed => false,
        }
    }

    /// Consumes one fast placement retry.
    ///
    /// Returns `true` when the caller should schedule another retry. Only the
    /// Settings window (`Opening` with an owner) is placed this way.
    pub(crate) fn request_position_retry(&mut self) -> bool {
        match self {
            Self::Opening {
                owner: Some(_),
                position_attempts,
                ..
            } if *position_attempts < MAX_POSITION_ATTEMPTS => {
                *position_attempts += 1;
                true
            }
            Self::Opening { .. } | Self::Open { .. } | Self::Closed => false,
        }
    }

    /// Returns to `Closed`.
    pub(crate) fn closed(&mut self) {
        *self = Self::Closed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> Rect {
        Rect {
            left: 10,
            top: 20,
            right: 210,
            bottom: 70,
        }
    }

    #[test]
    fn hidden_window_is_closed() {
        assert!(!DialogState::Closed.is_open());
        assert_eq!(DialogState::Closed.owner(), None);
    }

    #[test]
    fn opening_reports_the_owner_until_the_window_is_seen() {
        let mut state = DialogState::opened(Some(owner()));

        assert!(state.is_open());
        assert_eq!(state.owner(), Some(owner()));
        assert!(state.window_seen());
        assert_eq!(state.owner(), None);
        assert!(state.is_open());
        assert!(!state.window_seen());
    }

    #[test]
    fn about_has_no_owner_and_no_placement_retries() {
        let mut state = DialogState::opened(None);

        assert!(!state.request_position_retry());
        assert!(state.window_seen());
        assert!(!state.request_position_retry());
    }

    #[test]
    fn missing_window_clears_after_the_opening_timeout() {
        let mut state = DialogState::opened(Some(owner()));

        for _ in 0..MISSING_TICKS_BEFORE_SEEN {
            assert!(!state.window_missing());
            assert!(state.is_open());
        }
        assert!(state.window_missing());
        assert!(!state.is_open());
    }

    #[test]
    fn seen_window_clears_after_the_shorter_timeout() {
        let mut state = DialogState::opened(None);
        assert!(state.window_seen());

        for _ in 0..MISSING_TICKS_AFTER_SEEN {
            assert!(!state.window_missing());
        }
        assert!(state.window_missing());
        assert!(!state.is_open());
    }

    #[test]
    fn a_sighting_resets_the_missing_counter() {
        let mut state = DialogState::opened(None);
        assert!(state.window_seen());
        assert!(!state.window_missing());
        assert!(!state.window_missing());

        assert!(!state.window_seen());
        assert!(!state.window_missing());
        assert!(state.is_open());
    }

    #[test]
    fn placement_retries_stop_at_the_limit() {
        let mut state = DialogState::opened(Some(owner()));

        for _ in 0..MAX_POSITION_ATTEMPTS {
            assert!(state.request_position_retry());
        }
        assert!(!state.request_position_retry());
    }

    #[test]
    fn closed_resets_every_open_flag() {
        let mut state = DialogState::opened(Some(owner()));
        state.closed();

        assert!(!state.is_open());
        assert!(!state.request_position_retry());
        assert!(!state.window_missing());
    }
}
