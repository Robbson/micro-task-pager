//! Reactor About window reference.
//!
//! Run from `rust/` with `cargo run --example reactor_about`. The component
//! lives in `src/dialogs/about.rs`; this example runs it standalone without a
//! pager link. It requires the Windows App Runtime 2.x framework package.

use micro_task_pager::dialogs::about::{About, AboutInput};
use windows_reactor::App;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    App::run_component::<About>(AboutInput::standalone())?;
    Ok(())
}
