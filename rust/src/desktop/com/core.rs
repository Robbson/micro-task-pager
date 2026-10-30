//! Shared COM plumbing for the virtual desktop layouts.
//!
//! # How the COM declarations work
//!
//! A COM object is a pointer to a pointer to a *vtable*: a table of function
//! pointers. The undocumented shell interfaces are not in the generated
//! `windows` bindings, so their vtables are described here as `#[repr(C)]`
//! structs. The field order must match what the operating system uses exactly,
//! including methods that this application never calls.
//!
//! The first field of every vtable is `IUnknown_Vtbl`
//! (`QueryInterface`/`AddRef`/`Release`). The `Interface` implementation on each
//! wrapper type tells the `windows` crate which IID and which vtable type to
//! use. `as_raw`/`from_raw` convert between the wrapper and the raw COM pointer;
//! the wrapper owns one reference count.
//!
//! # Why some methods have odd signatures
//!
//! The C# reference declares many methods as returning `bool` or `int` without
//! `[PreserveSig]`. In COM those methods actually return an `HRESULT` and write
//! the real value through an out pointer. For example, `IsAppIdPinned` is
//! `HRESULT IsAppIdPinned(PCWSTR, BOOL*)`, not a `BOOL` return. The vtable
//! fields below use the native signatures so the arguments stay aligned.
//!
//! # Why two generic parameters
//!
//! [`VirtualDesktop`] is generic over the manager type `M` and the desktop type
//! `D`. That is what lets one implementation serve both the Win10 and Win11
//! layouts: the layout modules only provide the two wrapper types (with their
//! GUIDs), while all method bodies live here. `PhantomData` records that `D`
//! belongs to the type even though no field stores a `D` directly.

use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::null_mut;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree, IServiceProvider};
use windows::Win32::UI::Shell::Common::IObjectArray;
use windows::Win32::UI::Shell::IVirtualDesktopManager;
use windows::core::{BOOL, GUID, HRESULT, IUnknown, IUnknown_Vtbl, Interface, PCWSTR, PWSTR};

use crate::desktop::{ApiError, PinOutcome, VirtualDesktopApi};

const CLSID_IMMERSIVE_SHELL: GUID = GUID::from_u128(0xC2F03A33_21F5_47FA_B4BB_156362A2F239);
const CLSID_VIRTUAL_DESKTOP_MANAGER: GUID = GUID::from_u128(0xAA509086_5CA9_4C25_8F95_589D3C07B48A);
const CLSID_VIRTUAL_DESKTOP_MANAGER_INTERNAL: GUID =
    GUID::from_u128(0xC5E0CDCA_7B6E_41B2_9FC4_D93975CC467B);
const CLSID_VIRTUAL_DESKTOP_PINNED_APPS: GUID =
    GUID::from_u128(0xB5A399E7_1C87_46B8_88E9_FC5747B171BD);
const APP_ON_ALL_DESKTOPS: GUID = GUID::from_u128(0xBB64D5B7_4DE3_4AB2_A87C_DB7601AEA7DC);
const WINDOW_ON_ALL_DESKTOPS: GUID = GUID::from_u128(0xC2DDEA68_66F2_4CF9_8264_1BFD00FBBBAC);

/// The part of `IVirtualDesktop` that this application uses.
///
/// Win11 adds `GetName`, `GetWallpaperPath`, and `IsRemote` after `GetId`, but
/// a vtable prefix is valid as long as it matches the beginning of the real
/// table, so the extra methods do not need to be declared.
#[repr(C)]
#[allow(non_snake_case, dead_code)]
pub(crate) struct DesktopVtable {
    pub base__: IUnknown_Vtbl,
    pub IsViewVisible: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut BOOL) -> HRESULT,
    pub GetId: unsafe extern "system" fn(*mut c_void, *mut GUID) -> HRESULT,
}

/// Behavior shared by every desktop wrapper type.
///
/// The blanket implementation applies to any type whose vtable is
/// [`DesktopVtable`], so the layout modules do not need to repeat `GetId`.
pub(crate) trait DesktopRef: Interface<Vtable = DesktopVtable> + Clone {
    fn id(&self) -> Result<GUID, windows::core::Error> {
        let mut id = GUID::zeroed();
        unsafe { (self.vtable().GetId)(self.as_raw(), &mut id) }.ok()?;
        Ok(id)
    }
}

impl<T> DesktopRef for T where T: Interface<Vtable = DesktopVtable> + Clone {}

