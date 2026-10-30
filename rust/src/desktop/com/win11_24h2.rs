use std::ffi::c_void;
use std::ptr::null_mut;

use windows::Win32::Foundation::{HWND, RECT};
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

#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, Debug)]
struct IApplicationView(IUnknown);

#[repr(C)]
#[allow(non_snake_case, dead_code)]
struct IApplicationView_Vtbl {
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
    GetExtendedFramePosition: unsafe extern "system" fn(*mut c_void, *mut RECT) -> HRESULT,
    GetAppUserModelId: unsafe extern "system" fn(*mut c_void, *mut PWSTR) -> HRESULT,
}

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

#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, Debug)]
struct IApplicationViewCollection(IUnknown);

#[repr(C)]
#[allow(non_snake_case, dead_code)]
struct IApplicationViewCollection_Vtbl {
    base__: IUnknown_Vtbl,
    GetViews: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    GetViewsByZOrder: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    GetViewsByAppUserModelId:
        unsafe extern "system" fn(*mut c_void, PCWSTR, *mut *mut c_void) -> HRESULT,
    GetViewForHwnd: unsafe extern "system" fn(*mut c_void, HWND, *mut *mut c_void) -> HRESULT,
}

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

#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, Debug)]
struct IVirtualDesktopPinnedApps(IUnknown);

#[repr(C)]
#[allow(non_snake_case, dead_code)]
struct IVirtualDesktopPinnedApps_Vtbl {
    base__: IUnknown_Vtbl,
    IsAppIdPinned: unsafe extern "system" fn(*mut c_void, PCWSTR, *mut BOOL) -> HRESULT,
    PinAppID: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    UnpinAppID: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    IsViewPinned: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut BOOL) -> HRESULT,
    PinView: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    UnpinView: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
}

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

    #[allow(dead_code)]
    fn is_view_pinned(&self, view: &IApplicationView) -> Result<bool, windows::core::Error> {
        let mut pinned = BOOL(0);
        unsafe { (self.vtable().IsViewPinned)(self.as_raw(), view.as_raw(), &mut pinned) }.ok()?;
        Ok(pinned.as_bool())
    }

    #[allow(dead_code)]
    fn pin_view(&self, view: &IApplicationView) -> Result<(), windows::core::Error> {
        unsafe { (self.vtable().PinView)(self.as_raw(), view.as_raw()) }.ok()
    }
}

#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, Debug)]
struct IVirtualDesktop(IUnknown);

#[repr(C)]
#[allow(non_snake_case, dead_code)]
struct IVirtualDesktop_Vtbl {
    base__: IUnknown_Vtbl,
    IsViewVisible: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut BOOL) -> HRESULT,
    GetId: unsafe extern "system" fn(*mut c_void, *mut GUID) -> HRESULT,
    GetName: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    GetWallpaperPath: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    IsRemote: unsafe extern "system" fn(*mut c_void, *mut BOOL) -> HRESULT,
}

unsafe impl Interface for IVirtualDesktop {
    type Vtable = IVirtualDesktop_Vtbl;
    const IID: GUID = GUID::from_u128(0x3F07F4BE_B107_441A_AF0F_39D82529072C);
}

impl IVirtualDesktop {
    fn id(&self) -> Result<GUID, windows::core::Error> {
        let mut id = GUID::zeroed();
        unsafe { (self.vtable().GetId)(self.as_raw(), &mut id) }.ok()?;
        Ok(id)
    }
}

#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, Debug)]
struct IVirtualDesktopManagerInternal(IUnknown);

#[repr(C)]
#[allow(non_snake_case, dead_code)]
struct IVirtualDesktopManagerInternal_Vtbl {
    base__: IUnknown_Vtbl,
    GetCount: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
    MoveViewToDesktop: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void) -> HRESULT,
    CanViewMoveDesktops: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut BOOL) -> HRESULT,
    GetCurrentDesktop: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    GetDesktops: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    GetAdjacentDesktop:
        unsafe extern "system" fn(*mut c_void, *mut c_void, i32, *mut *mut c_void) -> HRESULT,
    SwitchDesktop: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    SwitchDesktopAndMoveForegroundView:
        unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
}

unsafe impl Interface for IVirtualDesktopManagerInternal {
    type Vtable = IVirtualDesktopManagerInternal_Vtbl;
    const IID: GUID = GUID::from_u128(0x53F5CA0B_158F_4124_900C_057158060B27);
}

impl IVirtualDesktopManagerInternal {
    fn count(&self) -> Result<u32, windows::core::Error> {
        let mut count = 0i32;
        unsafe { (self.vtable().GetCount)(self.as_raw(), &mut count) }.ok()?;
        Ok(count.max(0) as u32)
    }

