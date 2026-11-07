//! Micro Task Pager, a WinUI 3 virtual desktop pager.
//!
//! The crate is intentionally split into small modules so the Windows-specific
//! and unsafe code stays away from the application logic:
//!
//! - [`com`] initializes the COM apartment on the UI thread.
//! - [`desktop`] is the safe wrapper around the undocumented virtual desktop
//!   COM interfaces. Start with [`desktop::open`], which returns a
//!   [`desktop::SupervisedDesktop`].
//! - [`dialogs`] contains the WinUI About and Settings windows.
//! - [`logging`] writes a small daily log file.
//! - [`native_window`] applies pager-specific Win32 window properties that
//!   `windows-reactor` does not expose.
//! - [`pager`] is the WinUI 3 pager component and the application UI.
//! - [`settings`] reads the C#-compatible registry values and provides defaults.
//! - [`single_instance`] prevents two pagers from running at the same time.
//!
//! The binary and the probe example share this library, so both exercise the
//! same code path. The probe is not a separate test framework; it is a small
//! command-line program built from `examples/probe/main.rs`.

pub mod com;
pub mod desktop;
pub mod dialogs;
pub mod foreground;
pub mod logging;
pub mod monitors;
pub mod native_window;
pub mod pager;
pub mod settings;
pub mod single_instance;
pub mod windows;
