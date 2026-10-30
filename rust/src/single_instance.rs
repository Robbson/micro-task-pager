use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::{Result, w};

pub enum Acquisition {
    Acquired(Instance),
    AlreadyRunning,
}

pub struct Instance {
    handle: HANDLE,
}

pub fn acquire() -> Result<Acquisition> {
    let handle = unsafe { CreateMutexW(None, true, w!("micro-task-pager-win-app"))? };

    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            let _ = CloseHandle(handle);
        }
        return Ok(Acquisition::AlreadyRunning);
    }

    Ok(Acquisition::Acquired(Instance { handle }))
}

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
            let _ = ReleaseMutex(self.handle);
            let _ = CloseHandle(self.handle);
        }
    }
}
