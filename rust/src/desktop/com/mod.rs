mod win11_24h2;

use super::ApiError;

pub fn open() -> Result<Box<dyn super::VirtualDesktopApi>, ApiError> {
    let build = windows_version::OsVersion::current().build;

    if build >= 22621 {
        Ok(Box::new(win11_24h2::VirtualDesktop::open()?))
    } else {
        Err(ApiError::NotSupported { build })
    }
}
