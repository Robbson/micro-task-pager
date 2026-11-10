//! Runs the reactor pager.
//!
//! Run from `rust-winui/` with `cargo run --example reactor_pager`. Since the Phase 11 cutover this is the
//! same component as the main binary; the example remains a convenient smoke runner. It uses the
//! release mutex, so stop any other pager first.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use micro_task_pager_winui::{com, logging, pager, single_instance};
use single_instance::Acquisition;

fn main() {
    logging::init();

    if let Err(error) = run() {
        logging::error(format!("startup failed: {error}"));
        eprintln!("micro-task-pager reactor shell: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    logging::info("micro-task-pager reactor shell starting");

    let _apartment = com::Apartment::init()?;

    let _instance = match single_instance::acquire()? {
        Acquisition::Acquired(instance) => instance,
        Acquisition::AlreadyRunning => {
            single_instance::show_already_running();
            return Ok(());
        }
    };

    pager::run()
}