/// The part of `IVirtualDesktopManagerInternal` that this application uses.
///
/// The first seven methods after `IUnknown` have the same order in the Win10
/// and Win11 layouts. Later methods differ (Win11 adds
/// `SwitchDesktopAndMoveForegroundView`), so this prefix stops at
/// `SwitchDesktop`.
#[repr(C)]
#[allow(non_snake_case, dead_code)]
pub(crate) struct ManagerVtable {
    pub base__: IUnknown_Vtbl,
    pub GetCount: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
    pub MoveViewToDesktop:
        unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void) -> HRESULT,
    pub CanViewMoveDesktops:
        unsafe extern "system" fn(*mut c_void, *mut c_void, *mut BOOL) -> HRESULT,
    pub GetCurrentDesktop: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    pub GetDesktops: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    pub GetAdjacentDesktop:
        unsafe extern "system" fn(*mut c_void, *mut c_void, i32, *mut *mut c_void) -> HRESULT,
    pub SwitchDesktop: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
}

/// Behavior shared by every manager wrapper type.
///
/// This trait has default methods, so the layout modules only supply the
/// associated [`DesktopRef`] type and the two GUIDs. The `Interface<Vtable =
/// ManagerVtable>` bound is what makes `self.vtable().GetCount` valid.
pub(crate) trait ManagerInternal<D: DesktopRef>:
    Interface<Vtable = ManagerVtable> + Clone
{
    fn count(&self) -> Result<u32, windows::core::Error> {
        // The native method is HRESULT GetCount(int*), not an int return.
        let mut count = 0i32;
        unsafe { (self.vtable().GetCount)(self.as_raw(), &mut count) }.ok()?;
        Ok(count.max(0) as u32)
    }

    fn desktops(&self) -> Result<Option<IObjectArray>, windows::core::Error> {
        // The shell returns an IObjectArray; `from_raw` takes ownership of the
        // returned reference.
        let mut raw = null_mut();
        unsafe { (self.vtable().GetDesktops)(self.as_raw(), &mut raw) }.ok()?;
        if raw.is_null() {
            Ok(None)
        } else {
            Ok(Some(unsafe { IObjectArray::from_raw(raw) }))
        }
    }

    fn current_desktop(&self) -> Result<Option<D>, windows::core::Error> {
        let mut raw = null_mut();
        unsafe { (self.vtable().GetCurrentDesktop)(self.as_raw(), &mut raw) }.ok()?;
        if raw.is_null() {
            Ok(None)
        } else {
            Ok(Some(unsafe { D::from_raw(raw) }))
        }
    }

    fn switch_desktop(&self, desktop: &D) -> Result<(), windows::core::Error> {
        unsafe { (self.vtable().SwitchDesktop)(self.as_raw(), desktop.as_raw()) }.ok()
    }

    fn move_view_to_desktop(
        &self,
        view: &IApplicationView,
        desktop: &D,
    ) -> Result<(), windows::core::Error> {
        unsafe { (self.vtable().MoveViewToDesktop)(self.as_raw(), view.as_raw(), desktop.as_raw()) }
            .ok()
    }
}

impl<T, D> ManagerInternal<D> for T
where
    T: Interface<Vtable = ManagerVtable> + Clone,
    D: DesktopRef,
{
}

/// The `IApplicationView` interface for a window.
///
/// `IApplicationView` derives from `IInspectable`, not directly from
/// `IUnknown`. Its vtable therefore has `GetIids`, `GetRuntimeClassName`, and
/// `GetTrustLevel` between `IUnknown` and `SetFocus`. Those three entries must
/// be declared even though this application only calls `GetAppUserModelId`,
/// otherwise every later method would be read from the wrong slot.
#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct IApplicationView(IUnknown);

#[repr(C)]
#[allow(non_snake_case, dead_code)]
pub(crate) struct IApplicationView_Vtbl {
    base__: IUnknown_Vtbl,
    GetIids: unsafe extern "system" fn(*mut c_void, *mut u32, *mut *mut GUID) -> HRESULT,
    GetRuntimeClassName: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    GetTrustLevel: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
    SetFocus: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    SwitchTo: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    TryInvokeBack: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    GetThumbnailWindow: unsafe extern "system" fn(*mut c_void, *mut HWND) -> HRESULT,
    GetMonitor: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    GetVisibility: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
    SetCloak: unsafe extern "system" fn(*mut c_void, i32, i32) -> HRESULT,
    GetPosition: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    SetPosition: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    InsertAfterWindow: unsafe extern "system" fn(*mut c_void, HWND) -> HRESULT,
    GetExtendedFramePosition:
        unsafe extern "system" fn(*mut c_void, *mut windows::Win32::Foundation::RECT) -> HRESULT,
    GetAppUserModelId: unsafe extern "system" fn(*mut c_void, *mut PWSTR) -> HRESULT,
}

