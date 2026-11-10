//! Selection between the two virtual desktop COM layouts.
//!
//! The C# reference implementation has four variants (`Win10LTSC`, `Win10`,
//! `Win11`, `Win11_`). The Rust port only needs two because the methods it
//! calls use the same interface GUIDs and the same vtable prefix inside each
//! pair:
//!
//! - Win10 layout: build 10240 through 21999 (Win10 and Win10LTSC).
//! - Win11 layout: build 22000 and later.
//!
//! Each layout module only declares GUIDs and vtable prefixes. The actual
//! implementation is shared in [`core`].

mod core;
mod win10;
mod win11;

use super::{ApiError, VirtualDesktopApi};
use crate::logging;

/// Which COM layout should be used for a Windows build number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendKind {
    Win10,
    Win11,
    Unsupported,
}

/// Maps a build number to a layout without touching COM, which keeps the
/// threshold logic unit-testable.
fn backend_kind(build: u32) -> BackendKind {
    if build < 10240 {
        BackendKind::Unsupported
    } else if build >= 22000 {
        BackendKind::Win11
    } else {
        BackendKind::Win10
    }
}

/// Creates the backend for the running build.
///
/// This function returns a boxed trait object so callers do not depend on the
/// concrete layout type. The supervisor may call it again after a restartable
/// failure; each call creates a fresh set of shell objects.
pub(crate) fn open_backend() -> Result<Box<dyn VirtualDesktopApi>, ApiError> {
    let build = windows_version::OsVersion::current().build;
    logging::info(format!("opening virtual desktop backend for build {build}"));

    match backend_kind(build) {
        BackendKind::Unsupported => Err(ApiError::NotSupported { build }),
        BackendKind::Win11 => {
            logging::info("selected Win11 virtual desktop layout");
            Ok(Box::new(win11::open()?))
        }
        BackendKind::Win10 => {
            logging::info("selected Win10 virtual desktop layout");
            Ok(Box::new(win10::open()?))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_win10_for_windows_10_and_ltsc() {
        assert_eq!(backend_kind(17763), BackendKind::Win10);
        assert_eq!(backend_kind(19045), BackendKind::Win10);
        assert_eq!(backend_kind(21999), BackendKind::Win10);
    }

    #[test]
    fn selects_win11_for_windows_11() {
        assert_eq!(backend_kind(22000), BackendKind::Win11);
        assert_eq!(backend_kind(22621), BackendKind::Win11);
        assert_eq!(backend_kind(26200), BackendKind::Win11);
    }

    #[test]
    fn rejects_builds_before_windows_10() {
        assert_eq!(backend_kind(10239), BackendKind::Unsupported);
    }
}
