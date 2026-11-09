//! Win10 virtual desktop layout (build 10240 through 21999).
//!
//! This covers both Windows 10 and Windows 10 LTSC. The two structs are opaque
//! wrappers around owned COM interface pointers; `#[repr(transparent)]` makes
//! them pointer-sized, and the `Interface` implementation supplies the IID and
//! vtable shape used by the shared code in [`super::core`].
//!
//! Only the GUIDs differ from the Win11 module; all method implementations are
//! shared through `core`.

use windows::core::{GUID, IUnknown, Interface};

use super::core::{self, DesktopVtable, ManagerVtable, VirtualDesktop};
use crate::desktop::ApiError;

const IID_VIRTUAL_DESKTOP_MANAGER_INTERNAL: GUID =
    GUID::from_u128(0xF31574D6_B682_4CDC_BD56_1827860ABEC6);
const IID_VIRTUAL_DESKTOP: GUID = GUID::from_u128(0xFF72FFDD_BE7E_43FC_9C03_AD81681E88E4);

/// A desktop object (`IVirtualDesktop`) for the Win10 layout.
#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Win10Desktop(IUnknown);

unsafe impl Interface for Win10Desktop {
    type Vtable = DesktopVtable;
    const IID: GUID = IID_VIRTUAL_DESKTOP;
}

/// The internal desktop manager (`IVirtualDesktopManagerInternal`).
#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Win10Manager(IUnknown);

unsafe impl Interface for Win10Manager {
    type Vtable = ManagerVtable;
    const IID: GUID = IID_VIRTUAL_DESKTOP_MANAGER_INTERNAL;
}

/// Opens the shared backend with the Win10 GUIDs.
pub(crate) fn open() -> Result<VirtualDesktop<Win10Manager, Win10Desktop>, ApiError> {
    core::open_with()
}
