//! Safe virtual desktop API.
//!
//! There are three layers:
//!
//! 1. The raw COM interfaces and vtables live in `desktop/com`. They are the
//!    only place that knows about GUIDs, `HRESULT`s, and raw pointers.
//! 2. [`VirtualDesktopApi`] is the small trait the rest of the application
//!    should use. It returns normal Rust `Result` values and plain types such
//!    as `u32`, `GUID`, and `HWND`.
//! 3. [`SupervisedDesktop`] wraps one backend and adds a retry-once policy for
//!    restartable COM failures. "Supervised" means the wrapper watches the
//!    backend; it does not start a thread.
//!
//! All calls must happen on the COM apartment thread that was initialized with
//! [`crate::com::Apartment`]. The UI thread will own these objects; no `Send`
//! or cross-thread access is allowed.

mod com;

use std::cell::RefCell;
use std::fmt;

use windows::Win32::Foundation::HWND;
use windows::core::{GUID, HRESULT};

use crate::logging;

/// Errors returned by the safe virtual desktop API.
#[derive(Debug)]
pub enum ApiError {
    /// The running Windows build is older than the supported minimum.
    NotSupported { build: u32 },
    /// A COM call failed. The original `HRESULT` is preserved.
    Windows(windows::core::Error),
    /// The shell returned a null pointer where an object was required.
    UnexpectedNull(&'static str),
    /// A desktop index was outside `0..count`.
    DesktopOutOfRange { index: u32, count: u32 },
    /// The current desktop GUID was not present in the desktop list.
    DesktopNotFound,
    /// The window has no application view or no resolvable desktop.
    WindowNotFound(HWND),
    /// A string returned by the shell was not valid UTF-16.
    InvalidString,
}

impl ApiError {
    /// Returns `true` when recreating the COM objects is a sensible response.
    ///
    /// The virtual desktop objects live in the shell process. When that
    /// connection is lost, every cached pointer becomes unusable. RPC errors
    /// (`0x8001xxxx`), `E_FAIL`, `E_UNEXPECTED`, and the null-reference failure
    /// seen from `FindDesktop` are treated as restartable. Ordinary argument
    /// errors are not; retrying them would only repeat the same mistake.
    pub fn is_restartable(&self) -> bool {
        match self {
            Self::Windows(error) => is_restartable_hresult(error.code()),
            _ => false,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSupported { build } => {
                write!(
                    f,
                    "Windows build {build} is not supported by the Rust port yet"
                )
            }
            Self::Windows(error) => write!(f, "{error}"),
            Self::UnexpectedNull(value) => write!(f, "unexpected null {value}"),
            Self::DesktopOutOfRange { index, count } => {
                write!(f, "desktop index {index} is out of range (count {count})")
            }
            Self::DesktopNotFound => write!(f, "desktop was not found in the desktop list"),
            Self::WindowNotFound(hwnd) => write!(f, "no application view for window {hwnd:?}"),
            Self::InvalidString => write!(f, "string returned by the shell was not valid UTF-16"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<windows::core::Error> for ApiError {
    fn from(error: windows::core::Error) -> Self {
        Self::Windows(error)
    }
}

/// Result of asking to pin an application to all desktops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinOutcome {
    /// The application was already pinned; no shell call was needed.
    AlreadyPinned,
    /// The application is now pinned.
    Pinned,
}

/// The safe operations the application needs from the virtual desktop shell.
///
/// Implementations are version-specific, but callers only see this trait. A
/// `HWND` is an opaque Windows window handle; it is not a Rust pointer that
/// callers should dereference.
pub trait VirtualDesktopApi {
    /// Number of virtual desktops.
    fn count(&self) -> Result<u32, ApiError>;
    /// Index of the desktop that is currently shown.
    fn current_index(&self) -> Result<u32, ApiError>;
    /// Stable GUIDs of all desktops, in index order.
    fn desktop_ids(&self) -> Result<Vec<GUID>, ApiError>;
    /// Desktop index for a window, or `None` if the shell cannot map it.
    fn index_of_window(&self, hwnd: HWND) -> Result<Option<u32>, ApiError>;
    /// Switches to a desktop by index.
    fn switch_to(&self, index: u32) -> Result<(), ApiError>;
    /// Moves a window to a desktop by index.
    fn move_window_to(&self, hwnd: HWND, index: u32) -> Result<(), ApiError>;
    /// Whether the application that owns the window is pinned to all desktops.
    fn is_application_pinned(&self, hwnd: HWND) -> Result<bool, ApiError>;
    /// Pins the application that owns the window to all desktops.
    fn pin_application(&self, hwnd: HWND) -> Result<PinOutcome, ApiError>;
    /// Removes the all-desktops pin from the application that owns the window.
    fn unpin_application(&self, hwnd: HWND) -> Result<(), ApiError>;
}

/// Opens the virtual desktop API with the layout for the running Windows build.
///
/// This is the normal entry point for the application. The returned
/// [`SupervisedDesktop`] owns one backend and can recreate it after a
/// restartable COM failure.
pub fn open() -> Result<SupervisedDesktop, ApiError> {
    SupervisedDesktop::open()
}

type BackendFactory = Box<dyn Fn() -> Result<Box<dyn VirtualDesktopApi>, ApiError>>;

/// A safe API wrapper that recreates its COM backend after a restartable error.
///
/// `VirtualDesktopApi` methods take `&self`, but a restart needs to replace the
/// backend. `RefCell` provides interior mutability: it allows changing the
/// value through a shared reference. This is safe here because all calls are
/// expected on one thread; `RefCell` would panic on a reentrant borrow, not
/// race across threads.
///
/// The backend factory is stored separately so tests can replace it with a
/// fake backend and verify the retry logic without touching real COM objects.
pub struct SupervisedDesktop {
    inner: RefCell<Option<Box<dyn VirtualDesktopApi>>>,
    factory: BackendFactory,
}

impl SupervisedDesktop {
    /// Opens the real backend for the running Windows build.
    pub fn open() -> Result<Self, ApiError> {
        Self::with_factory(Box::new(com::open_backend))
    }

    /// Builds the wrapper with an injected backend factory.
    fn with_factory(factory: BackendFactory) -> Result<Self, ApiError> {
        let backend = factory()?;
        logging::info("virtual desktop backend opened");

        Ok(Self {
            inner: RefCell::new(Some(backend)),
            factory,
        })
    }

    /// Runs one operation against the backend, retrying once after a
    /// restartable failure.
    ///
    /// Attempt 0 uses the current backend. If it fails with a restartable
    /// error, the backend is dropped and attempt 1 creates a fresh one.
    /// Non-restartable errors are returned immediately. The `RefCell` borrow is
    /// scoped to the call so replacing the backend afterwards is allowed.
    fn execute<T>(
        &self,
        operation: impl Fn(&dyn VirtualDesktopApi) -> Result<T, ApiError>,
    ) -> Result<T, ApiError> {
        let mut inner = self.inner.borrow_mut();

        for attempt in 0..2 {
            if inner.is_none() {
                *inner = Some((self.factory)()?);
                logging::info("virtual desktop backend recreated");
            }

            let result = match inner.as_deref() {
                Some(api) => operation(api),
                None => return Err(ApiError::UnexpectedNull("virtual desktop backend")),
            };

            match result {
                Ok(value) => return Ok(value),
                Err(error) => {
                    let restart = attempt == 0 && error.is_restartable();
                    logging::warn(format!(
                        "virtual desktop call failed (restart={restart}): {error}"
                    ));
                    if !restart {
                        return Err(error);
                    }
                    *inner = None;
                }
            }
        }

        Err(ApiError::UnexpectedNull(
            "virtual desktop backend after restart",
        ))
    }
}

impl VirtualDesktopApi for SupervisedDesktop {
    fn count(&self) -> Result<u32, ApiError> {
        self.execute(|api| api.count())
    }

    fn current_index(&self) -> Result<u32, ApiError> {
        self.execute(|api| api.current_index())
    }

    fn desktop_ids(&self) -> Result<Vec<GUID>, ApiError> {
        self.execute(|api| api.desktop_ids())
    }

    fn index_of_window(&self, hwnd: HWND) -> Result<Option<u32>, ApiError> {
        self.execute(|api| api.index_of_window(hwnd))
    }

    fn switch_to(&self, index: u32) -> Result<(), ApiError> {
        self.execute(|api| api.switch_to(index))
    }

    fn move_window_to(&self, hwnd: HWND, index: u32) -> Result<(), ApiError> {
        self.execute(|api| api.move_window_to(hwnd, index))
    }

    fn is_application_pinned(&self, hwnd: HWND) -> Result<bool, ApiError> {
        self.execute(|api| api.is_application_pinned(hwnd))
    }

    fn pin_application(&self, hwnd: HWND) -> Result<PinOutcome, ApiError> {
        self.execute(|api| api.pin_application(hwnd))
    }

    fn unpin_application(&self, hwnd: HWND) -> Result<(), ApiError> {
        self.execute(|api| api.unpin_application(hwnd))
    }
}

/// Recognizes the COM failures that mean "the shell connection is gone".
fn is_restartable_hresult(code: HRESULT) -> bool {
    let value = code.0 as u32;

    matches!(value, 0x8000_4005 | 0x8000_FFFF | 0x8007_06F4) || (value & 0xFFFF_0000) == 0x8001_0000
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;
    use windows::core::Error;

    /// Minimal fake backend used to exercise the supervisor without COM.
    struct Fake {
        count: u32,
        error: Option<HRESULT>,
    }

    impl VirtualDesktopApi for Fake {
        fn count(&self) -> Result<u32, ApiError> {
            match self.error {
                Some(code) => Err(ApiError::Windows(Error::from_hresult(code))),
                None => Ok(self.count),
            }
        }

        fn current_index(&self) -> Result<u32, ApiError> {
            unimplemented!()
        }

        fn desktop_ids(&self) -> Result<Vec<GUID>, ApiError> {
            unimplemented!()
        }

        fn index_of_window(&self, _hwnd: HWND) -> Result<Option<u32>, ApiError> {
            unimplemented!()
        }

        fn switch_to(&self, _index: u32) -> Result<(), ApiError> {
            unimplemented!()
        }

        fn move_window_to(&self, _hwnd: HWND, _index: u32) -> Result<(), ApiError> {
            unimplemented!()
        }

        fn is_application_pinned(&self, _hwnd: HWND) -> Result<bool, ApiError> {
            unimplemented!()
        }

        fn pin_application(&self, _hwnd: HWND) -> Result<PinOutcome, ApiError> {
            unimplemented!()
        }

        fn unpin_application(&self, _hwnd: HWND) -> Result<(), ApiError> {
            unimplemented!()
        }
    }

    fn supervisor_with_failure(error: HRESULT) -> (SupervisedDesktop, Rc<Cell<u32>>) {
        let opens = Rc::new(Cell::new(0));
        let opens_for_factory = Rc::clone(&opens);

        let supervisor = SupervisedDesktop {
            inner: RefCell::new(Some(Box::new(Fake {
                count: 0,
                error: Some(error),
            }))),
            factory: Box::new(move || {
                opens_for_factory.set(opens_for_factory.get() + 1);
                Ok(Box::new(Fake {
                    count: 7,
                    error: None,
                }))
            }),
        };

        (supervisor, opens)
    }

    #[test]
    fn rpc_errors_are_restartable() {
        let error = ApiError::Windows(Error::from_hresult(HRESULT(0x80010108u32 as i32)));

        assert!(error.is_restartable());
    }

    #[test]
    fn invalid_argument_is_not_restartable() {
        let error = ApiError::Windows(Error::from_hresult(HRESULT(0x80070057u32 as i32)));

        assert!(!error.is_restartable());
    }

    #[test]
    fn supervisor_restarts_once_after_rpc_error() {
        let (supervisor, opens) = supervisor_with_failure(HRESULT(0x80010108u32 as i32));

        assert_eq!(supervisor.count().unwrap(), 7);
        assert_eq!(opens.get(), 1);
    }

    #[test]
    fn supervisor_does_not_restart_after_invalid_argument() {
        let (supervisor, opens) = supervisor_with_failure(HRESULT(0x80070057u32 as i32));

        assert!(supervisor.count().is_err());
        assert_eq!(opens.get(), 0);
    }
}
