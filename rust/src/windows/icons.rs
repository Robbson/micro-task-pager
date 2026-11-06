//! Icon lookup and a small per-window cache.
//!
//! The shell returns borrowed `HICON` handles; the application must not call
//! `DestroyIcon` on them, because the window or its class owns them. The
//! fallback icon (`IDI_APPLICATION`) is a shared system icon for the same
//! reason.
//!
//! Phase 3 stores the handle and detects icon changes. Phase 4 converts it to a
//! GDI bitmap when rendering; that is also when a sized bitmap cache matters.

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::hash::{Hash, Hasher};
use std::mem::size_of;

use windows::Win32::Foundation::{HGLOBAL, HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits,
    GetObjectW, HGDIOBJ, ReleaseDC,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppBGRA,
    IWICBitmapEncoder, IWICBitmapFrameEncode, IWICBitmapSource, IWICImagingFactory,
    WICBitmapEncoderNoCache,
};
use windows::Win32::System::Com::StructuredStorage::{
    CreateStreamOnHGlobal, GetHGlobalFromStream, IPropertyBag2,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::UI::WindowsAndMessaging::{
    GCLP_HICON, GetClassLongPtrW, GetIconInfo, HICON, ICON_SMALL2, ICONINFO, IDI_APPLICATION,
    LoadIconW, SMTO_ABORTIFHUNG, SMTO_BLOCK, SendMessageTimeoutW, WM_GETICON,
};
use windows::core::Interface;

/// A borrowed icon handle supplied by the shell.
///
/// The handle is shared and must not be destroyed by the pager. `Hash` is
/// implemented manually because `HICON` itself does not implement it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowIcon(HICON);

impl WindowIcon {
    /// Returns the raw handle for the renderer. Kept crate-private so the raw
    /// handle does not leak into the public API.
    pub(crate) fn handle(self) -> HICON {
        self.0
    }

    /// Encodes the icon as a PNG for a reactor image source.
    ///
    /// The shell icon is a shared handle, so this only reads it. The returned
    /// bytes own no native resources and can be cached across snapshots.
    pub(crate) fn png_bytes(self) -> Option<Vec<u8>> {
        encode_png(self.0)
    }
}

impl Hash for WindowIcon {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.0.0 as usize).hash(state);
    }
}

/// Per-window icon cache.
///
/// The source handle is queried on every snapshot, because a window can change
/// its icon. The cache keeps one entry per window so the renderer can reuse a
/// converted bitmap later, and `retain` removes entries for windows that are
/// gone.
#[derive(Default)]
pub(crate) struct IconCache {
    entries: HashMap<usize, Option<WindowIcon>>,
}

impl IconCache {
    /// Looks up the current icon and updates the cache entry.
    pub(crate) fn get_or_load(&mut self, hwnd: HWND) -> Option<WindowIcon> {
        let icon = load(hwnd);
        self.entries.insert(hwnd.0 as usize, icon);
        icon
    }

    /// Drops entries for windows that are no longer in the snapshot.
    pub(crate) fn retain(&mut self, active: &HashSet<usize>) {
        self.entries.retain(|hwnd, _| active.contains(hwnd));
    }
}

/// Encodes an icon's color bitmap as a PNG.
///
/// `GetIconInfo` returns device-dependent bitmaps that the caller must delete.
/// Alpha comes from the 32-bit color bitmap; older icons whose color bitmap has
/// no alpha derive it from the monochrome AND mask.
fn encode_png(icon: HICON) -> Option<Vec<u8>> {
    let mut info = ICONINFO::default();
    // SAFETY: `icon` is a live shell icon; `info` is a local initialized struct.
    unsafe {
        GetIconInfo(icon, &mut info).ok()?;
    }

    let result = encode_icon_bitmaps(&info);

    // SAFETY: `GetIconInfo` transfers ownership of both bitmaps to the caller.
    unsafe {
        if !info.hbmColor.0.is_null() {
            let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
        }
        if !info.hbmMask.0.is_null() {
            let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
        }
    }

    result
}

fn encode_icon_bitmaps(info: &ICONINFO) -> Option<Vec<u8>> {
    if info.hbmColor.0.is_null() {
        return None;
    }

    let mut bitmap = BITMAP::default();
    // SAFETY: the bitmap belongs to `info` and `bitmap` is sized for BITMAP.
    let copied = unsafe {
        GetObjectW(
            HGDIOBJ(info.hbmColor.0),
            size_of::<BITMAP>() as i32,
            Some(&mut bitmap as *mut BITMAP as *mut c_void),
        )
    };
    if copied == 0 || bitmap.bmWidth <= 0 || bitmap.bmHeight <= 0 {
        return None;
    }

    let width = bitmap.bmWidth;
    let height = bitmap.bmHeight;
    let mut pixels = icon_pixels(info.hbmColor, width, height)?;

    if pixels.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 0) {
        apply_mask(&mut pixels, info.hbmMask, width, height);
    }

    encode_bgra_png(width as u32, height as u32, &pixels)
}