    fn desktops(&self) -> Result<Option<IObjectArray>, windows::core::Error> {
        let mut raw = null_mut();
        unsafe { (self.vtable().GetDesktops)(self.as_raw(), &mut raw) }.ok()?;
        if raw.is_null() {
            Ok(None)
        } else {
            Ok(Some(unsafe { IObjectArray::from_raw(raw) }))
        }
    }

    fn current_desktop(&self) -> Result<Option<IVirtualDesktop>, windows::core::Error> {
        let mut raw = null_mut();
        unsafe { (self.vtable().GetCurrentDesktop)(self.as_raw(), &mut raw) }.ok()?;
        if raw.is_null() {
            Ok(None)
        } else {
            Ok(Some(unsafe { IVirtualDesktop::from_raw(raw) }))
        }
    }

    fn switch_desktop(&self, desktop: &IVirtualDesktop) -> Result<(), windows::core::Error> {
        unsafe { (self.vtable().SwitchDesktop)(self.as_raw(), desktop.as_raw()) }.ok()
    }

    fn move_view_to_desktop(
        &self,
        view: &IApplicationView,
        desktop: &IVirtualDesktop,
    ) -> Result<(), windows::core::Error> {
        unsafe { (self.vtable().MoveViewToDesktop)(self.as_raw(), view.as_raw(), desktop.as_raw()) }
            .ok()
    }
}

pub struct VirtualDesktop {
    _shell: IServiceProvider,
    manager_private: IVirtualDesktopManager,
    views: IApplicationViewCollection,
    internal: IVirtualDesktopManagerInternal,
    pinned: IVirtualDesktopPinnedApps,
}

impl VirtualDesktop {
    pub fn open() -> Result<Self, ApiError> {
        unsafe {
            let shell: IServiceProvider =
                CoCreateInstance(&CLSID_IMMERSIVE_SHELL, None::<&IUnknown>, CLSCTX_ALL)?;
            let manager_private: IVirtualDesktopManager = CoCreateInstance(
                &CLSID_VIRTUAL_DESKTOP_MANAGER,
                None::<&IUnknown>,
                CLSCTX_ALL,
            )?;
            let views = shell
                .QueryService::<IApplicationViewCollection>(&IApplicationViewCollection::IID)?;
            let internal = shell.QueryService::<IVirtualDesktopManagerInternal>(
                &CLSID_VIRTUAL_DESKTOP_MANAGER_INTERNAL,
            )?;
            let pinned = shell
                .QueryService::<IVirtualDesktopPinnedApps>(&CLSID_VIRTUAL_DESKTOP_PINNED_APPS)?;

            Ok(Self {
                _shell: shell,
                manager_private,
                views,
                internal,
                pinned,
            })
        }
    }

    fn desktop_at(&self, index: u32) -> Result<IVirtualDesktop, ApiError> {
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

    fn view_for_hwnd(&self, hwnd: HWND) -> Result<Option<IApplicationView>, ApiError> {
        Ok(self.views.view_for_hwnd(hwnd)?)
    }
}

impl VirtualDesktopApi for VirtualDesktop {
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
            let desktop: IVirtualDesktop = unsafe { desktops.GetAt(index)? };
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
        let view = self
            .view_for_hwnd(hwnd)?
            .ok_or(ApiError::WindowNotFound(hwnd))?;
        let app_id = view.app_user_model_id()?;

        if app_id.is_null() {
            return Err(ApiError::UnexpectedNull("app user model id"));
        }

        let result = self.pinned.is_app_id_pinned(PCWSTR::from_raw(app_id.0));
        unsafe {
            CoTaskMemFree(Some(app_id.0 as *const c_void));
        }
        Ok(result?)
    }

    fn pin_application(&self, hwnd: HWND) -> Result<PinOutcome, ApiError> {
        let view = self
            .view_for_hwnd(hwnd)?
            .ok_or(ApiError::WindowNotFound(hwnd))?;
        let app_id = view.app_user_model_id()?;

        if app_id.is_null() {
            return Err(ApiError::UnexpectedNull("app user model id"));
        }

        let result = match self.pinned.is_app_id_pinned(PCWSTR::from_raw(app_id.0)) {
            Ok(true) => Ok(PinOutcome::AlreadyPinned),
            Ok(false) => self
                .pinned
                .pin_app_id(PCWSTR::from_raw(app_id.0))
                .map(|()| PinOutcome::Pinned),
            Err(error) => Err(error),
        };

        unsafe {
            CoTaskMemFree(Some(app_id.0 as *const c_void));
        }
        Ok(result?)
    }
}
