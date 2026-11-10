//! Pure dialog helpers shared by every front end.
//!
//! These modules do no Win32 or UI work. They are plain Rust with unit tests,
//! so the winsafe and WinUI dialogs can share the same behavior:
//!
//! - [`placement`] decides where a secondary dialog goes relative to its owner.
//! - [`settings_draft`] is the editable working copy behind the Settings dialog.

pub mod placement;
pub mod settings_draft;
