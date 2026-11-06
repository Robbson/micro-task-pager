//! Runs the Phase 8 reactor pager shell.
//!
//! Run from `rust/` with `cargo run --example reactor_pager`. The native v1 in
//! `src/main.rs` stays the main binary until Phase 11. This example is the
//! runnable shell used to verify borderless, topmost, layered, and pinned
//! behavior with the target UI stack.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use micro_task_pager::{com, logging, pager, single_instance};
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
