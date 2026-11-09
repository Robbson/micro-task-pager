//! Single-instance guard built on a named Win32 mutex.
//!
//! A named mutex is a kernel object with a name shared across processes.
//! `CreateMutexW` creates it if it does not exist or opens the existing one.
//! Because `bInitialOwner` is `true`, only the process that actually created
//! the mutex owns it; a second process gets a valid handle but
//! `GetLastError` reports `ERROR_ALREADY_EXISTS`. That is the signal to show a
//! message and exit instead of starting a second pager.
//!
//! The handle is owned by `Instance`, which closes it in `Drop`. Rust's
//! deterministic destructors make this the equivalent of a C# `using` block or
//! a `try/finally`.

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::{Result, w};

/// Result of trying to become the single running instance.
pub enum Acquisition {
    /// This process is the first instance and now owns the mutex.
    Acquired(Instance),
    /// Another process already owns the mutex.
    AlreadyRunning,
}

/// Owns the mutex handle and releases it when dropped.
pub struct Instance {
    handle: HANDLE,
}

/// Tries to acquire the application-wide mutex.
pub fn acquire() -> Result<Acquisition> {
    // The `w!` macro creates a wide (UTF-16) string literal at compile time,
    // which is what the `W` Win32 functions expect.
    let handle = unsafe { CreateMutexW(None, true, w!("micro-task-pager-win-app"))? };

    // `CreateMutexW` succeeds even when the mutex already exists; the extra
    // information is in the thread's last-error value, so it must be checked
    // immediately after the call.
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            // We opened the existing mutex but did not become its owner, so we
            // only close our handle. Releasing ownership here would be wrong.
            let _ = CloseHandle(handle);
        }
        return Ok(Acquisition::AlreadyRunning);
    }

    Ok(Acquisition::Acquired(Instance { handle }))
}

/// Shows the "already running" message without starting the pager.
pub fn show_already_running() {
    unsafe {
        let _ = MessageBoxW(
            None,
            w!("Micro Task Pager is already running."),
            w!("Startup Error"),
            MB_OK | MB_ICONERROR,
        );
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            // Release ownership first, then close the handle. Both calls are
            // best-effort in a destructor and cannot return an error to the
            // caller.
            let _ = ReleaseMutex(self.handle);
            let _ = CloseHandle(self.handle);
        }
    }
}
