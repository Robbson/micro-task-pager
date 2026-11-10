//! Native dialogs.
//!
//! Both dialogs are modal win32 dialogs owned by the pager window, not separate
//! top-level components:
//!
//! - [`about`] is a winsafe `WindowModal` with labels and a Close button.
//! - [`settings`] loads a fixed `.rc` template with three tab pages and returns
//!   the accepted settings to [`crate::app::App`].
//!
//! Placement uses the same right/left/below/above sequence as the C# reference
//! and `rust-winui`; see [`placement`].

pub mod about;
pub use micro_task_pager_core::dialogs::placement;
pub mod settings;
