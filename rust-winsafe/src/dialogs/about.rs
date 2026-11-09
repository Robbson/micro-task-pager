//! Native About dialog built with winsafe.
//!
//! Mirrors the reference WinForms `AboutDialog`: a fixed 600x480 client area,
//! the embedded logo, the version, author, acknowledgments, and a centered
//! Close button. The window, labels, and button are winsafe controls; only the
//! PNG logo is painted with GDI because winsafe exposes no image control.
//!
//! The dialog is modal, so the pager is disabled while it is open and Escape or
//! the Close button dismiss it. `app.rs` pauses its timers and topmost
//! reassertion for the duration of the call.

use std::io::Cursor;
use std::mem::size_of;
use std::rc::Rc;

use winsafe::{self as w, co, gui, prelude::*};

use windows::Win32::Foundation::{COLORREF, HWND as WinHWND, RECT};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, BeginPaint,
    CreateCompatibleDC, CreateDIBSection, CreateSolidBrush, DIB_RGB_COLORS, DeleteDC, DeleteObject,
    EndPaint, FillRect, HBITMAP, HGDIOBJ, PAINTSTRUCT, SelectObject, SetBkMode, SetTextColor,
    TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

use crate::logging;
use crate::render::scale;

const LOGO_PNG: &[u8] = include_bytes!("../../../csharp-winforms/Resources/logo.png");
const ICON_RESOURCE: u16 = 1;
const CLIENT_WIDTH: i32 = 600;
const CLIENT_HEIGHT: i32 = 480;
const LOGO_WIDTH: i32 = 512;
const LOGO_HEIGHT: i32 = 278;
const WM_KEY_ESCAPE: u16 = 0x1B;

/// Shows the modal About dialog owned by `parent`.
///
/// Returns after the dialog closes. The caller keeps the pager's timers and
/// topmost handling suspended while this runs.
pub fn show(parent: &gui::WindowMain, dpi: u32) -> w::AnyResult<()> {
    let width = scale(CLIENT_WIDTH, dpi);
    let height = scale(CLIENT_HEIGHT, dpi);

    let wnd = gui::WindowModal::new(gui::WindowModalOpts {
        title: "About Micro Task Pager",
        size: (width, height),
        class_icon: gui::Icon::Id(ICON_RESOURCE),
        class_bg_brush: gui::Brush::Color(co::COLOR::WINDOW),
        ..Default::default()
    });

    let version = format!("Version {}", env!("CARGO_PKG_VERSION"));
    let _version = gui::Label::new(
        &wnd,
        gui::LabelOpts {
            text: &version,
            position: (0, scale(300, dpi)),
            size: (width, scale(24, dpi)),
            control_style: co::SS::CENTER | co::SS::CENTERIMAGE,
            ..Default::default()
        },
    );
    let _author = gui::Label::new(
        &wnd,
        gui::LabelOpts {
            text: "Author: Robbson 2026",
            position: (scale(20, dpi), scale(330, dpi)),
            size: (width - scale(40, dpi), scale(24, dpi)),
            control_style: co::SS::CENTER | co::SS::CENTERIMAGE,
            ..Default::default()
        },
    );
    let _contributors = gui::Label::new(
        &wnd,
        gui::LabelOpts {
            text: "Acknowledgments:\n • DARKGuy (original project)\n • MScholtes (Virtual Desktop)",
            position: (scale(220, dpi), scale(360, dpi)),
            size: (width - scale(40, dpi), scale(60, dpi)),
            control_style: co::SS::LEFT,
            ..Default::default()
        },
    );

    let button_width = scale(100, dpi);
    let button_height = scale(30, dpi);
    let close = gui::Button::new(
        &wnd,
        gui::ButtonOpts {
            text: "Close",
            position: ((width - button_width) / 2, scale(425, dpi)),
            width: button_width,
            height: button_height,
            ..Default::default()
        },
    );

    attach_events(&wnd, &close, dpi);
    wnd.show_modal(parent)?;
    Ok(())
}

/// Wires the Close button, Escape, static-label colors, and logo painting.
fn attach_events(wnd: &gui::WindowModal, close: &gui::Button, dpi: u32) {
    let wnd_close = wnd.clone();
    close.on().bn_clicked(move || {
        wnd_close.close();
        Ok(())
    });

    let wnd_escape = wnd.clone();
    wnd.on().wm_key_down(move |p: w::msg::WmKeyDown| {
        if p.vkey_code.raw() == WM_KEY_ESCAPE {
            wnd_escape.close();
        }
        Ok(())
    });

    // Static labels inherit the window background. Force the same white used
    // by the paint handler and draw the text transparently on it.
    wnd.on().wm_ctl_color_static(|p| {
        // SAFETY: the HDC belongs to this `WM_CTLCOLORSTATIC` message.
        let hdc = windows::Win32::Graphics::Gdi::HDC(p.hdc.ptr());
        unsafe {
            let _ = SetTextColor(hdc, COLORREF(0));
            let _ = SetBkMode(hdc, TRANSPARENT);
        }
        Ok(w::HBRUSH::from_sys_color(co::COLOR::WINDOW))
    });

    let logo = Rc::new(load_logo(dpi));
    let wnd_paint = wnd.clone();
    wnd.on().wm_paint(move || {
        let logo_ref = logo.as_ref().as_ref();
        paint(win_hwnd(wnd_paint.hwnd()), logo_ref, dpi);
        Ok(())
    });
    wnd.on().wm_erase_bkgnd(|_| Ok(1));
}

