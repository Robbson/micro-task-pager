//! Native About dialog.
//!
//! The dialog mirrors the reference WinForms `AboutDialog`: a fixed 600x480
//! client area, the embedded logo, the version, author, acknowledgments, and a
//! centered Close button. It is a real modal dialog, so the owner is disabled
//! and Enter/Escape close it through the dialog manager.
//!
//! The logo is embedded as an RCDATA PNG and decoded with WIC. The rest of the
//! dialog uses standard static and button controls, which keeps the dialog
//! responsive to system DPI scaling without adding a GUI framework.

use std::iter::once;
use std::mem::size_of;

use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleDC, CreateDIBSection, CreateFontW,
    CreateSolidBrush, DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, DeleteDC, DeleteObject,
    FW_BOLD, FW_NORMAL, HBITMAP, HBRUSH, HFONT, HGDIOBJ, OUT_DEFAULT_PRECIS, SelectObject,
    SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICBitmapSource, IWICImagingFactory,
    IWICPalette, WICBitmapDitherTypeNone, WICBitmapInterpolationModeHighQualityCubic,
    WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnLoad,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::LibraryLoader::{
    FindResourceW, GetModuleHandleW, LoadResource, LockResource, SizeofResource,
};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, BS_DEFPUSHBUTTON, CreateWindowExW, DI_NORMAL, DLGTEMPLATE, DS_MODALFRAME,
    DS_NOIDLEMSG, DS_SETFONT, DialogBoxIndirectParamW, EndDialog, GWL_EXSTYLE, GWL_STYLE,
    GetClientRect, GetSystemMetrics, GetWindowLongPtrW, HICON, HMENU, ICON_BIG, ICON_SMALL,
    IDCANCEL, IDOK, IMAGE_ICON, LR_DEFAULTCOLOR, LR_SHARED, LoadImageW, SM_CXSCREEN, SM_CYSCREEN,
    SWP_NOACTIVATE, SWP_NOZORDER, SendMessageW, SetWindowLongPtrW, SetWindowPos, WINDOW_EX_STYLE,
    WINDOW_LONG_PTR_INDEX, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CTLCOLORSTATIC, WM_DESTROY,
    WM_ERASEBKGND, WM_INITDIALOG, WM_PAINT, WM_SETFONT, WM_SETICON, WS_CAPTION, WS_CHILD,
    WS_CLIPCHILDREN, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{Error as CoreError, Interface, PCWSTR, w};

use crate::logging;
use crate::render::scale;

const LOGO_RESOURCE: u16 = 101;
const ICON_RESOURCE: u16 = 1;
const RT_RCDATA: u16 = 10;
const SS_CENTER: u32 = 0x0000_0001;
const SS_CENTERIMAGE: u32 = 0x0000_0200;
const SS_LEFT: u32 = 0x0000_0000;
const IDOK_VALUE: i32 = IDOK.0;
const IDCANCEL_VALUE: i32 = IDCANCEL.0;
const CLIENT_WIDTH: i32 = 600;
const CLIENT_HEIGHT: i32 = 480;
const LOGO_WIDTH: i32 = 512;
const LOGO_HEIGHT: i32 = 278;
const DIM_GRAY: COLORREF = COLORREF(0x0069_6969);

/// Dialog user data lives after `DWLP_MSGRESULT` and `DWLP_DLGPROC`.
const DWLP_USER: WINDOW_LONG_PTR_INDEX = WINDOW_LONG_PTR_INDEX((size_of::<isize>() * 2) as i32);

/// Shows the modal About dialog owned by `owner`.
pub fn show(owner: HWND, dpi: u32) -> Result<(), CoreError> {
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(PCWSTR::null())?.0);
        let template = dialog_template();
        let result = DialogBoxIndirectParamW(
            Some(instance),
            template.0.as_ptr().cast::<DLGTEMPLATE>(),
            Some(owner),
            Some(about_proc),
            LPARAM(dpi as isize),
        );

        if result == -1 {
            Err(CoreError::from_thread())
        } else {
            Ok(())
        }
    }
}

/// A minimal `DLGTEMPLATE` with no controls. Controls are created in
/// `WM_INITDIALOG` so their positions can be computed in physical pixels.
#[repr(C, align(4))]
struct DialogTemplate([u8; 128]);

