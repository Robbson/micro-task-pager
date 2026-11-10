//! Converts the snapshot's borrowed `HICON` handles to reactor image sources.
//!
//! WinUI cannot draw an `HICON`, so each icon is encoded once into a PNG of the
//! exact display size:
//!
//! ```text
//! WindowSnapshot HICON -> PNG bytes at size_px -> reactor EncodedImage -> Image
//! ```
//!
//! Encoding at the display size (instead of scaling a bitmap in XAML) is what
//! keeps the icons crisp at 125%, 150%, and other scale factors.
//!
//! The shared snapshotter already caches the shell icon handle per window. This
//! cache adds the encoded PNG keyed by window and icon handle so a 200 ms
//! refresh does not re-encode every icon. Entries are dropped when a window
//! disappears or changes its icon, and the whole cache is cleared when the
//! target pixel size changes.
//!
//! The handle itself, and the rule that it must not be destroyed, come from
//! `micro-task-pager-core`. Everything below is WinUI-specific because WIC and
//! `windows-reactor` are not shared with the winsafe front end.

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits,
    GetObjectW, HBITMAP, HGDIOBJ, ReleaseDC,
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
    CopyImage, DestroyIcon, GetIconInfo, HICON, ICONINFO, IMAGE_ICON, LR_COPYFROMRESOURCE,
};
use windows::core::Interface;
use windows_reactor::EncodedImage;

use crate::logging;
use crate::windows::{WindowIcon, WindowSnapshot};

#[derive(Default)]
pub(super) struct IconSources {
    entries: HashMap<usize, (WindowIcon, EncodedImage)>,
    size_px: i32,
}

impl IconSources {
    /// Updates the cache for a snapshot at the target pixel size.
    ///
    /// A DPI change clears the cache because the encoded bitmaps are rendered
    /// at the exact display size to stay crisp.
    pub fn refresh(&mut self, snapshot: &WindowSnapshot, size_px: i32) {
        let size_px = size_px.max(1);
        if self.size_px != size_px {
            self.entries.clear();
            self.size_px = size_px;
        }

        let mut active = HashSet::with_capacity(snapshot.windows.len());

        for window in &snapshot.windows {
            let Some(icon) = window.icon else {
                continue;
            };
            let key = window.hwnd.0 as usize;
            active.insert(key);

            let unchanged = self
                .entries
                .get(&key)
                .is_some_and(|(cached, _)| *cached == icon);
            if unchanged {
                continue;
            }

            match png_bytes(icon, size_px) {
                Some(bytes) => {
                    self.entries.insert(key, (icon, EncodedImage::new(bytes)));
                }
                None => {
                    self.entries.remove(&key);
                    logging::warn(format!(
                        "icon conversion failed for window {:?}",
                        window.hwnd
                    ));
                }
            }
        }

        self.entries.retain(|key, _| active.contains(key));
    }

    /// Returns the encoded icon for a window, if conversion succeeded.
    pub fn get(&self, hwnd: usize) -> Option<&EncodedImage> {
        self.entries.get(&hwnd).map(|(_, image)| image)
    }

    /// Current encoded pixel size.
    pub fn size_px(&self) -> i32 {
        self.size_px
    }
}

/// Encodes a shared shell icon as a PNG for a reactor image source.
///
/// The shell icon is a shared handle, so this only reads it. A scaled copy is
/// created at the target pixel size so WinUI does not resample a larger icon
/// frame down to 16 DIP. The returned bytes own no native resources and can be
/// cached across snapshots.
fn png_bytes(icon: WindowIcon, size_px: i32) -> Option<Vec<u8>> {
    let scaled = if size_px > 0 {
        // SAFETY: `CopyImage` reads the shared icon and returns a new icon
        // handle that is destroyed below.
        unsafe {
            CopyImage(
                HANDLE(icon.handle().0),
                IMAGE_ICON,
                size_px,
                size_px,
                LR_COPYFROMRESOURCE,
            )
            .ok()
        }
    } else {
        None
    };

    match scaled {
        Some(handle) => {
            let icon = HICON(handle.0);
            let result = encode_png(icon);
            // SAFETY: `CopyImage` returned an owned icon handle.
            unsafe {
                let _ = DestroyIcon(icon);
            }
            result
        }
        // Some icons are not backed by a resource and cannot be copied.
        // Fall back to the original frame; it is better than no icon.
        None => encode_png(icon.handle()),
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

fn icon_pixels(hbitmap: HBITMAP, width: i32, height: i32) -> Option<Vec<u8>> {
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

fn apply_mask(pixels: &mut [u8], mask: HBITMAP, width: i32, height: i32) {
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
