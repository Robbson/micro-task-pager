//! Micro Task Pager, a native Windows virtual desktop pager.
//!
//! The crate is intentionally split into small modules so the Windows-specific
//! and unsafe code stays away from the application logic:
//!
//! - [`com`] initializes the COM apartment on the UI thread.
//! - [`desktop`] is the safe wrapper around the undocumented virtual desktop
//!   COM interfaces. Start with [`desktop::open`], which returns a
//!   [`desktop::SupervisedDesktop`].
//! - [`logging`] writes a small daily log file.
//! - [`settings`] reads the C#-compatible registry values and provides defaults.
//! - [`single_instance`] prevents two pagers from running at the same time.
//!
//! The binary and the probe example share this library, so both exercise the
//! same code path. The probe is not a separate test framework; it is a small
//! command-line program built from `examples/probe/main.rs`.

pub mod com;
pub mod desktop;
pub mod logging;
pub mod settings;
pub mod single_instance;