fn dialog_template() -> DialogTemplate {
    let style = WS_POPUP.0
        | WS_CAPTION.0
        | WS_SYSMENU.0
        | WS_CLIPCHILDREN.0
        | (DS_MODALFRAME as u32)
        | (DS_SETFONT as u32)
        | (DS_NOIDLEMSG as u32);

    let mut template = DialogTemplate([0; 128]);
    let bytes = &mut template.0;

    write_u32(bytes, 0, style);
    write_u32(bytes, 4, 0);
    write_u16(bytes, 8, 0);
    write_u16(bytes, 10, 0);
    write_u16(bytes, 12, 0);
    write_u16(bytes, 14, 1);
    write_u16(bytes, 16, 1);

    let mut offset = 18;
    write_u16(bytes, offset, 0);
    offset += 2;
    write_u16(bytes, offset, 0);
    offset += 2;

    for unit in "About Micro Task Pager".encode_utf16().chain(once(0)) {
        write_u16(bytes, offset, unit);
        offset += 2;
    }

    write_u16(bytes, offset, 9);
    offset += 2;
    for unit in "Segoe UI".encode_utf16().chain(once(0)) {
        write_u16(bytes, offset, unit);
        offset += 2;
    }

    template
}

fn write_u16(buffer: &mut [u8], offset: usize, value: u16) {
    buffer[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(buffer: &mut [u8], offset: usize, value: u32) {
    buffer[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

unsafe extern "system" fn about_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    match message {
        WM_INITDIALOG => unsafe {
            let dpi = lparam.0 as u32;
            let state = Box::new(AboutState::new(dpi));
            let raw = Box::into_raw(state);
            SetWindowLongPtrW(hwnd, DWLP_USER, raw as isize);
            init_dialog(hwnd, &mut *raw, dpi);
            1
        },
        WM_COMMAND => unsafe {
            let id = (wparam.0 & 0xFFFF) as i32;
            if id == IDOK_VALUE || id == IDCANCEL_VALUE {
                let _ = EndDialog(hwnd, 1);
                1
            } else {
                0
            }
        },
        WM_CLOSE => unsafe {
            let _ = EndDialog(hwnd, 0);
            1
        },
        WM_ERASEBKGND => unsafe {
            let state = state(hwnd);
            if let Some(state) = state {
                let mut rect = RECT::default();
                if GetClientRect(hwnd, &mut rect).is_ok() {
                    let _ = windows::Win32::Graphics::Gdi::FillRect(
                        windows::Win32::Graphics::Gdi::HDC(wparam.0 as _),
                        &rect,
                        state.white_brush,
                    );
                }
            }
            1
        },
        WM_PAINT => unsafe {
            paint(hwnd);
            1
        },
        WM_CTLCOLORSTATIC => unsafe {
            let state = match state(hwnd) {
                Some(state) => state,
                None => return 0,
            };
            let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as _);
            let control = HWND(lparam.0 as _);
            let _ = SetBkMode(hdc, TRANSPARENT);
            let color = if control == state.version_hwnd {
                DIM_GRAY
            } else {
                COLORREF(0)
            };
            let _ = SetTextColor(hdc, color);
            state.white_brush.0 as isize
        },
        WM_DESTROY => unsafe {
            let ptr = GetWindowLongPtrW(hwnd, DWLP_USER);
            if ptr != 0 {
                SetWindowLongPtrW(hwnd, DWLP_USER, 0);
                drop(Box::from_raw(ptr as *mut AboutState));
            }
            1
        },
        _ => 0,
    }
}

unsafe fn state(hwnd: HWND) -> Option<&'static AboutState> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, DWLP_USER) };
    if ptr == 0 {
        None
    } else {
        Some(unsafe { &*(ptr as *const AboutState) })
    }
}

unsafe fn init_dialog(hwnd: HWND, state: &mut AboutState, dpi: u32) {
    unsafe {
        let client_width = scale(CLIENT_WIDTH, dpi);
        let client_height = scale(CLIENT_HEIGHT, dpi);
        let style = WINDOW_STYLE(GetWindowLongPtrW(hwnd, GWL_STYLE) as u32);
        let ex_style = WINDOW_EX_STYLE(GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32);
        let mut window_rect = RECT {
            left: 0,
            top: 0,
            right: client_width,
            bottom: client_height,
        };
        let _ = AdjustWindowRectEx(&mut window_rect, style, false, ex_style);
        let window_width = window_rect.right - window_rect.left;
        let window_height = window_rect.bottom - window_rect.top;
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);
        let _ = SetWindowPos(
            hwnd,
            None,
            (screen_width - window_width) / 2,
            (screen_height - window_height) / 2,
            window_width,
            window_height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );

        let version = format!("Version {}", env!("CARGO_PKG_VERSION"));
        state.version_hwnd = create_static(
            hwnd,
            state.instance,
            &version,
            SS_CENTER | SS_CENTERIMAGE,
            ControlRect {
                x: 0,
                y: scale(300, dpi),
                width: client_width,
                height: scale(24, dpi),
            },
            state.version_font,
        );
        let _ = create_static(
            hwnd,
            state.instance,
            "Author: Robbson 2026",
            SS_CENTER | SS_CENTERIMAGE,
            ControlRect {
                x: scale(20, dpi),
                y: scale(330, dpi),
                width: client_width - scale(40, dpi),
                height: scale(24, dpi),
            },
            state.author_font,
        );
        let _ = create_static(
            hwnd,
            state.instance,
            "Acknowledgments:\n • DARKGuy (original project)\n • MScholtes (Virtual Desktop)",
            SS_LEFT,
            ControlRect {
                x: scale(220, dpi),
                y: scale(360, dpi),
                width: client_width - scale(40, dpi),
                height: scale(60, dpi),
            },
            state.contributor_font,
        );

        let button_width = scale(100, dpi);
        let button_height = scale(30, dpi);
        if let Ok(button) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            w!("Close"),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | BS_DEFPUSHBUTTON as u32),
            (client_width - button_width) / 2,
            scale(425, dpi),
            button_width,
            button_height,
            Some(hwnd),
            Some(HMENU(IDOK_VALUE as isize as _)),
            Some(state.instance),
            None,
        ) {
            state.close_hwnd = button;
            let _ = SendMessageW(
                button,
                WM_SETFONT,
                Some(WPARAM(state.button_font.0 as usize)),
                Some(LPARAM(1)),
            );
        }

        if let Some(icon) = state.icon_large {
            let _ = SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(ICON_BIG as usize)),
                Some(LPARAM(icon.0 as isize)),
            );
        }
        if let Some(icon) = state.icon_small {
            let _ = SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(ICON_SMALL as usize)),
                Some(LPARAM(icon.0 as isize)),
            );
        }

        if !state.close_hwnd.0.is_null() {
            let _ = SetFocus(Some(state.close_hwnd));
        }
    }
}

