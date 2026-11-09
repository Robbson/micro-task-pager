//! Micro Task Pager, a WinUI 3 virtual desktop pager.
//!
//! # Where to start
//!
//! 1. `src/main.rs` for the process order: logging, COM apartment, single
//!    instance, pager.
//! 2. [`pager`] for the UI component. Its module documentation explains the
//!    state model and how one refresh flows from a tick to the view.
//! 3. `rust/ARCHITECTURE.md` for the behavior contract and the C# comparison,
//!    and `rust/MIGRATION.md` for the code map and phase status.
//!
//! # Mental model
//!
//! The crate is intentionally split into small modules so the Windows-specific
//! and unsafe code stays away from the application logic. Two ideas connect
//! them:
//!
//! - **One UI thread owns everything Windows-related.** COM objects, the
//!   virtual-desktop API, window enumeration, and the WinUI component all run
//!   on the thread that reactor drives. Modules never spawn their own COM
//!   threads.
//! - **Pure data in the middle.** Layout, hit testing, the pointer state
//!   machine, the dialog lifecycle, the settings draft, and dialog placement
//!   are plain Rust with unit tests; only the platform modules bind them to
//!   Win32 and COM.
//!
//! # Modules
//!
//! - [`com`] initializes the COM apartment on the UI thread.
//! - [`desktop`] is the safe wrapper around the undocumented virtual desktop
//!   COM interfaces. Start with [`desktop::open`], which returns a
//!   [`desktop::SupervisedDesktop`] that recreates the backend after shell
//!   restarts.
//! - [`pager`] is the WinUI 3 pager component and the application UI.
//! - [`dialogs`] contains the WinUI About and Settings windows.
//! - [`settings`] reads the C#-compatible registry values and provides defaults.
//! - [`windows`] enumerates windows and builds the [`windows::WindowSnapshot`].
//! - [`monitors`], [`foreground`] provide monitor enumeration and last-active
//!   window tracking.
//! - [`native_window`] applies pager-specific Win32 window properties that
//!   `windows-reactor` does not expose.
//! - [`single_instance`] prevents two pagers from running at the same time.
//! - [`logging`] writes a small daily log file.
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
