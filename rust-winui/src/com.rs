//! COM apartment initialization for the UI thread.
//!
//! The shell virtual desktop objects are *apartment-threaded*: they must be
//! created and used on a thread that initialized COM as a single-threaded
//! apartment (STA). Rust does not enforce that rule, so the application keeps
//! all COM work on the UI thread and uses this guard to pair
//! `CoInitializeEx` with `CoUninitialize`.
//!
//! `Apartment` is an RAII guard. RAII ("resource acquisition is
//! initialization") means the constructor acquires a resource and `Drop` (the
//! Rust destructor that runs when the value goes out of scope) releases it.

use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::core::Result;

/// Owns one COM apartment initialization on the current thread.
///
/// Keep this value alive for as long as any COM interface is in use. Dropping
/// it calls `CoUninitialize`; using COM afterwards is invalid.
pub struct Apartment;

impl Apartment {
    /// Initializes the current thread as a single-threaded apartment (STA).
    ///
    /// Call this once at startup, before creating the virtual desktop backend.
    /// The `Result` reports a failed `CoInitializeEx` (for example because the
    /// thread already initialized COM with a different apartment model).
    pub fn init() -> Result<Self> {
        unsafe {
            // `CoInitializeEx` returns an `HRESULT`; `.ok()` turns it into a
            // Rust `Result`. The `unsafe` block is required because the call
            // changes thread-local COM state and is a foreign function.
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        Ok(Self)
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            // Destructors cannot return errors, so the HRESULT is ignored.
            CoUninitialize();
        }
    }
}