fn win_hwnd(hwnd: &w::HWND) -> WinHWND {
    WinHWND(hwnd.ptr())
}

/// Fills the dialog white and alpha-blends the logo at the top center.
fn paint(hwnd: WinHWND, logo: Option<&Logo>, dpi: u32) {
    unsafe {
        let mut paint = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut paint);
        let mut rect = RECT::default();
        if GetClientRect(hwnd, &mut rect).is_ok() {
            let white = CreateSolidBrush(COLORREF(0x00FF_FFFF));
            let _ = FillRect(hdc, &rect, white);
            let _ = DeleteObject(HGDIOBJ(white.0));

            if let Some(logo) = logo {
                let width = rect.right - rect.left;
                let x = (width - logo.width) / 2;
                let y = scale(20, dpi);
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
            }
        }
        let _ = EndPaint(hwnd, &paint);
    }
}

/// A decoded logo bitmap owned by the dialog and freed on drop.
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

/// Decodes the embedded PNG, downscales it to the current DPI, and uploads it
/// as a 32-bit top-down DIB section.
fn load_logo(dpi: u32) -> Option<Logo> {
    let mut decoder = png::Decoder::new(Cursor::new(LOGO_PNG));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0u8; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buffer).ok()?;

    let source_width = info.width as usize;
    let source_height = info.height as usize;
    let bgra = to_bgra(
        &buffer[..info.buffer_size()],
        info.color_type,
        source_width * source_height,
    )?;

    // GDI's AlphaBlend downscale is nearest-neighbour and looks jagged. Scale
    // once with an area-average filter at the current DPI, then blend 1:1.
    let target_width = scale(LOGO_WIDTH, dpi).max(1) as usize;
    let target_height = scale(LOGO_HEIGHT, dpi).max(1) as usize;
    let bgra = downscale_bgra(
        &bgra,
        source_width,
        source_height,
        target_width,
        target_height,
    );
    let width = target_width as i32;
    let height = target_height as i32;

    let bitmap_info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut bits = std::ptr::null_mut();
    let bitmap =
        unsafe { CreateDIBSection(None, &bitmap_info, DIB_RGB_COLORS, &mut bits, None, 0).ok()? };
    if bits.is_null() {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
        }
        return None;
    }

    unsafe {
        std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits.cast::<u8>(), bgra.len());
    }
    Some(Logo {
        bitmap,
        width,
        height,
    })
}

/// Converts PNG samples to premultiplied BGRA, the layout AlphaBlend expects.
fn to_bgra(source: &[u8], color: png::ColorType, pixels: usize) -> Option<Vec<u8>> {
    let mut out = vec![0u8; pixels * 4];
    match color {
        png::ColorType::Rgba => {
            for index in 0..pixels {
                let src = index * 4;
                let dst = index * 4;
                let alpha = source[src + 3] as u32;
                out[dst] = ((source[src + 2] as u32 * alpha + 127) / 255) as u8;
                out[dst + 1] = ((source[src + 1] as u32 * alpha + 127) / 255) as u8;
                out[dst + 2] = ((source[src] as u32 * alpha + 127) / 255) as u8;
                out[dst + 3] = alpha as u8;
            }
        }
        png::ColorType::Rgb => {
            for index in 0..pixels {
                let src = index * 3;
                let dst = index * 4;
                out[dst] = source[src + 2];
                out[dst + 1] = source[src + 1];
                out[dst + 2] = source[src];
                out[dst + 3] = 255;
            }
        }
        other => {
            logging::error(format!("about: unsupported logo color type {other:?}"));
            return None;
        }
    }
    Some(out)
}

/// Area-average downscale for premultiplied BGRA pixels.
fn downscale_bgra(source: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return Vec::new();
    }
    if dw >= sw && dh >= sh {
        return source.to_vec();
    }

    let mut out = vec![0u8; dw * dh * 4];
    for dy in 0..dh {
        let sy0 = dy * sh / dh;
        let sy1 = ((dy + 1) * sh / dh).max(sy0 + 1).min(sh);
        for dx in 0..dw {
            let sx0 = dx * sw / dw;
            let sx1 = ((dx + 1) * sw / dw).max(sx0 + 1).min(sw);
            let mut sums = [0u32; 4];
            let mut count = 0u32;
            for sy in sy0..sy1 {
                let row = sy * sw;
                for sx in sx0..sx1 {
                    let index = (row + sx) * 4;
                    for (channel, sum) in sums.iter_mut().enumerate() {
                        *sum += source[index + channel] as u32;
                    }
                }
                count += (sx1 - sx0) as u32;
            }
            if count == 0 {
                continue;
            }
            let dst = (dy * dw + dx) * 4;
            for (channel, sum) in sums.iter().enumerate() {
                out[dst + channel] = ((*sum + count / 2) / count) as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::downscale_bgra;

    #[test]
    fn downscale_averages_the_source_area() {
        let source = [
            0u8, 0, 0, 0, //
            20, 40, 60, 80, //
            10, 30, 50, 70, //
            30, 50, 70, 90,
        ];
        assert_eq!(downscale_bgra(&source, 2, 2, 1, 1), vec![15, 30, 45, 60]);
    }

    #[test]
    fn downscale_returns_copy_when_target_is_not_smaller() {
        let source = [1u8, 2, 3, 4];
        assert_eq!(downscale_bgra(&source, 1, 1, 1, 1), source);
        assert_eq!(downscale_bgra(&source, 1, 1, 2, 2), source);
    }
}
