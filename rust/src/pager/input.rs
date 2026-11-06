//! Pure pointer/drag state transitions.
//!
//! Reactor reports pointer events with window-relative DIPs and pointer capture.
//! This module turns those events into pager actions without touching Windows
//! APIs, so the state machine can be unit-tested.
//!
//! - Middle drag or Shift+left drag moves the pager window.
//! - Left drag on a window cell starts a window drag after a small threshold.
//! - A release without a drag is a click: switch desktop or focus a window.

use windows::Win32::Foundation::HWND;

use super::layout::{Hit, PagerLayout};

/// Movement in DIPs before a window press becomes a drag.
pub const DRAG_THRESHOLD_DIPS: f64 = 4.0;

/// The active drag, if any.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drag {
    Pager {
        start_cursor: (i32, i32),
        start_origin: (i32, i32),
    },
    Window {
        hwnd: HWND,
        origin_desktop: u32,
        start_x: f64,
        start_y: f64,
        active: bool,
    },
}

/// The icon ghost shown while a window is being dragged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ghost {
    pub hwnd: HWND,
    pub x: f64,
    pub y: f64,
}

/// What the release should do after the state machine is cleared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseAction {
    None,
    Click { desktop: u32, hwnd: Option<HWND> },
    MoveWindow { hwnd: HWND, target_desktop: u32 },
}

/// Pointer state owned by the pager component.
#[derive(Default)]
pub struct InputState {
    drag: Option<Drag>,
    ghost: Option<Ghost>,
    right_press: bool,
}

/// A pointer press to feed into [`InputState::press`].
#[derive(Clone, Copy, Debug)]
pub struct PressInput {
    pub hit: Option<Hit>,
    pub x: f64,
    pub y: f64,
    pub middle: bool,
    pub shift: bool,
    pub cursor: Option<(i32, i32)>,
    pub origin: Option<(i32, i32)>,
}

impl InputState {
    /// Starts a gesture on a pointer press.
    pub fn press(&mut self, press: PressInput) {
        self.right_press = false;
        self.ghost = None;

        if press.middle || press.shift {
            self.drag = match (press.cursor, press.origin) {
                (Some(start_cursor), Some(start_origin)) => Some(Drag::Pager {
                    start_cursor,
                    start_origin,
                }),
                _ => None,
            };
            return;
        }

        self.drag = press
            .hit
            .and_then(|hit| hit.hwnd.map(|hwnd| (hit.desktop_index, hwnd)))
            .map(|(origin_desktop, hwnd)| Drag::Window {
                hwnd,
                origin_desktop,
                start_x: press.x,
                start_y: press.y,
                active: false,
            });
    }

    /// Records a right press so the following release does not count as a click.
    pub fn right_press(&mut self) {
        self.drag = None;
        self.ghost = None;
        self.right_press = true;
    }

    /// Updates an active window drag and returns `true` once it is active.
    pub fn pointer_moved(&mut self, x: f64, y: f64) -> bool {
        let Some(Drag::Window {
            hwnd,
            origin_desktop: _,
            start_x,
            start_y,
            active,
        }) = self.drag.as_mut()
        else {
            return false;
        };

        if !*active {
            let dx = x - *start_x;
            let dy = y - *start_y;
            if dx * dx + dy * dy < DRAG_THRESHOLD_DIPS * DRAG_THRESHOLD_DIPS {
                return false;
            }
            *active = true;
        }

        self.ghost = Some(Ghost { hwnd: *hwnd, x, y });
        true
    }

    /// Finishes a gesture and clears the drag/ghost state.
    pub fn release(&mut self, layout: &PagerLayout, x: f64, _y: f64) -> ReleaseAction {
        if self.right_press {
            self.right_press = false;
            self.drag = None;
            self.ghost = None;
            return ReleaseAction::None;
        }

        let drag = self.drag.take();
        self.ghost = None;

        match drag {
            Some(Drag::Pager { .. }) => ReleaseAction::None,
            Some(Drag::Window {
                hwnd,
                origin_desktop,
                active,
                ..
            }) => {
                if active {
                    match layout.desktop_at(x) {
                        Some(target_desktop) if target_desktop != origin_desktop => {
                            ReleaseAction::MoveWindow {
                                hwnd,
                                target_desktop,
                            }
                        }
                        _ => ReleaseAction::None,
                    }
                } else {
                    click(layout, x, Some(hwnd))
                }
            }
            None => click(layout, x, None),
        }
    }

