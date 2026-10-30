use micro_task_pager::{com, settings, single_instance};

use single_instance::Acquisition;

fn main() {
    if let Err(error) = run() {
        eprintln!("micro-task-pager: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let _apartment = com::Apartment::init()?;

    let _instance = match single_instance::acquire()? {
        Acquisition::Acquired(instance) => instance,
        Acquisition::AlreadyRunning => {
            single_instance::show_already_running();
            return Ok(());
        }
    };

    let settings = settings::Settings::load();
    println!("{settings:#?}");

    Ok(())
}
