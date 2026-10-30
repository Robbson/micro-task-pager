mod com;

use std::fmt;

use windows::Win32::Foundation::HWND;
use windows::core::GUID;

pub use com::open;

#[derive(Debug)]
pub enum ApiError {
    NotSupported { build: u32 },
    Windows(windows::core::Error),
    UnexpectedNull(&'static str),
    DesktopOutOfRange { index: u32, count: u32 },
    DesktopNotFound,
    WindowNotFound(HWND),
    InvalidString,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinOutcome {
    AlreadyPinned,
    Pinned,
}

pub trait VirtualDesktopApi {
    fn count(&self) -> Result<u32, ApiError>;
    fn current_index(&self) -> Result<u32, ApiError>;
    fn desktop_ids(&self) -> Result<Vec<GUID>, ApiError>;
    fn index_of_window(&self, hwnd: HWND) -> Result<Option<u32>, ApiError>;
    fn switch_to(&self, index: u32) -> Result<(), ApiError>;
    fn move_window_to(&self, hwnd: HWND, index: u32) -> Result<(), ApiError>;
    fn is_application_pinned(&self, hwnd: HWND) -> Result<bool, ApiError>;
    fn pin_application(&self, hwnd: HWND) -> Result<PinOutcome, ApiError>;
}
