# Rust winsafe – Architecture

Status: stable implementation. `rust-winsafe` is a native Win32 pager built with
[`winsafe`](https://crates.io/crates/winsafe) and GDI. It has **no Windows App Runtime or reactor
dependency** and ships a single statically linked executable. It has fewer features than
`csharp-winforms`, and new features are added only after a delay. `csharp-winforms` has the latest
features; `rust-winui` is the experimental WinUI 3 implementation.

This document covers only what is specific to the winsafe front end. The shared platform contract,
application behavior, COM rules, window model, settings, and the parity checklist live in
[`../crates/micro-task-pager-core/ARCHITECTURE.md`](../crates/micro-task-pager-core/ARCHITECTURE.md).
Source baseline, verification evidence, and known gaps live in [`MIGRATION.md`](MIGRATION.md).

## 1. Scope and Shared Core

### 1.1 What the front end owns

`rust-winsafe` owns the native window host, the GDI renderer, the native dialogs, and the application
coordinator:

- `app.rs` – the single-threaded state machine and event handlers,
- `window.rs` – the winsafe `gui::WindowMain` host and message loop,
- `render/` – layout geometry, paint instructions, and the GDI backend,
- `dialogs/` – the winsafe About window, the `.rc`-backed Settings dialog, and native tooltips,
- resources, the manifest, and process bootstrap.

Everything else comes from `micro-task-pager-core`: COM bootstrap, virtual desktops, window
enumeration, settings, logging, single instance, monitor/foreground tracking, the settings draft, and
dialog placement. The core modules are re-exported from `src/lib.rs`, so internal paths such as
`crate::desktop` and `crate::settings` keep working.

### 1.2 Relationship to `rust-winui`

Both Rust front ends consume the same core crate and must preserve the same behavior contract. They
differ only in the UI toolkit:

| Area | `rust-winsafe` | `rust-winui` |
| --- | --- | --- |
| UI framework | `winsafe` controls | WinUI 3 via `windows-reactor` |
| Rendering | GDI | WinUI composition |
| Layout units | physical pixels | DIPs, snapped to device pixels |
| Dialogs | native About, `.rc` Settings | reactor secondary windows |
| Runtime | none beyond Windows | Windows App Runtime 2.x |

Do not copy core modules into this crate. Behavior changes that affect both front ends belong in
`micro-task-pager-core`.

### 1.3 Read together with

- The shared contract: `../crates/micro-task-pager-core/ARCHITECTURE.md`
- The quick module map: `../crates/micro-task-pager-core/README.md`
- Winsafe status and verification: [`MIGRATION.md`](MIGRATION.md)
- The experimental WinUI counterpart: [`../rust-winui/ARCHITECTURE.md`](../rust-winui/ARCHITECTURE.md)

## 2. Application Coordinator

`app.rs` is the single-threaded coordinator between the winsafe window, the virtual-desktop API, the
window snapshotter, the layout, and the renderer. It owns no window-procedure code; `window.rs`
forwards messages to it.

### 2.1 One Refresh

The flow on each primary timer tick is:

1. Consume any pending foreground change from the hook.
2. Capture a `WindowSnapshot` through the supervised desktop API.
3. Rebuild the `PagerLayout` for the current monitors and view modes.
4. Resize the window if the desktop count or monitor arrangement changed.
5. Invalidate the window only when the snapshot hash or layout changed.

Mouse handling uses the same layout that was painted, so hit testing cannot disagree with the
visible cells.

### 2.2 Module Organization

The inherent methods are grouped by concern, in the order a newcomer is likely to need them:

- construction and lifecycle,
- timers and desktop switching,
- snapshot and layout,
- painting and pointer input,
- drag image and pager actions,
- context menu commands,
- settings and dialogs,
- native window maintenance and positioning.

Each group starts with a `// ---- ... ----` banner. Functions may call across groups; the banners are
for reading, not layering.

### 2.3 Timers

`window.rs` installs three `WM_TIMER` timers:

- `TIMER_PRIMARY` – foreground consumption and snapshot/layout refresh (default 200 ms),
- `TIMER_SECONDARY` – monitor changes, desktop-count changes, pinning, covered-window recovery, and
  topmost reassertion (default 500 ms),
- `TIMER_RESTORE` – a one-shot 150 ms retry that restores the last active window after a desktop
  switch.

There is no permanent render loop. The window is invalidated only when something visible changed.

### 2.4 Process Identity

`App::new` sets the explicit Application User Model ID `MicroTaskPager.Rust` before the first window
exists. Pinning to all desktops works by application ID, not by window handle.

## 3. Window Host

`window.rs` replaces a hand-written `WNDCLASSW`/`CreateWindowExW`/`WndProc`/message loop with a
winsafe `gui::WindowMain`.

- winsafe creates and owns the window, its class, and the message loop.
- The pager uses `WS_POPUP | WS_VISIBLE`, `WS_EX_TOPMOST | WS_EX_LAYERED`, and adds
  `WS_EX_APPWINDOW` when `ShowAppInTaskbar` is enabled.
- winsafe stores its own pointer in `GWLP_USERDATA`, so the `App` pointer is kept in a thread-local
  slot instead of the window user data.
- `run` boxes the `App`, installs its raw pointer in `wm_create`, and unboxes it after `run_main`
  returns. All pointer access is on the owning UI thread.
- Every winsafe event handler is a thin adapter: it decodes the message into plain arguments and
  calls one `App` method. State changes, COM calls, and drawing happen in `app.rs`, never in an event
  handler.
- The only exception is the native context menu, which runs its own modal `TrackPopupMenu` loop in a
  private helper.

The context menu uses the C# glyph labels (`↔`, `⌖`, `⚙`, `ℹ`, `✕`) as leading text and
`MNS_NOCHECK` to remove the empty check column. While the menu is open, topmost is temporarily
dropped and the timer-driven reassertion is suppressed.

## 4. Rendering

### 4.1 GDI

The output is a few dozen filled rectangles, application icons, one-pixel borders, and one text
string per overflow tile. The renderer draws into an off-screen memory DC
(`CreateCompatibleDC` + `CreateCompatibleBitmap`) and blits once per `WM_PAINT` with `BitBlt`. This
is the same double-buffering the C# application gets from
`ControlStyles.OptimizedDoubleBuffer` and avoids flicker.

The renderer receives an immutable `PagerLayout` and produces drawing calls. It does not call COM,
enumerate windows, or read settings. No COM calls and no window enumeration happen inside
`WM_PAINT`.

### 4.2 Layout, Paint Instructions, and Backend

`render/` is split the same way as the WinUI pager, even though the output is GDI:

- `layout.rs` turns a `WindowSnapshot` plus monitor geometry into a `PagerLayout`. The same layout
  drives painting and hit testing, so a click can never land somewhere different from what was
  painted. Layout math is unit-tested without a window.
- `paint.rs` walks a finished `PagerLayout` and emits simple drawing calls through the `Painter`
  trait. It contains no platform code.
- `gdi.rs` implements `Painter` with the double-buffered GDI memory DC.

`Painter` is a small trait:

```rust
pub trait Painter {
    fn fill_rect(&mut self, rect: RECT, color: u32);
    fn stroke_rect(&mut self, rect: RECT, color: u32, width: i32);
    fn draw_icon(&mut self, icon: WindowIcon, center_x: i32, center_y: i32, size: i32);
    fn draw_text_centered(&mut self, text: &str, rect: RECT, color: u32, size: i32);
    fn push_clip(&mut self, rect: RECT);
    fn pop_clip(&mut self);
}
```

Keeping layout, drawing instructions, and the GDI backend separate means geometry and paint order
can be read and tested without a window, and the backend can be replaced without touching either.

### 4.3 Coordinates

All rectangles are client coordinates of the pager window in physical pixels. Screen and cell
coordinates are absolute, not relative to their desktop column; a cell on desktop 1 therefore has an
x coordinate past the first desktop's width.

Each monitor's pager width is its aspect-ratio width at the configured pager height, so the pager
keeps the monitor's shape instead of using the raw pixel width. Desktops are columns; within a
desktop, one renderer per physical monitor is ordered left-to-right and then top-to-bottom.

### 4.4 View Modes

- `Preview` places windows at their scaled screen position, so cells may overlap and are stored
  back-to-front.
- `Micro Taskbar` packs icons into a fixed grid and reports an overflow tile when not every window
  fits.

The paint order is fixed: background first, then each screen clips and paints its cells, then
separators and desktop borders. Each screen pushes a clip rectangle, so a cell cannot paint into the
neighboring screen.

### 4.5 GDI Details

- Pens, brushes, and the font are cached for the duration of one paint and deleted in `Drop`.
- `GdiPainter::stroke_rect` adds one pixel to the right and bottom before calling GDI `Rectangle`,
  because GDI treats the far edge as exclusive while the C# GDI+ reference includes it.
- Separators pre-blend the C# translucent color because `FillRect` has no alpha channel.
- Icons are drawn with `DrawIconEx` at the target size; no per-icon bitmap cache is required.

## 5. Dialogs

Both dialogs are modal Win32 dialogs owned by the pager window, not separate top-level
components. Placement uses the shared `find_placement` algorithm.

### 5.1 About

`dialogs/about.rs` is a winsafe `gui::WindowModal`:

- fixed 600 x 480 client area,
- the embedded `logo.png`,
- version, author, acknowledgments, and a centered Close button,
- light theme with a white background.

Only the PNG logo is painted with GDI because winsafe exposes no image control. The PNG is decoded
with the pure-Rust `png` crate, converted to premultiplied BGRA, and downscaled once with an
area-average filter to the current DPI size. Escape or the Close button dismisses the dialog. The
pager pauses its timers and topmost reassertion while the dialog is open.

### 5.2 Settings

`dialogs/settings/` is a fixed `.rc` dialog template loaded with winsafe `new_dlg`:

- `settings.rc` defines the main dialog and three page dialogs (Layout, Theme, Misc);
  `settings_ids.h` holds the shared control IDs and must stay in sync with `settings/ids.rs`.
- The pages are winsafe `TabPage` dialogs; winsafe handles tab selection and resizes the selected
  page.
- Controls are standard winsafe widgets: `ComboBox`, `Edit` + `UpDown`, `Trackbar`, `CheckBox`,
  `Button`, and `Label`.
- Colors use owner-drawn buttons and `ChooseColorW`.
- Tooltips use a raw `tooltips_class32` helper because winsafe 0.0.29 has no tooltip wrapper.
- All edits target the shared `SettingsDraft`; the caller applies the result only when the user
  presses Save.
- `Save` persists and closes; `Cancel` discards the draft. Per-option revert targets the values the
  dialog opened with; each section has its own Defaults button.
- Safe changes (opacity, colors, height, view mode) preview live through a callback, but
  `ShowAppInTaskbar` takes effect only on Save.
- `show` blocks in the modal loop and returns the accepted settings.

The draft, its limits, and the placement algorithm come from `micro-task-pager-core`; keep the
behavior compatible with `rust-winui`.

## 6. Technology Stack

| Area | Technology | Status |
| --- | --- | --- |
| Programming language | Rust (stable, MSVC toolchain) | decided |
| Window host | `winsafe` 0.0.29 `gui::WindowMain` / `WindowModal` | decided |
| Native Windows bindings | `windows` 0.62 | decided |
| Shared platform | `micro-task-pager-core` (path dependency) | decided |
| Rendering | GDI into a double-buffered memory DC behind `Painter` | decided |
| Direct2D | not used; deferred and not required | deferred |
| Window previews | Scaled window rectangles + shell icons (no DWM) | decided |
| Application icons | Shared `HICON` lookup, `DrawIconEx` | decided |
| Context menu | Raw `TrackPopupMenu` shim | decided |
| Settings UI | Fixed `.rc` dialog with winsafe controls | decided |
| About | winsafe modal with a PNG logo painted with GDI | decided |
| Settings storage | Windows registry through `micro-task-pager-core` | decided |
| Logging | Core crate: daily file, 14-day retention | decided |
| Build system | Cargo workspace | decided |
| Testing | `cargo test`, shared `probe` example | decided |

`windows-reactor` is not a dependency and must not be added. This front end is deliberately
reactor-free and requires no Windows App Runtime.

## 7. Repository Structure

The stable front end lives in `rust-winsafe/`, next to the experimental `rust-winui/` and the shared
`crates/micro-task-pager-core/` platform crate. They form one Cargo workspace rooted at the
repository `Cargo.toml`.

```
rust-winsafe/
  Cargo.toml                  # package micro-task-pager-winsafe
  app.manifest                # asInvoker, system DPI awareness
  app.rc                      # icon/version; includes settings.rc
  settings.rc                 # fixed Settings dialog templates
  settings_ids.h              # control IDs, mirrored in settings/ids.rs
  build.rs                    # embeds resources and the manifest
  ARCHITECTURE.md
  MIGRATION.md
  src/
    main.rs                   # logging, COM apartment, single instance, window loop
    lib.rs                    # re-exports the core modules
    app.rs                    # application state, timers, input, menu, settings
    window.rs                 # winsafe window host and message loop
    render/
      mod.rs                  # scale helper and Painter trait
      layout.rs               # geometry and hit testing
      paint.rs                # drawing instructions
      gdi.rs                  # GDI backend
    dialogs/
      mod.rs
      about.rs                # winsafe About window
      settings/
        mod.rs                # `.rc`-backed dialog and event wiring
        ids.rs                # control IDs
        tooltips.rs           # raw tooltip helper
```

## 8. DPI, Positioning, and Always-On Details

### 8.1 DPI

The process is **system DPI aware** (`app.manifest`), and all layout is computed in physical pixels
from a single scale factor (`GetDpiForSystem` / 96). This matches the C# application:

- The system DPI is read once at startup.
- `WM_DPICHANGED` is not used to rescale the layout.
- Pager height, paddings, borders, icon size, and font size are logical values multiplied by the
  scale factor.
- Per-monitor V2 support is out of scope for this front end.

This is a deliberate difference from `rust-winui`, which is per-monitor DPI aware through WinUI.

### 8.2 Positioning

Initial position is the saved `PagerLocationX`/`PagerLocationY`, or bottom-center of the primary
monitor's work area. Dragging the pager uses the middle mouse button or Shift + left button. The
context menu can restore, save, or reset the position.

### 8.3 Opacity

`BackgroundOpacity` is applied to the whole window with `WS_EX_LAYERED` and
`SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA)`. It is never simulated by pre-multiplying
colors.

### 8.4 Always-On Behavior

- `try_pin` pins the application to all desktops when `ShowAppInTaskbar` is enabled; failures are
  logged and retried on the secondary tick.
- `reassert_topmost` raises the window unless a menu or dialog is open.
- `restore_window` re-shows the pager with `SW_SHOWNOACTIVATE` and re-applies topmost when another
  window covers its center.
- `ShowAppInTaskbar` also adds or removes `WS_EX_APPWINDOW`.

### 8.5 Drag Image

Window drags use the common-controls image-list drag API (`ImageList_BeginDrag`,
`ImageList_DragEnter`), which is the same mechanism used by common list controls. The image list
owns a copy of the icon and stays alive until `ImageList_EndDrag`. This replaces OLE drag-and-drop:
the payload is only a window handle and a source desktop, so `IDropSource`/`IDropTarget` registration
is unnecessary. The normal arrow cursor stays; the four-way move cursor is reserved for dragging the
pager itself.

## 9. Known Deviations from C#

These deviations are specific to the winsafe front end. The shared deviations common to both Rust
front ends are listed in `../crates/micro-task-pager-core/ARCHITECTURE.md` §11.1.

- Use `winsafe` controls for the main window, About, and Settings instead of hand-written
  `WNDCLASSW`/`WndProc`/`DLGTEMPLATE` code. Behavior is intended to match.
- Keep the C# system-DPI behavior; per-monitor DPI belongs to `rust-winui`.
- Pre-blend the desktop separator color because GDI `FillRect` has no alpha channel.
- Add one pixel to the far edge in `stroke_rect` because GDI `Rectangle` excludes the right/bottom
  edge while the C# GDI+ reference includes it.
- Draw the About logo from the original PNG with the `png` crate and an area-average downscale
  instead of WIC/`AlphaBlend`; the result is sharper at scaled DPIs.
- Use `ImageList_BeginDrag` for the drag ghost and keep the normal arrow cursor; the four-way cursor
  is reserved for pager dragging.
- Use the same leading menu glyphs as the C# and WinUI menus (`↔`, `⌖`, `⚙`, `ℹ`, `✕`) with
  `MNS_NOCHECK`.
- The Settings dialog uses Save/Cancel and per-section Defaults, matching the C# dialog. Safe edits
  preview live; `ShowAppInTaskbar` applies only on Save.

## 10. Testing and Verification

- Unit-test pure logic: aspect-ratio scaling, Micro Taskbar grid and overflow computation,
  change-detection hash (shared core), registry parsing (shared core), settings defaults, hit testing,
  and the settings draft (shared core).
- Keep the shared `probe` example for COM and version-matrix verification.
- Add `#[ignore]`d integration tests for mutation paths, gated by an environment variable, so they
  never move real windows during a normal `cargo test`.
- Add screenshot comparison or at least a deterministic layout test for both view modes using a fake
  `Painter`.
- Run `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and
  `cargo test --workspace` before declaring work complete.

## 11. Deployment and Packaging

- x64 MSVC build; no .NET runtime and no installer.
- `#![windows_subsystem = "windows"]` so no console window appears.
- Statically link the MSVC runtime (`.cargo/config.toml` at the workspace root) so the executable
  does not require the Visual C++ redistributable.
- Embed the icon and a version resource.
- Keep the application manifest `asInvoker` with system DPI awareness.
- Keep the C# mutex name (`micro-task-pager-win-app`) for release builds so the implementations
  cannot run simultaneously and fight over topmost/pinning. Use a different name for development
  builds if side-by-side comparison is needed.
- Log to `%LOCALAPPDATA%\MicroTaskPager\logs` through the core crate.
- The release artifact is `target/release/micro-task-pager-winsafe.exe`. A distribution folder may
  rename it to `MicroTaskPager.exe` as a drop-in replacement. The C# implementation stays in the
  repository until older-build and multi-monitor parity are verified.

## 12. Rust Development Guidelines

1. Prefer safe Rust APIs over direct FFI.
2. Keep `unsafe` blocks small and document their safety requirements.
3. Encapsulate all undocumented COM interfaces behind the core `VirtualDesktopApi` trait.
4. Avoid leaking raw Windows handles across application layers.
5. Use RAII wrappers for GDI, COM, and other Windows resources.
6. Handle `HRESULT` errors explicitly; no silent fallbacks.
7. Respect COM apartment and thread-affinity requirements; use one STA thread.
8. Avoid unnecessary dependencies.
9. Keep application logic independent of the window and the renderer.
10. Write compatibility tests for undocumented Windows APIs.
11. Do not assume Windows implementation details remain stable across updates.
12. Prefer native Windows functionality over unnecessary abstractions.
13. No panics across FFI boundaries.
14. Keep the settings model and the rendering model free of Win32 types where practical.
15. Put shared platform behavior in `micro-task-pager-core`.

`winsafe` is a wrapper for ordinary Win32 UI work; it does not remove the `unsafe` needed for the
virtual-desktop COM layer or GDI. Keep `unsafe` confined to the platform modules in the core crate,
`render/gdi.rs`, `window.rs`, the dialogs, and the small Win32 calls in `app.rs`, and give every
block a safety comment. Layout, paint instructions, snapshot models, settings, and the supervisor
stay safe Rust.

## 13. Important Architectural Decisions

| Decision | Direction |
| --- | --- |
| Repository layout | Separate `rust-winsafe/` directory in a Cargo workspace |
| Shared platform | `micro-task-pager-core` |
| Main window | winsafe `gui::WindowMain` |
| Rendering | GDI behind a `Painter` trait |
| Rendering later | Direct2D via `windows-canvas` only after a spike; not planned |
| Window previews | Scaled geometries + shell icons |
| Live thumbnails | Not in v1 |
| Settings UI | Fixed `.rc` dialog with winsafe controls |
| About | winsafe modal with a PNG logo |
| Settings storage | C#-compatible registry values through the core crate |
| Virtual desktops | Core: one safe API + Win10/Win11 layout modules |
| Threading | Single STA UI thread, `WM_TIMER` driven |
| DPI | System DPI aware, physical-pixel layout |
| Application icons | Shell `HICON` handles drawn with `DrawIconEx` |
| Application state | Pure Rust coordinator |
| .NET dependency | Removed |
| Reactor / Windows App Runtime | Not used |

## 14. Instructions for AI Coding Assistants

When assisting with this project:

- Read this document, the shared
  [`core architecture`](../crates/micro-task-pager-core/ARCHITECTURE.md), `AGENTS.md`, and
  `MIGRATION.md` before proposing structural changes.
- Treat `winsafe` + GDI as this front end's UI architecture. Do not propose WinUI 3 or reactor here,
  and do not introduce `egui` or another GUI toolkit.
- Keep winsafe controls, the GDI renderer, and `.rc` dialogs inside this crate.
- Prefer Microsoft's official Rust crates.
- Verify current crate APIs against their actual documentation and source code; pin versions.
- Do not invent APIs, methods, COM interfaces, or vtable layouts.
- Pay particular attention to undocumented Windows interfaces; never assume a COM interface
  definition is identical across Windows builds.
- Explain all required `unsafe` operations and their safety assumptions.
- Keep version-specific Windows compatibility code isolated in the core crate.
- Do not add DWM thumbnails, Direct2D, per-monitor DPI, or ARM64 unless explicitly asked.
- Put shared platform behavior in `micro-task-pager-core`, not in this front end.
- Avoid unnecessary architectural complexity.
- Prefer small, independently testable implementation steps.
- Preserve existing application behavior unless a deviation is listed in section 9 or explicitly
  requested.
- When porting existing C# functionality, use the existing implementation as the behavioral
  reference.

The primary objective is a reliable, maintainable, fully native Windows application implemented in
Rust, not merely a line-by-line translation of the original C# code.
