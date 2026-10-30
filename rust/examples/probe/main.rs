//! Standalone probe for the virtual desktop COM backend.
//!
//! A Cargo *example* is a separate binary built from the same crate; it is not
//! part of the pager. This one lets the COM layer be tested from a terminal
//! without starting the UI:
//!
//! ```text
//! cargo run --example probe              # read-only status
//! cargo run --example probe -- index <hwnd>
//! cargo run --example probe -- switch <index>
//! cargo run --example probe -- move <hwnd> <index>
//! cargo run --example probe -- pin <hwnd> / pinned <hwnd> / unpin <hwnd>
//! cargo run --example probe -- window 60 # throwaway window for mutation tests
//! ```
//!
//! Start with the read-only commands. `switch`, `move`, `pin`, and `unpin`
//! change real desktop or shell state. `window` exists so those tests can use a
//! window owned by the probe instead of moving a real application.
//!
//! The probe initializes COM itself, exactly like the future UI thread will.

use std::env;
use std::error::Error;
use std::ffi::c_void;
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

use micro_task_pager::com::Apartment;
use micro_task_pager::desktop::{self, PinOutcome, VirtualDesktopApi};
use micro_task_pager::logging;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::CreateSolidBrush;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetMessageW, IDC_ARROW, LoadCursorW, MSG, PostMessageW, PostQuitMessage, RegisterClassW,
    SW_SHOW, SetTimer, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WM_CLOSE, WM_DESTROY,
    WM_TIMER, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

fn main() {
    logging::init();

    if let Err(error) = run() {
        logging::error(format!("probe failed: {error}"));
        eprintln!("probe: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    logging::info(format!("probe command: {args:?}"));

    let _apartment = Apartment::init()?;

    // Give the probe process a stable Application User Model ID. Windows uses
    // this ID for pinning; without it, GetAppUserModelId may return an empty
    // value and pin/unpin cannot be tested safely.
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(w!("MicroTaskPager.Probe"));
    }

    let api = desktop::open()?;

    match args.as_slice() {
        [] => status(&api),
        [command] if command == "status" => status(&api),
        [command, hwnd] if command == "index" => index(&api, hwnd),
        [command, index] if command == "switch" => switch(&api, index),
        [command, hwnd, index] if command == "move" => move_window(&api, hwnd, index),
        [command, hwnd] if command == "pin" => pin(&api, hwnd),
        [command, hwnd] if command == "pinned" => pinned(&api, hwnd),
        [command, hwnd] if command == "unpin" => unpin(&api, hwnd),
        [command] if command == "window" => window("30"),
        [command, seconds] if command == "window" => window(seconds),
        _ => {
            usage();
            Ok(())
        }
    }
}

fn status(api: &dyn VirtualDesktopApi) -> Result<(), Box<dyn Error>> {
    // `dyn VirtualDesktopApi` means "any type implementing the trait"; the
    // probe does not need to know whether the backend is Win10 or Win11.
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
    // The shell animates desktop switches. A short pause makes the following
    // `current_index` call report the destination instead of the old desktop.
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

fn unpin(api: &dyn VirtualDesktopApi, hwnd: &str) -> Result<(), Box<dyn Error>> {
    let hwnd = parse_hwnd(hwnd)?;
    api.unpin_application(hwnd)?;
    println!("application for {hwnd:?} is unpinned");

    Ok(())
}

fn window(seconds: &str) -> Result<(), Box<dyn Error>> {
    // The window is only a test fixture. It closes itself through a WM_TIMER
    // message, so a forgotten probe cannot stay on screen forever.
    let seconds = seconds.parse::<u32>().unwrap_or(30).clamp(1, 3600);

    unsafe {
        let module = GetModuleHandleW(PCWSTR::null())?;
        let instance = HINSTANCE(module.0);
        let brush = CreateSolidBrush(COLORREF(0x0020_2020));
        let cursor = LoadCursorW(None, IDC_ARROW)?;
        let class_name = w!("MicroTaskPagerProbeWindow");
        let wndclass = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: cursor,
            hbrBackground: brush,
            lpszClassName: class_name,
            ..Default::default()
        };

        if RegisterClassW(&wndclass) == 0 {
            return Err(windows::core::Error::from_thread().into());
        }

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("Micro Task Pager Probe"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            100,
            100,
            320,
            200,
            None,
            None,
            Some(instance),
            None,
        )?;

        let _ = ShowWindow(hwnd, SW_SHOW);
        // `SetTimer` posts WM_TIMER after the requested delay; `wndproc`
        // translates that into WM_CLOSE.
        let _ = SetTimer(Some(hwnd), 1, seconds * 1000, None);
        println!("hwnd: 0x{:x}", hwnd.0 as usize);
        let _ = std::io::stdout().flush();

        // A Win32 message loop. `GetMessageW` blocks until a message arrives and
        // returns false for WM_QUIT, which is posted by `PostQuitMessage` after
        // WM_DESTROY.
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    Ok(())
}

// A window procedure is an `extern "system"` callback: Windows calls it with
// the platform calling convention. `unsafe_op_in_unsafe_fn` is denied crate
// wide, so each unsafe Win32 call inside this unsafe function needs its own
// block.
unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_CLOSE => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        WM_TIMER => {
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn parse_hwnd(value: &str) -> Result<HWND, Box<dyn Error>> {
    // `HWND` is a handle, not a pointer that Rust may dereference. Parse it as
    // an integer and wrap it so it can be passed to the COM API.
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
    eprintln!("  probe unpin <hwnd>         unpin the window's application");
    eprintln!("  probe window [seconds]     create a throwaway window and wait");
    eprintln!("  <hwnd> accepts decimal or 0x-prefixed hexadecimal");
}
