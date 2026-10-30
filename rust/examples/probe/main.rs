use std::env;
use std::error::Error;
use std::ffi::c_void;
use std::thread::sleep;
use std::time::Duration;

use micro_task_pager::com::Apartment;
use micro_task_pager::desktop::{self, PinOutcome, VirtualDesktopApi};
use windows::Win32::Foundation::HWND;

fn main() {
    if let Err(error) = run() {
        eprintln!("probe: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let _apartment = Apartment::init()?;
    let api = desktop::open()?;
    let args: Vec<String> = env::args().skip(1).collect();

    match args.as_slice() {
        [] => status(api.as_ref()),
        [command] if command == "status" => status(api.as_ref()),
        [command, hwnd] if command == "index" => index(api.as_ref(), hwnd),
        [command, index] if command == "switch" => switch(api.as_ref(), index),
        [command, hwnd, index] if command == "move" => move_window(api.as_ref(), hwnd, index),
        [command, hwnd] if command == "pin" => pin(api.as_ref(), hwnd),
        [command, hwnd] if command == "pinned" => pinned(api.as_ref(), hwnd),
        _ => {
            usage();
            Ok(())
        }
    }
}

fn status(api: &dyn VirtualDesktopApi) -> Result<(), Box<dyn Error>> {
    let build = windows_version::OsVersion::current().build;
    let count = api.count()?;
    let current = api.current_index()?;
    let ids = api.desktop_ids()?;

    println!("build: {build}");
    println!("count: {count}");
    println!("current: {current}");

    for (index, id) in ids.iter().enumerate() {
        println!("desktop[{index}]: {id:?}");
    }

    Ok(())
}

fn index(api: &dyn VirtualDesktopApi, hwnd: &str) -> Result<(), Box<dyn Error>> {
    let hwnd = parse_hwnd(hwnd)?;

    match api.index_of_window(hwnd)? {
        Some(index) => println!("window {hwnd:?} is on desktop {index}"),
        None => println!("window {hwnd:?} has no resolvable desktop"),
    }

    Ok(())
}

fn switch(api: &dyn VirtualDesktopApi, index: &str) -> Result<(), Box<dyn Error>> {
    let index = parse_u32(index)?;
    let before = api.current_index()?;

    api.switch_to(index)?;
    sleep(Duration::from_millis(300));

    let after = api.current_index()?;
    println!("current desktop: {before} -> {after}");

    Ok(())
}

fn move_window(api: &dyn VirtualDesktopApi, hwnd: &str, index: &str) -> Result<(), Box<dyn Error>> {
    let hwnd = parse_hwnd(hwnd)?;
    let index = parse_u32(index)?;
    let before = api.index_of_window(hwnd)?;

    api.move_window_to(hwnd, index)?;
    let after = api.index_of_window(hwnd)?;
    println!("window {hwnd:?}: desktop {before:?} -> {after:?}");

    Ok(())
}

fn pin(api: &dyn VirtualDesktopApi, hwnd: &str) -> Result<(), Box<dyn Error>> {
    let hwnd = parse_hwnd(hwnd)?;

    match api.pin_application(hwnd)? {
        PinOutcome::AlreadyPinned => println!("application for {hwnd:?} was already pinned"),
        PinOutcome::Pinned => println!("application for {hwnd:?} is now pinned"),
    }

    Ok(())
}

fn pinned(api: &dyn VirtualDesktopApi, hwnd: &str) -> Result<(), Box<dyn Error>> {
    let hwnd = parse_hwnd(hwnd)?;
    println!(
        "application for {hwnd:?} pinned: {}",
        api.is_application_pinned(hwnd)?
    );

    Ok(())
}

fn parse_hwnd(value: &str) -> Result<HWND, Box<dyn Error>> {
    let trimmed = value.trim();
    let number = if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        usize::from_str_radix(hex, 16)?
    } else {
        trimmed.parse::<usize>()?
    };

    if number == 0 {
        return Err("window handle must not be zero".into());
    }

    Ok(HWND(number as *mut c_void))
}

fn parse_u32(value: &str) -> Result<u32, Box<dyn Error>> {
    Ok(value.trim().parse::<u32>()?)
}

fn usage() {
    eprintln!("usage:");
    eprintln!("  probe                      print build, desktop count, current index, and ids");
    eprintln!("  probe status               same as no arguments");
    eprintln!("  probe index <hwnd>         print the desktop index of a window");
    eprintln!("  probe switch <index>       switch to a desktop");
    eprintln!("  probe move <hwnd> <index>  move a window to a desktop");
    eprintln!("  probe pin <hwnd>           pin the window's application to all desktops");
    eprintln!("  probe pinned <hwnd>        query whether the application is pinned");
    eprintln!("  <hwnd> accepts decimal or 0x-prefixed hexadecimal");
}