// SAFETY: the IID is the shell's IApplicationView IID and the vtable prefix
// above matches the real table, starting with IUnknown and IInspectable.
unsafe impl Interface for IApplicationView {
    type Vtable = IApplicationView_Vtbl;
    const IID: GUID = GUID::from_u128(0x372E1D3B_38D3_42E4_A15B_8AB2B178F513);
}

impl IApplicationView {
    fn app_user_model_id(&self) -> Result<PWSTR, windows::core::Error> {
        let mut id = PWSTR::null();
        unsafe { (self.vtable().GetAppUserModelId)(self.as_raw(), &mut id) }.ok()?;
        Ok(id)
    }
}

/// Maps a window handle to its `IApplicationView`.
#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct IApplicationViewCollection(IUnknown);

#[repr(C)]
#[allow(non_snake_case, dead_code)]
pub(crate) struct IApplicationViewCollection_Vtbl {
    base__: IUnknown_Vtbl,
    GetViews: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    GetViewsByZOrder: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    GetViewsByAppUserModelId:
        unsafe extern "system" fn(*mut c_void, PCWSTR, *mut *mut c_void) -> HRESULT,
    GetViewForHwnd: unsafe extern "system" fn(*mut c_void, HWND, *mut *mut c_void) -> HRESULT,
}

// SAFETY: the IID and the four-method prefix match the shell interface.
unsafe impl Interface for IApplicationViewCollection {
    type Vtable = IApplicationViewCollection_Vtbl;
    const IID: GUID = GUID::from_u128(0x1841C6D7_4F9D_42C0_AF41_8747538F10E5);
}

impl IApplicationViewCollection {
    fn view_for_hwnd(&self, hwnd: HWND) -> Result<Option<IApplicationView>, windows::core::Error> {
        let mut raw = null_mut();
        unsafe { (self.vtable().GetViewForHwnd)(self.as_raw(), hwnd, &mut raw) }.ok()?;
        if raw.is_null() {
            Ok(None)
        } else {
            Ok(Some(unsafe { IApplicationView::from_raw(raw) }))
        }
    }
}

/// Pins applications or views to all virtual desktops.
#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct IVirtualDesktopPinnedApps(IUnknown);

#[repr(C)]
#[allow(non_snake_case, dead_code)]
pub(crate) struct IVirtualDesktopPinnedApps_Vtbl {
    base__: IUnknown_Vtbl,
    IsAppIdPinned: unsafe extern "system" fn(*mut c_void, PCWSTR, *mut BOOL) -> HRESULT,
    PinAppID: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    UnpinAppID: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    IsViewPinned: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut BOOL) -> HRESULT,
    PinView: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    UnpinView: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
}

// SAFETY: the IID and the six-method vtable match the shell interface.
unsafe impl Interface for IVirtualDesktopPinnedApps {
    type Vtable = IVirtualDesktopPinnedApps_Vtbl;
    const IID: GUID = GUID::from_u128(0x4CE81583_1E4C_4632_A621_07A53543148F);
}

impl IVirtualDesktopPinnedApps {
    fn is_app_id_pinned(&self, app_id: PCWSTR) -> Result<bool, windows::core::Error> {
        let mut pinned = BOOL(0);
        unsafe { (self.vtable().IsAppIdPinned)(self.as_raw(), app_id, &mut pinned) }.ok()?;
        Ok(pinned.as_bool())
    }

    fn pin_app_id(&self, app_id: PCWSTR) -> Result<(), windows::core::Error> {
        unsafe { (self.vtable().PinAppID)(self.as_raw(), app_id) }.ok()
    }

    fn unpin_app_id(&self, app_id: PCWSTR) -> Result<(), windows::core::Error> {
        unsafe { (self.vtable().UnpinAppID)(self.as_raw(), app_id) }.ok()
    }
}

