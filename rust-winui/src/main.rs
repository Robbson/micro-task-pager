#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Process entry point.
//!
//! The sequence is deliberate:
//!
//! 1. Logging first, so startup failures are recorded.
//! 2. `com::Apartment` initializes STA on this thread. The pager's virtual
//!    desktop objects belong to it, and the guard must stay alive for the whole
//!    UI loop; dropping it earlier would invalidate those objects.
//! 3. The single-instance mutex. A second instance shows a message and exits
//!    before creating any window, so the two implementations cannot fight over
//!    topmost or desktop pinning.
//! 4. The reactor pager takes over this thread with the WinUI message loop.
//!
//! Reactor runs the pager component on this same thread, so every COM call the
//! pager makes stays inside the apartment.

use micro_task_pager_winui::{com, logging, pager, single_instance};

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

    let result = pager::run();
    logging::info("micro-task-pager stopped");
    result
}
