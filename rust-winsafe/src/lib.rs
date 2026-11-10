//! Micro Task Pager, a native Win32 virtual desktop pager built on `winsafe`.
//!
//! This is the stable Rust implementation. It has no Windows App Runtime or
//! reactor dependency and ships a single statically linked executable. The
//! behavioral reference is `csharp-winforms`; the experimental Rust pager is
//! `rust-winui`.
//!
//! # Where to start
//!
//! 1. `src/main.rs` for the process order: logging, COM apartment, single
//!    instance, application state, window loop.
//! 2. [`app`] for the coordinator. Its module documentation explains the timer
//!    flow and groups the handlers by concern.
//! 3. `rust-winsafe/MIGRATION.md` for the source baseline, verification log, and
//!    known gaps.
//! 4. `crates/micro-task-pager-core/ARCHITECTURE.md` for the shared behavior
//!    contract, and `crates/micro-task-pager-core/README.md` for the shared
//!    platform module map.
//!
//! # Mental model
//!
//! The crate is split so Windows-specific and `unsafe` code stays close to the
//! platform and the application logic stays plain Rust:
//!
//! - **One UI thread owns everything Windows-related.** The COM objects, the
//!   virtual-desktop API, window enumeration, the winsafe window, and all
//!   drawing run on the thread that runs the message loop. Nothing spawns a COM
//!   thread.
//! - **Layout is data.** `render::layout` produces a [`render::PagerLayout`]
//!   from a snapshot; `render::paint` turns that layout into [`render::Painter`]
//!   calls. The same layout drives painting and hit testing, so they cannot
//!   disagree.
//! - **Platform code is shared.** COM, virtual desktops, window enumeration,
//!   settings, logging, single instance, monitor/foreground tracking, and the
//!   pure dialog helpers come from `micro-task-pager-core`. They are re-exported
//!   below, so internal paths such as `crate::desktop` keep working.
//!
//! # Modules
//!
//! - [`app`] owns the pager state and translates window events into state
//!   changes and desktop actions.
//! - `com` initializes the COM apartment on the UI thread.
//! - `desktop` is the safe wrapper around the undocumented virtual desktop
//!   COM interfaces. Start with `desktop::open`, which returns a
//!   `desktop::SupervisedDesktop` that recreates the backend after shell
//!   restarts.
//! - [`dialogs`] contains the native About and Settings dialogs.
//! - [`render`] builds the pager geometry and paints it through GDI.
//! - [`window`] is the winsafe window host and message loop.
//! - `windows` enumerates windows and builds the `windows::WindowSnapshot`.
//! - `monitors`, `foreground` provide monitor enumeration and last-active
//!   window tracking.
//! - `settings` reads the C#-compatible registry values and provides defaults.
//! - `single_instance` prevents two pagers from running at the same time.
//! - `logging` writes a small daily log file.
//!
//! The binary and the shared `probe` example exercise the same core code path.
//! The probe lives in the core crate (`cargo run -p micro-task-pager-core
//! --example probe`).

pub use micro_task_pager_core::{
    com, desktop, foreground, logging, monitors, settings, single_instance, windows,
};

pub mod app;
pub mod dialogs;
pub mod render;
pub mod window;
