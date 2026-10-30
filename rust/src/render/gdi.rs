//! GDI implementation of the layout's [`Painter`] trait.
//!
//! All drawing goes into an off-screen memory DC and is blitted to the window
//! once per paint. That is the same double-buffering the C# application gets
//! from `ControlStyles.OptimizedDoubleBuffer` and avoids flicker.
//!
//! Brushes, pens, and the font are cached for the duration of one paint and
//! deleted in `Drop`. The pager repaints on state changes, so allocating a few
//! GDI objects per frame is cheap; the cache mainly keeps the code readable.

use std::collections::HashMap;

use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleBitmap,
    CreateCompatibleDC, CreateFontW, CreatePen, CreateSolidBrush, DEFAULT_CHARSET, DEFAULT_PITCH,
    DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, EndPaint,
    FF_DONTCARE, FW_NORMAL, FillRect, GetStockObject, HBRUSH, HDC, HFONT, HGDIOBJ, HPEN,
    IntersectClipRect, NULL_BRUSH, OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_SOLID, Rectangle, RestoreDC,
    SRCCOPY, SaveDC, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{DI_NORMAL, DrawIconEx, GetClientRect};
use windows::core::w;

use crate::render::{PagerLayout, Painter, paint_layout, scale};
use crate::settings::Settings;
use crate::windows::{WindowIcon, WindowSnapshot};

/// Paints the pager into `hwnd` using the current layout.
pub fn paint_window(
    hwnd: HWND,
    layout: &PagerLayout,
    snapshot: &WindowSnapshot,
    settings: &Settings,
    dpi: u32,
) {
    unsafe {
        let mut paint = PAINTSTRUCT::default();
        let window_dc = BeginPaint(hwnd, &mut paint);
        if window_dc.0.is_null() {
            return;
        }

        let mut client = RECT::default();
        let _ = GetClientRect(hwnd, &mut client);
        let width = (client.right - client.left).max(1);
        let height = (client.bottom - client.top).max(1);

        let memory_dc = CreateCompatibleDC(Some(window_dc));
        let bitmap = CreateCompatibleBitmap(window_dc, width, height);
        let old_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));

        {
            let mut painter = GdiPainter::new(memory_dc, dpi);
            paint_layout(&mut painter, layout, snapshot, settings, dpi);
        }

        let _ = BitBlt(
            window_dc,
            0,
            0,
            width,
            height,
            Some(memory_dc),
            0,
            0,
            SRCCOPY,
        );

        SelectObject(memory_dc, old_bitmap);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory_dc);
        let _ = EndPaint(hwnd, &paint);
    }
}

struct GdiPainter {
    dc: HDC,
    brushes: HashMap<u32, HBRUSH>,
    pens: HashMap<(u32, i32), HPEN>,
    font: HFONT,
    saved: Vec<i32>,
}

impl GdiPainter {
    fn new(dc: HDC, dpi: u32) -> Self {
        let font_height = -scale(12, dpi).max(1);
        let font = unsafe {
            CreateFontW(
                font_height,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                DEFAULT_PITCH.0 as u32 | FF_DONTCARE.0 as u32,
                w!("Segoe UI Symbol"),
            )
        };

        Self {
            dc,
            brushes: HashMap::new(),
            pens: HashMap::new(),
            font,
            saved: Vec::new(),
        }
    }

    fn brush(&mut self, color: u32) -> HBRUSH {
        if let Some(brush) = self.brushes.get(&color) {
            return *brush;
        }

        let brush = unsafe { CreateSolidBrush(colorref(color)) };
        self.brushes.insert(color, brush);
        brush
    }

    fn pen(&mut self, color: u32, width: i32) -> HPEN {
        let key = (color, width);
        if let Some(pen) = self.pens.get(&key) {
            return *pen;
        }

        let pen = unsafe { CreatePen(PS_SOLID, width.max(1), colorref(color)) };
        self.pens.insert(key, pen);
        pen
    }
}

impl Painter for GdiPainter {
    fn fill_rect(&mut self, rect: RECT, color: u32) {
        let brush = self.brush(color);
        unsafe {
            FillRect(self.dc, &rect, brush);
        }
    }

    fn stroke_rect(&mut self, rect: RECT, color: u32, width: i32) {
        let pen = self.pen(color, width);
        unsafe {
            let old_pen = SelectObject(self.dc, HGDIOBJ(pen.0));
            let old_brush = SelectObject(self.dc, GetStockObject(NULL_BRUSH));
            let _ = Rectangle(self.dc, rect.left, rect.top, rect.right, rect.bottom);
            SelectObject(self.dc, old_brush);
            SelectObject(self.dc, old_pen);
        }
    }

    fn draw_icon(&mut self, icon: WindowIcon, center_x: i32, center_y: i32, size: i32) {
        let size = size.max(1);
        unsafe {
            let _ = DrawIconEx(
                self.dc,
                center_x - size / 2,
                center_y - size / 2,
                icon.handle(),
                size,
                size,
                0,
                None,
                DI_NORMAL,
            );
        }
    }

    fn draw_text_centered(&mut self, text: &str, rect: RECT, color: u32, size: i32) {
        if self.font.0.is_null() {
            return;
        }

        let mut wide: Vec<u16> = text.encode_utf16().collect();
        if wide.is_empty() {
            return;
        }

        let mut area = rect;
        unsafe {
            let old_font = SelectObject(self.dc, HGDIOBJ(self.font.0));
            SetTextColor(self.dc, colorref(color));
            SetBkMode(self.dc, TRANSPARENT);
            let _ = DrawTextW(
                self.dc,
                &mut wide,
                &mut area,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            );
            SelectObject(self.dc, old_font);
        }

        let _ = size;
    }

    fn push_clip(&mut self, rect: RECT) {
        unsafe {
            let saved = SaveDC(self.dc);
            if saved != 0 {
                IntersectClipRect(self.dc, rect.left, rect.top, rect.right, rect.bottom);
                self.saved.push(saved);
            }
        }
    }

    fn pop_clip(&mut self) {
        if let Some(saved) = self.saved.pop() {
            unsafe {
                let _ = RestoreDC(self.dc, saved);
            }
        }
    }
}

impl Drop for GdiPainter {
    fn drop(&mut self) {
        unsafe {
            for (_, brush) in self.brushes.drain() {
                let _ = DeleteObject(HGDIOBJ(brush.0));
            }
            for (_, pen) in self.pens.drain() {
                let _ = DeleteObject(HGDIOBJ(pen.0));
            }
            if !self.font.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(self.font.0));
            }
        }
    }
}

/// Converts a 32-bit ARGB value to a GDI `COLORREF` (`0x00BBGGRR`).
fn colorref(argb: u32) -> COLORREF {
    let red = (argb >> 16) & 0xFF;
    let green = (argb >> 8) & 0xFF;
    let blue = argb & 0xFF;
    COLORREF(red | (green << 8) | (blue << 16))
}