struct ControlRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

unsafe fn create_static(
    parent: HWND,
    instance: HINSTANCE,
    text: &str,
    style: u32,
    rect: ControlRect,
    font: HFONT,
) -> HWND {
    let text = to_wide(text);
    let style = WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | style);
    let Ok(control) = (unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            PCWSTR(text.as_ptr()),
            style,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Some(parent),
            None,
            Some(instance),
            None,
        )
    }) else {
        return HWND::default();
    };

    if !font.0.is_null() {
        let _ = unsafe {
            SendMessageW(
                control,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            )
        };
    }
    control
}

unsafe fn paint(hwnd: HWND) {
    unsafe {
        let mut paint = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
        let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut paint);
        let mut rect = RECT::default();
        if GetClientRect(hwnd, &mut rect).is_ok()
            && let Some(state) = state(hwnd)
        {
            let _ = windows::Win32::Graphics::Gdi::FillRect(hdc, &rect, state.white_brush);

            if let Some(logo) = &state.logo {
                let width = rect.right - rect.left;
                let x = (width - logo.width) / 2;
                let y = scale(20, state.dpi);
                let memory = CreateCompatibleDC(Some(hdc));
                if !memory.0.is_null() {
                    let previous = SelectObject(memory, HGDIOBJ(logo.bitmap.0));
                    let blend = BLENDFUNCTION {
                        BlendOp: AC_SRC_OVER as u8,
                        BlendFlags: 0,
                        SourceConstantAlpha: 255,
                        AlphaFormat: AC_SRC_ALPHA as u8,
                    };
                    let _ = windows::Win32::Graphics::Gdi::AlphaBlend(
                        hdc,
                        x,
                        y,
                        logo.width,
                        logo.height,
                        memory,
                        0,
                        0,
                        logo.width,
                        logo.height,
                        blend,
                    );
                    let _ = SelectObject(memory, previous);
                    let _ = DeleteDC(memory);
                }
            } else if let Some(icon) = state.icon_large {
                let logo_height = scale(LOGO_HEIGHT, state.dpi);
                let icon_size = scale(128, state.dpi);
                let icon_x = (rect.right - icon_size) / 2;
                let icon_y = scale(20, state.dpi) + (logo_height - icon_size) / 2;
                let _ = windows::Win32::UI::WindowsAndMessaging::DrawIconEx(
                    hdc, icon_x, icon_y, icon, icon_size, icon_size, 0, None, DI_NORMAL,
                );
            }
        }
        let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &paint);
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(once(0)).collect()
}