/// The generic backend shared by the Win10 and Win11 layouts.
///
/// `M` is the manager wrapper and `D` is the desktop wrapper. Both are phantom
/// types to this struct in the sense that they only carry COM pointers and
/// GUIDs; the method bodies live here.
pub(crate) struct VirtualDesktop<M, D>
where
    M: ManagerInternal<D>,
    D: DesktopRef,
{
    _shell: IServiceProvider,
    manager_private: IVirtualDesktopManager,
    views: IApplicationViewCollection,
    internal: M,
    pinned: IVirtualDesktopPinnedApps,
    _desktop: PhantomData<fn() -> D>,
}

impl<M, D> VirtualDesktop<M, D>
where
    M: ManagerInternal<D>,
    D: DesktopRef,
{
    /// Stores the shell objects that were opened by [`open_with`].
    pub(crate) fn new(
        shell: IServiceProvider,
        manager_private: IVirtualDesktopManager,
        views: IApplicationViewCollection,
        internal: M,
        pinned: IVirtualDesktopPinnedApps,
    ) -> Self {
        Self {
            _shell: shell,
            manager_private,
            views,
            internal,
            pinned,
            _desktop: PhantomData,
        }
    }

    /// Returns the desktop at `index`, with a range check.
    fn desktop_at(&self, index: u32) -> Result<D, ApiError> {
        let desktops = self
            .internal
            .desktops()?
            .ok_or(ApiError::UnexpectedNull("desktop array"))?;
        let count = unsafe { desktops.GetCount()? };

        if index >= count {
            return Err(ApiError::DesktopOutOfRange { index, count });
        }

        unsafe { desktops.GetAt(index) }.map_err(ApiError::from)
    }

    /// Maps a window handle to its application view.
    fn view_for_hwnd(&self, hwnd: HWND) -> Result<Option<IApplicationView>, ApiError> {
        Ok(self.views.view_for_hwnd(hwnd)?)
    }

    /// Reads the application user model ID and wraps it in an owner that frees
    /// the COM-allocated string when it leaves scope.
    fn app_user_model_id(&self, hwnd: HWND) -> Result<CoTaskMemString, ApiError> {
        let view = self
            .view_for_hwnd(hwnd)?
            .ok_or(ApiError::WindowNotFound(hwnd))?;
        Ok(CoTaskMemString(view.app_user_model_id()?))
    }
}

impl<M, D> VirtualDesktopApi for VirtualDesktop<M, D>
where
    M: ManagerInternal<D>,
    D: DesktopRef,
{
    fn count(&self) -> Result<u32, ApiError> {
        Ok(self.internal.count()?)
    }

    fn current_index(&self) -> Result<u32, ApiError> {
        let current = self
            .internal
            .current_desktop()?
            .ok_or(ApiError::UnexpectedNull("current desktop"))?;
        let id = current.id()?;
        let ids = self.desktop_ids()?;

        ids.iter()
            .position(|candidate| *candidate == id)
            .map(|index| index as u32)
            .ok_or(ApiError::DesktopNotFound)
    }

    fn desktop_ids(&self) -> Result<Vec<GUID>, ApiError> {
        let desktops = self
            .internal
            .desktops()?
            .ok_or(ApiError::UnexpectedNull("desktop array"))?;
        let count = unsafe { desktops.GetCount()? };
        let mut ids = Vec::with_capacity(count as usize);

        for index in 0..count {
            // `GetAt<D>` performs the QueryInterface for `D::IID` internally,
            // so the result is already the right desktop wrapper type.
            let desktop: D = unsafe { desktops.GetAt(index)? };
            ids.push(desktop.id()?);
        }

        Ok(ids)
    }

    fn index_of_window(&self, hwnd: HWND) -> Result<Option<u32>, ApiError> {
        let id = unsafe { self.manager_private.GetWindowDesktopId(hwnd)? };

        if id == GUID::zeroed() {
            return Ok(None);
        }

        if id == APP_ON_ALL_DESKTOPS || id == WINDOW_ON_ALL_DESKTOPS {
            // Pinned windows do not live on one desktop. For pager purposes
            // they follow the current desktop.
            return Ok(Some(self.current_index()?));
        }

        let ids = self.desktop_ids()?;

        Ok(ids
            .iter()
            .position(|candidate| *candidate == id)
            .map(|index| index as u32))
    }

    fn switch_to(&self, index: u32) -> Result<(), ApiError> {
        let desktop = self.desktop_at(index)?;
        self.internal.switch_desktop(&desktop)?;
        Ok(())
    }

    fn move_window_to(&self, hwnd: HWND, index: u32) -> Result<(), ApiError> {
        let view = self
            .view_for_hwnd(hwnd)?
            .ok_or(ApiError::WindowNotFound(hwnd))?;
        let desktop = self.desktop_at(index)?;
        self.internal.move_view_to_desktop(&view, &desktop)?;
        Ok(())
    }

    fn is_application_pinned(&self, hwnd: HWND) -> Result<bool, ApiError> {
        let app_id = self.app_user_model_id(hwnd)?;
        if app_id.0.0.is_null() {
            return Err(ApiError::UnexpectedNull("app user model id"));
        }

        Ok(self.pinned.is_app_id_pinned(PCWSTR::from_raw(app_id.0.0))?)
    }

    fn pin_application(&self, hwnd: HWND) -> Result<PinOutcome, ApiError> {
        // Pinning works by application ID, not by HWND. `app_user_model_id`
        // returns a COM-allocated string that `CoTaskMemString` frees.
        let app_id = self.app_user_model_id(hwnd)?;
        if app_id.0.0.is_null() {
            return Err(ApiError::UnexpectedNull("app user model id"));
        }

        let app_id = PCWSTR::from_raw(app_id.0.0);
        match self.pinned.is_app_id_pinned(app_id)? {
            true => Ok(PinOutcome::AlreadyPinned),
            false => {
                self.pinned.pin_app_id(app_id)?;
                Ok(PinOutcome::Pinned)
            }
        }
    }

    fn unpin_application(&self, hwnd: HWND) -> Result<(), ApiError> {
        let app_id = self.app_user_model_id(hwnd)?;
        if app_id.0.0.is_null() {
            return Err(ApiError::UnexpectedNull("app user model id"));
        }

        let app_id = PCWSTR::from_raw(app_id.0.0);
        if self.pinned.is_app_id_pinned(app_id)? {
            self.pinned.unpin_app_id(app_id)?;
        }

        Ok(())
    }
}

