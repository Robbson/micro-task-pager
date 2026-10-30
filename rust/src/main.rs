//! Temporary console entry point for the Rust port.
//!
//! This binary currently proves the startup path without creating a window:
//! it initializes logging, initializes COM, enforces the single-instance
//! mutex, loads the registry settings, and exits. The Win32 window and message
//! loop replace the final `println!` in Phase 4.
//!
//! The real desktop API is exercised by `examples/probe`, which is why this
//! file does not open the virtual desktop backend yet.

use micro_task_pager::{com, logging, settings, single_instance};

use single_instance::Acquisition;

fn main() {
    logging::init();

    if let Err(error) = run() {
        // Logging is best-effort, but it is useful to record startup failures
        // before the console message disappears.
        logging::error(format!("startup failed: {error}"));
        eprintln!("micro-task-pager: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    logging::info("micro-task-pager starting");

    // COM must be initialized on this thread before any shell interface is
    // created, and all later COM calls must stay on this thread.
    let _apartment = com::Apartment::init()?;

    let _instance = match single_instance::acquire()? {
        Acquisition::Acquired(instance) => instance,
        Acquisition::AlreadyRunning => {
            single_instance::show_already_running();
            return Ok(());
        }
    };

    // The settings loader is deliberately infallible: missing registry values
    // fall back to the defaults from the C# application.
    let settings = settings::Settings::load();
    println!("{settings:#?}");

    Ok(())
}
