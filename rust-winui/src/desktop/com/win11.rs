//! Win11 virtual desktop layout (build 22000 and later).
//!
//! The two structs below are opaque wrappers around owned COM interface
//! pointers. `#[repr(transparent)]` makes them pointer-sized, and the
//! `Interface` implementation supplies the IID and the vtable shape that the
//! shared code in [`super::core`] calls. `Clone` is a reference-counted copy
//! (`AddRef`), not a deep copy.
//!
//! Only the GUIDs differ from the Win10 module; all method implementations are
//! shared through `core`.

use windows::core::{GUID, IUnknown, Interface};

use super::core::{self, DesktopVtable, ManagerVtable, VirtualDesktop};
use crate::desktop::ApiError;

const IID_VIRTUAL_DESKTOP_MANAGER_INTERNAL: GUID =
    GUID::from_u128(0x53F5CA0B_158F_4124_900C_057158060B27);
const IID_VIRTUAL_DESKTOP: GUID = GUID::from_u128(0x3F07F4BE_B107_441A_AF0F_39D82529072C);

/// A desktop object (`IVirtualDesktop`) for the Win11 layout.
#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Win11Desktop(IUnknown);

unsafe impl Interface for Win11Desktop {
    type Vtable = DesktopVtable;
    const IID: GUID = IID_VIRTUAL_DESKTOP;
}

/// The internal desktop manager (`IVirtualDesktopManagerInternal`).
#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Win11Manager(IUnknown);

unsafe impl Interface for Win11Manager {
    type Vtable = ManagerVtable;
    const IID: GUID = IID_VIRTUAL_DESKTOP_MANAGER_INTERNAL;
}

/// Opens the shared backend with the Win11 GUIDs.
pub(crate) fn open() -> Result<VirtualDesktop<Win11Manager, Win11Desktop>, ApiError> {
    core::open_with()
}