fn icon_pixels(
    hbitmap: windows::Win32::Graphics::Gdi::HBITMAP,
    width: i32,
    height: i32,
) -> Option<Vec<u8>> {
    let hdc = unsafe { GetDC(None) };
    if hdc.0.is_null() {
        return None;
    }

    let stride = width as usize * 4;
    let mut pixels = vec![0u8; stride * height as usize];
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            // A negative height requests top-down rows, matching reactor images.
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: (stride * height as usize) as u32,
            ..Default::default()
        },
        ..Default::default()
    };

    // SAFETY: `pixels` is large enough for 32-bit top-down rows and `info`
    // describes that layout. `hdc` is released below.
    let lines = unsafe {
        GetDIBits(
            hdc,
            hbitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut c_void),
            &mut info,
            DIB_RGB_COLORS,
        )
    };
    // SAFETY: `hdc` came from `GetDC` on this thread.
    unsafe {
        let _ = ReleaseDC(None, hdc);
    }

    (lines != 0).then_some(pixels)
}

fn apply_mask(
    pixels: &mut [u8],
    mask: windows::Win32::Graphics::Gdi::HBITMAP,
    width: i32,
    height: i32,
) {
    if mask.0.is_null() {
        return;
    }

    let hdc = unsafe { GetDC(None) };
    if hdc.0.is_null() {
        return;
    }

    let mask_stride = (width as usize).div_ceil(32) * 4;
    let mut mask_bits = vec![0u8; mask_stride * height as usize];
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 1,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    // SAFETY: `mask_bits` matches the 1-bpp top-down layout in `info`.
    let lines = unsafe {
        GetDIBits(
            hdc,
            mask,
            0,
            height as u32,
            Some(mask_bits.as_mut_ptr() as *mut c_void),
            &mut info,
            DIB_RGB_COLORS,
        )
    };
    // SAFETY: `hdc` came from `GetDC` on this thread.
    unsafe {
        let _ = ReleaseDC(None, hdc);
    }

    if lines == 0 {
        return;
    }

    for y in 0..height as usize {
        for x in 0..width as usize {
            let index = y * mask_stride + x / 8;
            let transparent = mask_bits[index] & (0x80 >> (x % 8)) != 0;
            pixels[(y * width as usize + x) * 4 + 3] = if transparent { 0 } else { 255 };
        }
    }
}

fn encode_bgra_png(width: u32, height: u32, pixels: &[u8]) -> Option<Vec<u8>> {
    // SAFETY: WIC is initialized by the caller's COM apartment. All objects
    // stay inside this function and are released by Rust ownership.
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
        let stride = width * 4;
        let bitmap = factory
            .CreateBitmapFromMemory(width, height, &GUID_WICPixelFormat32bppBGRA, stride, pixels)
            .ok()?;
        let source: IWICBitmapSource = bitmap.cast().ok()?;

        let stream = CreateStreamOnHGlobal(HGLOBAL::default(), true).ok()?;
        let encoder: IWICBitmapEncoder = factory
            .CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())
            .ok()?;
        encoder.Initialize(&stream, WICBitmapEncoderNoCache).ok()?;

        let mut frame: Option<IWICBitmapFrameEncode> = None;
        encoder
            .CreateNewFrame(&mut frame, std::ptr::null_mut())
            .ok()?;
        let frame = frame?;
        frame.Initialize(None::<&IPropertyBag2>).ok()?;
        frame.SetSize(width, height).ok()?;
        let mut format = GUID_WICPixelFormat32bppBGRA;
        frame.SetPixelFormat(&mut format).ok()?;
        frame.WriteSource(&source, std::ptr::null()).ok()?;
        frame.Commit().ok()?;
        encoder.Commit().ok()?;

        let hglobal = GetHGlobalFromStream(&stream).ok()?;
        let pointer = GlobalLock(hglobal);
        if pointer.is_null() {
            return None;
        }
        let length = GlobalSize(hglobal);
        let bytes = std::slice::from_raw_parts(pointer as *const u8, length).to_vec();
        let _ = GlobalUnlock(hglobal);

        (!bytes.is_empty()).then_some(bytes)
    }
}

/// Queries `WM_GETICON`, then the window class icon, then the system default.
///
/// `SendMessageTimeoutW` is used instead of `SendMessage` so a hung application
/// cannot block the pager forever. The timeout is short; the fallback chain
/// still produces an icon for normal windows.
fn load(hwnd: HWND) -> Option<WindowIcon> {
    let mut result = 0usize;
    unsafe {
        let _ = SendMessageTimeoutW(
            hwnd,
            WM_GETICON,
            WPARAM(ICON_SMALL2 as usize),
            LPARAM(0),
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            100,
            Some(&mut result as *mut usize),
        );
    }

    if result != 0 {
        return Some(WindowIcon(HICON(result as *mut c_void)));
    }

    let class_icon = unsafe { GetClassLongPtrW(hwnd, GCLP_HICON) };
    if class_icon != 0 {
        return Some(WindowIcon(HICON(class_icon as *mut c_void)));
    }

    unsafe { LoadIconW(None, IDI_APPLICATION) }
        .ok()
        .map(WindowIcon)
        .filter(|icon| !icon.0.0.is_null())
}
