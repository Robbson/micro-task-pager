//! Reactor Settings window runner.
//!
//! Run from `rust/` with `cargo run --example reactor_settings`. The dialog uses
//! the saved registry settings as its starting point but has no pager link, so
//! previews and Apply/OK only update the dialog state.
//!
//! Stop the native or reactor pager first only when you want to compare
//! settings side by side; this example does not create the pager mutex.

use micro_task_pager::dialogs::settings::{SettingsDialog, SettingsInput};
use micro_task_pager::settings::Settings;
use windows_reactor::App;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    App::run_component::<SettingsDialog>(SettingsInput::standalone(Settings::load()))?;
    Ok(())
}