struct AboutState {
    instance: HINSTANCE,
    dpi: u32,
    white_brush: HBRUSH,
    version_font: HFONT,
    author_font: HFONT,
    contributor_font: HFONT,
    button_font: HFONT,
    version_hwnd: HWND,
    close_hwnd: HWND,
    logo: Option<Logo>,
    icon_large: Option<HICON>,
    icon_small: Option<HICON>,
}

impl AboutState {
    fn new(dpi: u32) -> Self {
        unsafe {
            let instance = HINSTANCE(
                GetModuleHandleW(PCWSTR::null()).map_or(std::ptr::null_mut(), |module| module.0),
            );
            let white_brush = CreateSolidBrush(COLORREF(0x00FF_FFFF));
            let version_font = create_font(10, FW_NORMAL.0, dpi);
            let author_font = create_font(9, FW_BOLD.0, dpi);
            let contributor_font = create_font(9, FW_NORMAL.0, dpi);
            let button_font = create_font(9, FW_NORMAL.0, dpi);
            let icon_large = load_icon(instance, scale(128, dpi));
            let icon_small = load_icon(instance, scale(16, dpi));
            let logo = load_logo(dpi);

            Self {
                instance,
                dpi,
                white_brush,
                version_font,
                author_font,
                contributor_font,
                button_font,
                version_hwnd: HWND::default(),
                close_hwnd: HWND::default(),
                logo,
                icon_large,
                icon_small,
            }
        }
    }
}

impl Drop for AboutState {
    fn drop(&mut self) {
        unsafe {
            if !self.white_brush.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(self.white_brush.0));
            }
            for font in [
                self.version_font,
                self.author_font,
                self.contributor_font,
                self.button_font,
            ] {
                if !font.0.is_null() {
                    let _ = DeleteObject(HGDIOBJ(font.0));
                }
            }
        }
    }
}

unsafe fn create_font(points: i32, weight: u32, dpi: u32) -> HFONT {
    let height = -((points * dpi as i32 + 36) / 72).max(1);
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            weight as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32,
            w!("Segoe UI"),
        )
    }
}

unsafe fn load_icon(instance: HINSTANCE, size: i32) -> Option<HICON> {
    unsafe {
        LoadImageW(
            Some(instance),
            PCWSTR(ICON_RESOURCE as _),
            IMAGE_ICON,
            size,
            size,
            LR_DEFAULTCOLOR | LR_SHARED,
        )
        .ok()
        .map(|handle| HICON(handle.0))
    }
}

struct Logo {
    bitmap: HBITMAP,
    width: i32,
    height: i32,
}

impl Drop for Logo {
    fn drop(&mut self) {
        unsafe {
            if !self.bitmap.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            }
        }
    }
}

unsafe fn load_logo(dpi: u32) -> Option<Logo> {
    unsafe {
        let module = GetModuleHandleW(PCWSTR::null()).ok()?;
        let resource = FindResourceW(
            Some(module),
            PCWSTR(LOGO_RESOURCE as _),
            PCWSTR(RT_RCDATA as _),
        );
        if resource.0.is_null() {
            logging::error("about: logo resource not found");
            return None;
        }

        let handle = LoadResource(Some(module), resource).ok()?;
        let pointer = LockResource(handle);
        let length = SizeofResource(Some(module), resource) as usize;
        if pointer.is_null() || length == 0 {
            logging::error("about: logo resource is empty");
            return None;
        }

        let bytes = std::slice::from_raw_parts(pointer.cast::<u8>(), length);
        let stream = SHCreateMemStream(Some(bytes))?;
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;

        let width = scale(LOGO_WIDTH, dpi).max(1) as u32;
        let height = scale(LOGO_HEIGHT, dpi).max(1) as u32;
        let decoder = factory
            .CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnLoad)
            .ok()?;
        let frame = decoder.GetFrame(0).ok()?;
        let source: IWICBitmapSource = frame.cast().ok()?;

        let scaler = factory.CreateBitmapScaler().ok()?;
        scaler
            .Initialize(
                &source,
                width,
                height,
                WICBitmapInterpolationModeHighQualityCubic,
            )
            .ok()?;
        let scaled: IWICBitmapSource = scaler.cast().ok()?;

        let converter = factory.CreateFormatConverter().ok()?;
        converter
            .Initialize(
                &scaled,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None::<&IWICPalette>,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .ok()?;

        let stride = width * 4;
        let mut pixels = vec![0u8; (stride * height) as usize];
        converter
            .CopyPixels(std::ptr::null(), stride, &mut pixels)
            .ok()?;

        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            return None;
        }

        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len());
        Some(Logo {
            bitmap,
            width: width as i32,
            height: height as i32,
        })
    }
}