/// Owns a string allocated by COM.
///
/// `GetAppUserModelId` returns memory allocated with `CoTaskMemAlloc`. Rust
/// cannot free it with `drop`; the `Drop` implementation calls
/// `CoTaskMemFree`. This is the RAII pattern used to avoid leaks on early
/// returns.
struct CoTaskMemString(PWSTR);

impl Drop for CoTaskMemString {
    fn drop(&mut self) {
        if !self.0.0.is_null() {
            unsafe {
                CoTaskMemFree(Some(self.0.0 as *const c_void));
            }
        }
    }
}

/// Opens the shell objects and wires them into a [`VirtualDesktop`].
///
/// The layout module chooses `M` and `D`; their `Interface` implementations
/// supply the IIDs. The service CLSIDs are the same for both layouts, which is
/// why they live here.
pub(crate) fn open_with<M, D>() -> Result<VirtualDesktop<M, D>, ApiError>
where
    M: ManagerInternal<D>,
    D: DesktopRef,
{
    unsafe {
        // `CoCreateInstance` asks COM to create an object from a class ID.
        // `None::<&IUnknown>` means "no aggregation"; the type annotation is
        // needed because `None` alone does not say which pointer type it is.
        crate::logging::info("creating immersive shell service provider");
        let shell: IServiceProvider =
            CoCreateInstance(&CLSID_IMMERSIVE_SHELL, None::<&IUnknown>, CLSCTX_ALL)?;
        crate::logging::info("creating public virtual desktop manager");
        let manager_private: IVirtualDesktopManager = CoCreateInstance(
            &CLSID_VIRTUAL_DESKTOP_MANAGER,
            None::<&IUnknown>,
            CLSCTX_ALL,
        )?;
        crate::logging::info("querying application view collection");
        let views =
            shell.QueryService::<IApplicationViewCollection>(&IApplicationViewCollection::IID)?;
        // `QueryService` takes the service CLSID (C5E0CDCA...) and uses
        // `M::IID` as the requested interface. Passing the interface IID as the
        // service GUID returns E_NOTIMPL, so these two GUIDs must not be
        // confused.
        crate::logging::info("querying internal virtual desktop manager");
        let internal = shell.QueryService::<M>(&CLSID_VIRTUAL_DESKTOP_MANAGER_INTERNAL)?;
        crate::logging::info("querying pinned apps");
        let pinned =
            shell.QueryService::<IVirtualDesktopPinnedApps>(&CLSID_VIRTUAL_DESKTOP_PINNED_APPS)?;

        Ok(VirtualDesktop::new(
            shell,
            manager_private,
            views,
            internal,
            pinned,
        ))
    }
}
