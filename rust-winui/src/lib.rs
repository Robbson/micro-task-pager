//! Micro Task Pager, a WinUI 3 virtual desktop pager.
//!
//! # Where to start
//!
//! 1. `src/main.rs` for the process order: logging, COM apartment, single
//!    instance, pager.
//! 2. [`pager`] for the UI component. Its module documentation explains the
//!    state model and how one refresh flows from a tick to the view.
//! 3. `rust-winui/ARCHITECTURE.md` for the WinUI-specific design, and
//!    `rust-winui/MIGRATION.md` for the code map and phase status.
//! 4. `crates/micro-task-pager-core/ARCHITECTURE.md` for the shared behavior
//!    contract, and `crates/micro-task-pager-core/README.md` for the shared
//!    platform module map.
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
//!   Win32 and COM. The settings draft and placement algorithm come from
//!   `micro-task-pager-core`; the pager layout is WinUI-specific because it
//!   works in DIPs.
//!
//! # Modules
//!
//! - `com` initializes the COM apartment on the UI thread.
//! - `desktop` is the safe wrapper around the undocumented virtual desktop
//!   COM interfaces. Start with `desktop::open`, which returns a
//!   `desktop::SupervisedDesktop` that recreates the backend after shell
//!   restarts.
//! - [`pager`] is the WinUI 3 pager component and the application UI.
//! - [`dialogs`] contains the WinUI About and Settings windows.
//! - `settings` reads the C#-compatible registry values and provides defaults.
//! - `windows` enumerates windows and builds the `windows::WindowSnapshot`.
//! - `monitors`, `foreground` provide monitor enumeration and last-active
//!   window tracking.
//! - [`native_window`] applies pager-specific Win32 window properties that
//!   `windows-reactor` does not expose.
//! - `single_instance` prevents two pagers from running at the same time.
//! - `logging` writes a small daily log file.
//!
//! The binary and the shared `probe` example exercise the same core code path.
//! The probe lives in the core crate (`cargo run -p micro-task-pager-core
//! --example probe`).

pub use micro_task_pager_core::{
    com, desktop, foreground, logging, monitors, settings, single_instance, windows,
};

pub mod dialogs;
pub mod native_window;
pub mod pager;
