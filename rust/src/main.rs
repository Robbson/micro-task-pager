#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Process entry point.
//!
//! The sequence is: logging, COM apartment, single-instance mutex, application
//! state, then the Win32 message loop. The COM apartment guard must stay alive
//! for the whole loop because the pager's virtual desktop objects belong to it.

use micro_task_pager::{app, com, logging, single_instance, window};

use single_instance::Acquisition;

fn main() {
    logging::init();

    if let Err(error) = run() {
        logging::error(format!("startup failed: {error}"));
        eprintln!("micro-task-pager: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    logging::info("micro-task-pager starting");

    let _apartment = com::Apartment::init()?;

    let _instance = match single_instance::acquire()? {
        Acquisition::Acquired(instance) => instance,
        Acquisition::AlreadyRunning => {
            single_instance::show_already_running();
            return Ok(());
        }
    };

    let app = app::App::new()?;
    window::run(app)
}
