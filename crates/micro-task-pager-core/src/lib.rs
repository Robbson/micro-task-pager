//! Windows platform core shared by every Rust pager front end.
//!
//! This crate contains everything that is independent of the UI toolkit:
//! COM bootstrap, the virtual-desktop API, window enumeration, settings,
//! monitor and foreground tracking, logging, the single-instance guard, and
//! the pure dialog helpers (placement and the settings working copy).
//!
//! It deliberately knows nothing about `winsafe`, `windows-reactor`, GDI, or
//! WinUI. A new front end adds this crate as a path dependency and supplies
//! its own rendering, input handling, and dialogs.
//!
//! # Reading order
//!
//! 1. [`com`] initializes the single-threaded apartment. Keep the guard alive
//!    for as long as any desktop object is used.
//! 2. [`desktop`] is the safe virtual-desktop API. [`desktop::open`] returns a
//!    supervisor that recreates the COM backend after a shell restart.
//! 3. [`windows`] enumerates top-level windows into a [`windows::WindowSnapshot`].
//! 4. [`settings`] reads and writes the registry format shared with the C#
//!    implementation.
//! 5. [`monitors`], [`foreground`], [`logging`], and [`single_instance`] are
//!    process and desktop helpers.
//! 6. [`dialogs`] contains pure models that both UI toolkits drive: the dialog
//!    placement algorithm and the settings working copy.
//!
//! # Threading contract
//!
//! The virtual-desktop COM objects are apartment-threaded. Initialize
//! [`com::Apartment`] on the thread that will use [`desktop`], and keep every
//! call to it on that same thread. The type system does not encode this; it is
//! a runtime contract documented on the modules.
//!
//! # Feature-free by design
//!
//! The crate exposes no UI features. Front ends choose their own toolkit and
//! keep rendering, input, and dialog widgets in their own crate.

pub mod com;
pub mod desktop;
pub mod dialogs;
pub mod foreground;
pub mod logging;
pub mod monitors;
pub mod settings;
pub mod single_instance;
pub mod windows;