    /// Cancels a gesture without producing a click or a move.
    pub fn cancel(&mut self) {
        self.drag = None;
        self.ghost = None;
        self.right_press = false;
    }

    pub fn drag(&self) -> Option<Drag> {
        self.drag
    }

    pub fn ghost(&self) -> Option<Ghost> {
        self.ghost
    }
}

fn click(layout: &PagerLayout, x: f64, hwnd: Option<HWND>) -> ReleaseAction {
    match layout.desktop_at(x) {
        Some(desktop) => ReleaseAction::Click { desktop, hwnd },
        None => ReleaseAction::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::c_void;

    use windows::Win32::Foundation::HWND;

    fn hwnd() -> HWND {
        HWND(std::ptr::dangling_mut::<c_void>())
    }

    fn layout() -> PagerLayout {
        PagerLayout {
            width: 200.0,
            height: 40.0,
            desktop_width: 100.0,
            desktop_count: 2,
            desktops: Vec::new(),
        }
    }

    #[test]
    fn middle_press_starts_pager_drag() {
        let mut input = InputState::default();
        input.press(PressInput {
            hit: None,
            x: 10.0,
            y: 10.0,
            middle: true,
            shift: false,
            cursor: Some((1, 2)),
            origin: Some((3, 4)),
        });

        assert_eq!(
            input.drag(),
            Some(Drag::Pager {
                start_cursor: (1, 2),
                start_origin: (3, 4)
            })
        );
    }

    #[test]
    fn shift_left_press_starts_pager_drag() {
        let mut input = InputState::default();
        input.press(PressInput {
            hit: None,
            x: 10.0,
            y: 10.0,
            middle: false,
            shift: true,
            cursor: Some((1, 2)),
            origin: Some((3, 4)),
        });

        assert!(matches!(input.drag(), Some(Drag::Pager { .. })));
    }

    fn press_window(input: &mut InputState) {
        input.press(PressInput {
            hit: Some(Hit {
                desktop_index: 0,
                screen_index: 0,
                hwnd: Some(hwnd()),
            }),
            x: 10.0,
            y: 10.0,
            middle: false,
            shift: false,
            cursor: None,
            origin: None,
        });
    }

    #[test]
    fn window_press_below_threshold_is_a_click() {
        let mut input = InputState::default();
        press_window(&mut input);
        let action = input.release(&layout(), 10.0, 10.0);

        assert_eq!(
            action,
            ReleaseAction::Click {
                desktop: 0,
                hwnd: Some(hwnd())
            }
        );
    }

    #[test]
    fn window_drag_beyond_threshold_shows_ghost() {
        let mut input = InputState::default();
        press_window(&mut input);

        assert!(!input.pointer_moved(12.0, 12.0));
        assert!(input.ghost().is_none());
        assert!(input.pointer_moved(20.0, 20.0));
        assert_eq!(
            input.ghost(),
            Some(Ghost {
                hwnd: hwnd(),
                x: 20.0,
                y: 20.0
            })
        );
    }

    #[test]
    fn window_drag_to_another_desktop_moves_the_window() {
        let mut input = InputState::default();
        press_window(&mut input);
        assert!(input.pointer_moved(150.0, 20.0));

        let action = input.release(&layout(), 150.0, 20.0);

        assert_eq!(
            action,
            ReleaseAction::MoveWindow {
                hwnd: hwnd(),
                target_desktop: 1
            }
        );
    }

    #[test]
    fn right_release_is_not_a_click() {
        let mut input = InputState::default();
        input.right_press();

        assert_eq!(input.release(&layout(), 10.0, 10.0), ReleaseAction::None);
    }
}
