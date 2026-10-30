# Rust Migration – Architecture and Development Guidelines

Status: working draft. The status column in section 3 and the decisions in section 15 define the
current direction; everything else is a candidate or deferred.

## 1. Project Overview

This project is a migration of an existing C#/.NET Windows Forms application to native Rust.

The application is a **Virtual Desktop Pager for Windows**. Its primary purpose is managing Windows
virtual desktops and providing a persistent visual overview of them, including window layout and
application icons.

The application is heavily Windows-specific and does not need to be cross-platform.

The main goals of the migration are:

- Eliminate the .NET runtime dependency.
- Use Rust as the primary implementation language.
- Improve memory safety and reliability.
- Retain access to native Windows functionality.
- Preserve existing application behavior and functionality.
- Modernize selected UI components where it makes sense.
- Keep the architecture modular and maintainable.

Microsoft's official `windows-rs` ecosystem should be preferred wherever practical.

The existing C# implementation is the behavioral reference. When in doubt, read the C# code rather
than guessing what the application does.

### 1.1 Non-Goals for v1

- A settings window or any settings UI framework.
- DWM live thumbnails or screen capture.
- Direct2D/GPU rendering, unless a dedicated spike shows it is needed.
- Per-monitor DPI V2.
- ARM64.
- Desktop creation, removal, or renaming.
- Installer, auto-update, and code signing.

## 2. What the Existing Application Actually Does

This section describes the behavior that has to survive the migration. It is the most important
part of this document.

### 2.1 Main Window

`MainForm.cs` is a single borderless, always-on-top window with no control box:

- The pager is one window; virtual desktops are drawn inside it, not as child windows.
- Its width is the sum of all desktop widths; its height is the configured pager height scaled by DPI.
- Default position is bottom-center of the primary screen's working area. The user can move it with
  the middle mouse button or Shift + left mouse button, and can save, restore, or reset the position
  from the context menu (stored in the registry).
- Right-click opens a context menu: Toggle View Mode, Position (Restore/Save/Default), Settings,
  About, Exit. The Settings entry is dropped in v1 (see section 8); Exit must remain, because the
  window has no system menu.
- Background opacity is the window's alpha (`Form.Opacity`), i.e. a layered window.
- The process is **system DPI aware**, not per-monitor aware (see section 7.6).

### 2.2 View Modes

Each desktop is drawn as a row of one renderer per physical monitor. Each renderer independently uses
one of two modes, remembered per desktop/screen:

**Preview** is a *scaled geometry map*, not a live screen image. For every visible window on the
desktop and monitor it:

1. Takes the window rectangle in screen coordinates and subtracts the monitor origin.
2. Scales the rectangle into pager space.
3. Fills it with a solid color (active/inactive) and draws the application icon centered in it.
4. Draws a border.

Windows are drawn in z-order (back to front). Clicking a miniature selects that window. There is **no
DWM thumbnail and no screen capture** anywhere in the current implementation, and the migrated v1 must
not introduce one (see section 3).

**Micro Taskbar** draws a centered grid of application icons for the desktop/monitor. It computes
columns and rows from the available size and icon padding, and reserves the last cell for a `+N`
overflow label when not all windows fit. Active windows get a highlighted tile background.

### 2.3 Settings and Persistence

Settings are not a separate model today: `AppSettings` is loaded from and saved to
`HKEY_CURRENT_USER\SOFTWARE\MicroTaskPager` (`AppSettingsStore.cs`). The pager position is stored under
the same key (`RegistryAccess.cs`). The settings dialog is built programmatically with WinForms; the
project enables `UseWPF` and a few files carry unused `System.Windows.*` imports, but no WPF UI exists.

### 2.4 Virtual Desktop Management

The application uses undocumented shell COM interfaces. The public `IVirtualDesktopManager` is not
sufficient. The required capabilities are:

- Count desktops, get the current desktop, get a desktop by index.
- Switch to a desktop.
- Move a window (more precisely, its `IApplicationView`) to a desktop.
- Map a desktop object to its index and a window handle to its desktop.
- Pin the pager process/application to all desktops via `IVirtualDesktopPinnedApps`.

The C# implementation contains four version-specific variants under `src/Core/API/` (`Win10`,
`Win10LTSC`, `Win11`, `Win11_`). These variants differ in vtable layout, not in application logic.

### 2.5 Always-On Behavior

The pager is only useful if it is always present. The current app implements this with:

- Pinning itself to all virtual desktops (only possible when the app appears in the taskbar; a failed
  auto-pin at startup is expected and tolerated).
- `HWND_TOPMOST` plus a 500 ms watchdog: if `WindowFromPoint` at the pager's center returns another
  window, the pager re-shows itself with `ShowWindowAsync(SW_SHOWNOACTIVATE)` and re-applies topmost.
- A single-instance mutex so two pagers cannot fight over topmost and pinning.

### 2.6 Window Model

`WindowManager.GetOpenWindows()` enumerates top-level windows and filters them:

- Skip the shell window, invisible windows, windows without a title, and the
  `Windows.UI.Core.CoreWindow` class (Start menu).
- Skip windows owned by the pager process.
- Skip windows whose virtual desktop cannot be determined. Failures are counted per handle and the
  handle is blacklisted after 10 failures (snapped-window helper windows trigger this).
- Read title, class, process ID, rectangle, foreground state, and z-order. The C# code computes
  z-order by following `GetWindow(GW_HWNDNEXT)`; the Rust port derives the same relative order from
  `EnumWindows` and normalizes it so larger values are closer to the top.
- Read the icon via `WM_GETICON` (`ICON_SMALL2`), then `GCL_HICON`, then `IDI_APPLICATION`. Icons are
  cached per `(hwnd, hIcon)` and pruned when the window disappears.

### 2.7 Last Active Window per Desktop

`ActiveWindowTracker` installs a `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` hook and remembers the last
real foreground window for each desktop. When the user switches back to a desktop, the pager restores
that window, retrying once after 150 ms because the desktop switch animation can still be in progress.

### 2.8 Refresh Model

There is no render loop. Two polling loops (200 ms primary, 500 ms secondary) build a window snapshot,
compute a change-detection hash, and invalidate the window only when the hash changes. Desktop count
and monitor configuration are checked on the slower loop and cause a full rebuild of all desktop and
COM objects.

## 3. Technology Stack

| Area | Technology | Status |
|---|---|---|
| Programming language | Rust (stable, MSVC toolchain) | decided |
| Windows API bindings | `windows` / `windows-rs` | decided |
| Low-level Windows bindings | `windows-sys` only where necessary | decided |
| COM interoperability | `windows-core` | decided |
| Main window | Native Win32 (`RegisterClassExW`, `CreateWindowExW`, message loop) | decided |
| Main rendering (v1) | GDI into a double-buffered memory DC behind a `Painter` trait | decided |
| Main rendering (later) | Direct2D via `windows-canvas` | deferred, spike required |
| Window previews | Scaled window rectangles + shell icons (no DWM) | decided |
| Live thumbnails | DWM Thumbnail API | deferred, not planned for v1 |
| Application icons | `WM_GETICON` / `GCL_HICON` / `LoadIcon` | decided |
| Settings UI | none in v1; `winsafe` / `windows-reactor` are not used | not planned |
| Settings storage | Windows registry, C#-compatible | decided |
| Windows version detection | `windows-version` crate or `RtlGetVersion` | candidate |
| Logging | In-house file logger, daily file, 14-day retention | decided |
| Build system | Cargo | decided |
| Testing | `cargo test` for pure logic, `examples/probe` for COM | decided |

Do not add a third-party GUI framework. In v1 the application needs exactly one window, one context
menu, and GDI drawing; `winsafe`, `windows-reactor`, `egui`, and similar crates are unnecessary.

## 4. Repository Structure

The Rust implementation lives in `rust/` inside the existing Git repository. The C# implementation
remains available during migration and is the reference for behavior.

Start with a single Cargo crate. Split it into multiple crates only when there is a concrete
architectural benefit (for example, if the COM probe becomes reusable). Suggested module layout:

```
rust/
  Cargo.toml
  Cargo.lock                    # committed: this is a binary crate
  rust-toolchain.toml           # toolchain channel + rustfmt/clippy
  MIGRATION.md                  # phase status, verification log, open questions
  src/
    main.rs                     # process setup, single instance, message loop
    app.rs                      # application state and state transitions
    com.rs                      # COM apartment guard
    single_instance.rs          # mutex acquisition and second-instance handling
    foreground.rs               # foreground hook and last-active-window tracking
    window.rs                   # Win32 window, wndproc, context menu
    input.rs                    # hit testing, drag handling
    settings.rs                 # model, defaults, registry import/export
    logging.rs
    desktop/
      mod.rs                    # safe virtual-desktop API and restart supervisor
      com/mod.rs                # layout selection by build number
      com/core.rs               # shared interfaces and generic backend
      com/win10.rs
      com/win11.rs
    windows/
      mod.rs                    # window enumeration and snapshot
      icons.rs
    render/
      mod.rs                    # Painter trait and layout model
      gdi.rs
  examples/
    probe/                      # standalone COM verification binary
```

## 5. Windows API Integration

### 5.1 windows-rs

Use Microsoft's official `windows` crate as the primary binding. Enable only the features that are
used, for example:

- `Win32_Foundation`
- `Win32_Security` (required by `CreateMutexW`)
- `Win32_System_Com`, `Win32_System_LibraryLoader`, `Win32_System_Threading`
- `Win32_UI_WindowsAndMessaging`
- `Win32_UI_Shell`
- `Win32_UI_Accessibility` (`SetWinEventHook`, `UnhookWinEvent`)
- `Win32_Graphics_Gdi`, `Win32_Graphics_Dwm` (only if DWM is used later)
- `Win32_Storage_FileSystem` (logging)

Use `windows-sys` only when a lower-level binding is genuinely preferable. Keep `unsafe` localized
behind safe Rust interfaces. Use the focused `windows-registry` crate instead of raw
`Win32_System_Registry` bindings.

Pin Microsoft crate versions explicitly in `Cargo.toml` and verify APIs against docs.rs or the crate
source. Do not rely on remembered signatures. The crates do not share one version line:

- `windows = "0.62"` (the umbrella Win32/WinRT crate)
- `windows-registry = "0.100"`, `windows-version = "0.100"`, and `windows-canvas = "0.100"` (the
  focused native-app crates)

### 5.2 Win32 Main Window

The main window remains a classic Win32 window. Required messages:

- `WM_CREATE`, `WM_DESTROY`, `WM_CLOSE`
- `WM_PAINT`, `WM_SIZE`, `WM_DPICHANGED` (even if only logged in v1)
- `WM_TIMER` for the primary/secondary refresh ticks
- `WM_APP + n` for state changes posted from hooks or other threads
- mouse messages (`WM_LBUTTONDOWN`, `WM_LBUTTONUP`, `WM_MOUSEMOVE`, `WM_MOUSEWHEEL`,
  `WM_MBUTTONDOWN`, `WM_RBUTTONUP`) and `TrackPopupMenu` for the context menu

The window procedure stays small. It forwards to an `App`/`WindowState` object; it does not contain
window enumeration, COM calls, or drawing logic.

### 5.3 Threading and Message Model

- Initialize COM once on the UI thread with `CoInitializeEx(COINIT_APARTMENTTHREADED)`.
- **All** shell and virtual-desktop COM calls happen on that thread. The C# app achieves this by
  marshaling background loops back with `Invoke`; in Rust, remove the background threads entirely.
- Use `WM_TIMER` ticks to enumerate windows, query desktops, and invalidate. Optionally use a second
  timer for the slower watchdog. Do not use a permanent render loop.
- `SetWinEventHook` with `WINEVENT_OUTOFCONTEXT` delivers callbacks on the installing thread's message
  loop, which is the UI thread. Keep hook callbacks free of blocking COM calls where possible; store
  the handle and let the next tick use it.
- Do not block the UI thread with `SendMessage` to other processes. Use `SendMessageTimeoutW` with a
  short timeout for `WM_GETICON`.
- If a helper thread is ever added, communicate with the UI thread via `PostMessage(WM_APP + n)` and
  a small queue. Never touch application state or COM interfaces from two threads.

### 5.4 FFI Safety Rules

- No `unwrap`/`expect`/panics inside `extern "system"` callbacks (wndproc, WinEvent hook). A panic must
  not unwind across the FFI boundary; if a boundary can panic, wrap it in `catch_unwind`.
- Every Win32 handle and COM pointer gets an RAII wrapper (`HWND`, `HICON`, `HBITMAP`, `HBRUSH`,
  `HFONT`, `HMENU`, mutex handle, `IUnknown`-derived pointer).
- Check `HRESULT` explicitly. Do not convert a failed COM call into a default value.
- Document the safety contract of every `unsafe` block: which pointer must be valid, which thread it
  belongs to, and what owns it.

## 6. Virtual Desktop Subsystem

### 6.1 Structure: One Safe API, Four Thin Layout Modules

Do not port the C# file structure. The four C# files duplicate roughly 300 lines of wrapper logic each.
Use instead:

- One safe trait, for example:

  ```rust
  pub trait VirtualDesktopApi {
      fn count(&self) -> Result<u32, ApiError>;
      fn current_index(&self) -> Result<u32, ApiError>;
      fn desktop_ids(&self) -> Result<Vec<GUID>, ApiError>;
      fn index_of_window(&self, hwnd: HWND) -> Result<Option<u32>, ApiError>;
      fn switch_to(&self, index: u32) -> Result<(), ApiError>;
      fn move_window_to(&self, hwnd: HWND, index: u32) -> Result<(), ApiError>;
      fn is_application_pinned(&self, hwnd: HWND) -> Result<bool, ApiError>;
      fn pin_application(&self, hwnd: HWND) -> Result<PinOutcome, ApiError>;
      fn unpin_application(&self, hwnd: HWND) -> Result<(), ApiError>;
  }
  ```

- One generic implementation of that trait using the selected layout module.
- Two small layout modules containing only GUIDs, `#[repr(C)]` vtable prefixes, and the
  `QueryService` wiring for their build family. No application logic in these modules. The C# code has
  four variants, but the methods this application calls have identical GUIDs and vtable prefixes
  within the Win10/Win10LTSC pair and within the Win11/Win11_ pair, so two layouts are sufficient.
- Selection happens once at startup based on the build number, in one function.

Use the generated `windows::Win32::UI::Shell::IVirtualDesktopManager` for the documented interface.
Hand-define only the internal interfaces.

### 6.2 COM Interop Notes

These details are easy to get wrong and are not visible in the C# declarations:

- Class factories are activated with `CLSIDFromString` + `CoCreateInstance`, or created through
  `IServiceProvider10::QueryService` on the immersive shell instance. `QueryService` takes the service
  GUID and the requested interface IID.
- `IApplicationView` is declared `InterfaceType(ComInterfaceType.InterfaceIsIInspectable)` in all four
  C# variants. Its vtable starts with `IUnknown`, then `IInspectable`
  (`GetIids`, `GetRuntimeClassName`, `GetTrustLevel`), and only then `SetFocus`. Treating it as a plain
  `IUnknown`-derived interface shifts every method.
- `IVirtualDesktop` and `IVirtualDesktopManagerInternal` have different GUIDs *and* different vtables
  per build family. Win10/Win10LTSC use `IVirtualDesktop` `FF72FFDD-BE7E-43FC-9C03-AD81681E88E4` with
  only `IsViewVisible`/`GetId`; Win11/Win11_ use `3F07F4BE-B107-441A-AF0F-39D82529072C` with
  `GetName`/`GetWallpaperPath`/`IsRemote` appended. Win10/Win10LTSC use internal manager GUID
  `F31574D6-B682-4CDC-BD56-1827860ABEC6` and omit methods such as `MoveDesktop`, `SetDesktopName`, and
  `GetDesktopSwitchIncludeExcludeViews`; Win11/Win11_ use `53F5CA0B-158F-4124-900C-057158060B27`.
  Win11_ additionally inserts `SwitchDesktopAndMoveForegroundView` after `SwitchDesktop`.
  Never share one vtable definition across build families.
- `IVirtualDesktop::GetName` and `GetWallpaperPath` return `HSTRING` on Win11 and Win11_ only.
- C# declarations that return `bool` without `[PreserveSig]` are native `HRESULT` plus an out `BOOL`
  (`IsAppIdPinned`, `IsViewPinned`, `IsRemote`). Declare them with the out parameter; treating them as
  a native `BOOL` return reads garbage.
- `IObjectArray::GetAt(index, riid, &mut out)` takes an IID and returns a raw interface pointer. Release
  the array and every object retrieved from it.
- `IVirtualDesktopManager::GetWindowDesktopId` returns GUIDs that can be `APP_ON_ALL_DESKTOPS` or
  `WINDOW_ON_ALL_DESKTOPS`; those windows belong to the current desktop for pager purposes.
- Prefer `GetDesktopIndex` by comparing desktop GUIDs over `FindDesktop`, which has produced
  `0x800706F4` in practice.

### 6.3 Version Selection

| Build | Rust layout | Notes |
|---|---|---|
| `< 10240` | none | Not supported |
| `10240 – 21999` | Win10 | Covers Win10 and Win10LTSC; manager `F31574D6`, desktop `FF72FFDD` |
| `>= 22000` | Win11 | Covers 21H2 and later; manager `53F5CA0B`, desktop `3F07F4BE` |

The C# selector distinguishes `Win10LTSC`, `Win10`, `Win11`, and `Win11_`. The Rust port uses two
layouts because the methods it calls share the same GUIDs and vtable prefix inside each pair. The
Win11 layout has been exercised on build 26200 only. Verification on Windows 10 and on Windows 11
21H2/22H2 is postponed until suitable images are available; record results in `MIGRATION.md`, not
here.

Version detection should use `windows-version` or `RtlGetVersion`, not registry parsing. Keep the
build-number thresholds in one place.

### 6.4 Failure Handling

The C# app catches an exception during painting and recreates both COM singletons; window enumeration
feeds a per-handle failure counter. Model this explicitly:

- `ApiError` with variants for `NotSupported`, `Windows(Error)`, `UnexpectedNull`,
  `DesktopOutOfRange`, `DesktopNotFound`, `WindowNotFound`, and `InvalidString`.
- A supervisor that drops and recreates the COM objects on `RPC_E_*`, `E_FAIL`, or known shell
  failures. Recreating the singleton is behavior, not an implementation detail. The current
  `SupervisedDesktop` retries once per call and logs the failure.
- Keep the "blacklist a window after 10 desktop-index failures" rule; it prevents log spam from snapped
  window helper windows.
- Unsupported Windows builds fail with a clear, user-visible message, not a panic.

## 7. Rendering

### 7.1 v1: GDI

The current output is a few dozen filled rectangles, application icons, one-pixel borders, and one text
string per overflow tile. A GDI renderer into an off-screen memory DC (`CreateCompatibleDC` +
`CreateCompatibleBitmap`), blitted once per `WM_PAINT` with `BitBlt`, is a 1:1 behavioral match,
double-buffered, and has no device-loss story.

Use this for v1, and keep rendering independent of application logic:

- The renderer receives an immutable `ViewModel` (desktops, screens, window rectangles, icons, colors,
  active state) and produces drawing calls. It does not call COM, enumerate windows, or read settings.
- No COM calls and no window enumeration inside `WM_PAINT`.
- Reuse GDI objects (pens, brushes, fonts) for the process lifetime; do not create them per frame.
- Cache icon bitmaps per `(hwnd, hicon)` and prune them in the snapshot step.

### 7.2 Painter Abstraction

Define a small trait so a different backend can be added without touching layout code:

```rust
pub trait Painter {
    fn fill_rect(&mut self, rect: Rect, color: Color);
    fn stroke_rect(&mut self, rect: Rect, color: Color, width: i32);
    fn draw_icon(&mut self, icon: &Icon, center: Point);
    fn draw_text_centered(&mut self, text: &str, rect: Rect, color: Color);
    fn push_clip(&mut self, rect: Rect);
    fn pop_clip(&mut self);
}
```

`GdiPainter` is the only backend in v1. The trait also makes layout testable without a window.

### 7.3 Direct2D: Deferred

`windows-canvas` is real and usable (0.100.0, September 2026) but very young: two releases, a high MSRV,
and a small user base. More importantly, it does not solve the hard part of this application:

- The window uses per-window alpha (`BackgroundOpacity`). That means a layered window. Direct2D on a
  plain HWND swap chain does not give per-pixel alpha for free; it requires a composition surface or
  `UpdateLayeredWindow`.
- The workload does not need GPU acceleration. Repainting happens on state changes, not continuously.

Therefore: do not make Phase 4 depend on Direct2D. If it is added later, spike these first:

1. Hosting a Direct2D surface in a plain Win32 HWND (render target or swap chain).
2. Layered-window alpha.
3. `HICON`/`HBITMAP` to a Direct2D bitmap (WIC).
4. Text rendering for the `+N` label.
5. System DPI mapping.
6. Device-loss recovery.

Only then replace `GdiPainter` with `D2DPainter` behind the same trait.

### 7.4 GDI to Direct2D Mapping (reference only)

| GDI operation | Direct2D equivalent |
|---|---|
| `FillRect` | `FillRectangle` |
| `FrameRect` / `Rectangle` | `DrawRectangle` |
| `BitBlt` (memory DC to window) | `DrawBitmap` / present |
| `CreateSolidBrush` | `CreateSolidColorBrush` |
| `StretchBlt` | `DrawBitmap` with destination rectangle |
| `DrawText` | `DrawTextLayout` |

### 7.5 Opacity and Layering

`BackgroundOpacity` is applied to the whole window, including icons and text. In Win32, implement it
with `WS_EX_LAYERED` and `SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA)`. Do not implement
opacity by pre-multiplying every color; that produces different results and breaks icon rendering.

While the context menu is open, temporarily drop `HWND_TOPMOST` and suppress the timer-driven
topmost reassertion. Otherwise the pager covers its own popup menu. Restore topmost after
`TrackPopupMenu` returns.

### 7.6 DPI

The current process is **system DPI aware** (`app.manifest`), and all layout is computed in physical
pixels from a single scale factor (`GetDpiForSystem` / 96). Keep this for v1:

- Read the system DPI once at startup; do not react to `WM_DPICHANGED` beyond logging.
- Keep the pager height, paddings, borders, icon size, and font size as logical values multiplied by
  the scale factor.
- Document that per-monitor V2 support is out of scope for v1. It affects every coordinate, hit test,
  and icon size and is a separate project.

### 7.7 Multi-Monitor Layout

Desktops are columns. Within a desktop, one renderer per physical monitor, ordered left-to-right and
then top-to-bottom. The current app has a known limitation with vertically arranged monitors; v1 keeps
the same limitation and documents it rather than redesigning the layout model.

## 8. Settings and Persistence

### 8.1 No Settings Window in v1

The settings dialog is out of scope. There is no decision to make about `winsafe` versus
`windows-reactor` versus WinUI 3, because none of them is used. Once the dialog is gone, the C#
project no longer needs `UseWPF` or the unused `System.Windows.*` imports; that is a worthwhile
cleanup on the reference implementation as well.

"DEFAULTS only" does not mean "no configuration". The application still needs:

- A `Settings` struct with the current defaults.
- A registry reader so existing users keep their pager height, colors, and per-screen view modes, and
  so the C# and Rust builds agree during migration.
- Registry writes for the two behaviors that already persist without the dialog: pager position
  (Save) and the per-screen view-mode toggle.

The context menu remains: Toggle View Mode, Position (Restore/Save/Default), About, Exit. It must not
contain a Settings entry until a settings window exists.

### 8.2 Registry Compatibility

Path: `HKEY_CURRENT_USER\SOFTWARE\MicroTaskPager`. Keep the existing value names and types during the
migration:

| Value | Type | Default |
|---|---|---|
| `ViewModeDefault` | `REG_DWORD` | `1` (MicroTaskbar) |
| `RendererViewModes` | `REG_BINARY` | empty (one byte per desktop/screen) |
| `DesktopBorderStyle` | `REG_DWORD` | `0` (Box) |
| `BackgroundOpacity` | `REG_SZ` | `"1"` (stored as a string by C#) |
| `PagerHeight` | `REG_DWORD` | `40` |
| `IconPaddingX`, `IconPaddingY` | `REG_DWORD` | `0` |
| `DesktopBackgroundColor` | `REG_DWORD` | ARGB `0xFF404040` |
| `DesktopBorderColor` | `REG_DWORD` | ARGB `0xFF202020` |
| `ActiveDesktopBorderColor` | `REG_DWORD` | ARGB `0xFFADD8E6` (light blue) |
| `WindowBackgroundColor` | `REG_DWORD` | ARGB `0xFF808080` (gray) |
| `ActiveWindowBackgroundColor` | `REG_DWORD` | ARGB `0xFFC0C0C0` (silver) |
| `WindowBorderColor` | `REG_DWORD` | ARGB `0xFFC0C0C0` (silver) |
| `ActiveWindowBorderColor` | `REG_DWORD` | ARGB `0xFFFFFFFF` (white) |
| `PrimaryUpdateDelay` | `REG_DWORD` | `200` |
| `SecondaryUpdateDelay` | `REG_DWORD` | `500` |
| `ShowAppInTaskbar` | `REG_DWORD` | `1` |
| `PagerLocationX`, `PagerLocationY` | `REG_DWORD` | absent (use default position) |

Read tolerantly: accept both DWORD and string where a value may have been written by a different
version. Write the same types C# writes, so both implementations can share the key.

### 8.3 Settings Model

Keep the model independent of any UI:

- `Color` as a plain `u32` ARGB value or a small `Rgba` struct.
- View modes as `Vec<Vec<ViewMode>>` (desktop, screen), not a flat list. The C# flat index
  (`desktop + screen`) collides when a desktop spans multiple monitors; fix this in the model and
  translate only when importing/exporting the flat registry binary.
- Defaults implemented as `Default`, not scattered constants.

## 9. Behavioral Parity Checklist

Use this as the acceptance list. Every item should be verified against the C# build.

- [ ] Single instance (mutex) with a user-visible message on second start.
- [ ] Borderless, topmost, non-resizable pager sized from desktop count and pager height.
- [ ] Default position bottom-center above the taskbar; restore saved position; save/reset from menu.
- [ ] Drag the pager with middle mouse button or Shift + left mouse button.
- [ ] One renderer per physical monitor per desktop; per-screen view mode.
- [ ] Preview mode: scaled window rectangles, z-order, active highlighting, centered icon, borders.
- [ ] Micro Taskbar mode: icon grid, wrapping, `+N` overflow tile, active highlight.
- [ ] Left-click on another desktop switches to it.
- [ ] Left-click on the active desktop focuses the clicked window.
- [ ] Drag a miniature/icon onto another desktop moves the window there.
- [ ] Right-click menu with Toggle View Mode, Position, About, Exit.
- [ ] Mouse wheel (with Shift) toggles the view mode under the cursor and persists it.
- [ ] Window enumeration filters (shell window, invisible, empty title, CoreWindow, own process,
      unresolvable desktop) with failure blacklist.
- [ ] Icon fallback chain and icon cache.
- [ ] Window z-order and foreground/active state in the snapshot.
- [ ] Last-active-window restore on desktop switch, with one retry after 150 ms.
- [ ] Pin the app to all desktops when it appears in the taskbar; tolerate pin failure.
- [ ] Re-show and re-apply topmost when the pager is covered.
- [ ] Detect desktop count changes and monitor configuration changes; rebuild state and COM objects.
- [ ] Continue running when a COM call fails; recreate the COM objects.
- [ ] Registry import/export compatible with the C# app.
- [ ] Logs in `%LOCALAPPDATA%\MicroTaskPager\logs`, rotated, retained for 14 days.
- [ ] System DPI scaling of height, padding, borders, icons, and fonts.

## 10. Known Deviations from the C# Implementation

These are intentional fixes or simplifications. Document any additional deviation as it is made.

- Use one uniform scale factor per monitor for preview rectangles. The C# code computes width and height
  percentages separately, but both reduce to `pagerHeight / monitorHeight`, so the result is
  identical.
- Use `(desktop, screen)` view-mode indexing instead of the flat, colliding index.
- Hit-test using each renderer's actual mode, not the global default mode.
- Do not apply `PointToClient` to coordinates that are already client-space.
- Use `SendMessageTimeoutW` for `WM_GETICON` instead of a potentially blocking `SendMessage`.
- Compute foreground/active state once per snapshot instead of once per window.
- Prune desktop-lookup failure counters using every window that reaches the lookup. The C# code prunes
  against successfully added windows only, so a failing window loses its counter on every snapshot and
  never reaches the blacklist.
- Derive z-order from `EnumWindows` order and normalize it after enumeration, instead of walking
  `GetWindow(GW_HWNDNEXT)` for every window. The relative order is the same.
- Do not call COM during `WM_PAINT`; paint from the cached snapshot.
- Clip each monitor's drawing to its pager rectangle, so a window that extends across monitors cannot
  paint into the neighboring screen.
- Pre-blend the desktop separator color because GDI `FillRect` has no alpha channel.
- Clamp Micro Taskbar columns to at least one to avoid a division by zero in very narrow layouts.
- Replace OLE drag-and-drop with an internal mouse-capture drag: the payload is only a window handle
  and a source desktop, so `IDropSource`/`IDropTarget` registration is unnecessary. Show a
  semi-transparent icon drag image with the common-controls image-list drag API and keep the normal
  arrow cursor; the four-way move cursor is reserved for dragging the pager itself.
- Keep the vertical-monitor layout limitation for v1.

## 11. Migration Strategy

Do not rewrite the entire application at once. Use an incremental migration.

### Phase 1: Scaffold

- Initialize a Cargo crate in `rust/`, set the toolchain channel, add `.gitignore` for `target/`.
- Add the application manifest, icon, version resource, single-instance mutex.
- Implement logging and the settings model, defaults, and registry import.
- Configure the minimal `windows` features.

### Phase 2: Virtual Desktop Subsystem

Prioritize the technically most challenging component.

- Port the internal COM interfaces and the `QueryService` wiring.
- Build the `examples/probe` binary: count, current, index of window, switch, move, pin.
- Verify each supported Windows build and record the matrix.
- Implement the supervisor and error model.
- Do not build the UI before this works reliably.

### Phase 3: Window Model

- Window enumeration and all filters.
- Desktop index per window, failure counter, blacklist.
- Z-order, foreground state, titles, classes.
- Icon retrieval, caching, pruning.
- Immutable snapshot + change-detection hash.

### Phase 4: Main Window and Rendering

- Win32 window, message loop, timers.
- `Painter` trait, `GdiPainter`, double buffering, layered opacity.
- Preview and Micro Taskbar layout.
- Hit testing, click to switch/focus, internal drag to move a window.
- Context menu.

### Phase 5: Always-On Behavior

- Pin to all desktops.
- Topmost and covered-window watchdog.
- Foreground WinEvent hook and last-active-window restore.
- Desktop count and monitor configuration change handling with COM restart.

### Phase 6: Parity and Deployment

- Walk the checklist in section 9 against the C# build.
- Check shutdown, handle/COM leaks, and log output.
- Verify on all supported Windows builds.
- Test deployment without a .NET runtime.
- Keep the C# implementation until parity is proven; remove it only on explicit instruction.

Settings UI, Direct2D, live thumbnails, per-monitor DPI, and ARM64 are not phases of this migration.

## 12. Testing and Verification

There is no test project in the C# repository. The Rust port should not repeat that.

- Unit-test pure logic: aspect-ratio scaling, Micro Taskbar grid and overflow computation,
  change-detection hash, registry parsing (including missing/malformed values), settings defaults.
- Keep a `probe` example binary for COM and version-matrix verification; it is the practical
  integration test on each Windows build.
- Add `#[ignore]`d integration tests for mutation paths, gated by an environment variable, so they
  never move real windows during a normal `cargo test`.
- Add screenshot comparison or at least a deterministic layout test for both view modes using a fake
  `Painter`.
- Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` before declaring work
  complete.

## 13. Deployment and Packaging

- x64 MSVC build; no .NET runtime, no installer.
- `#![windows_subsystem = "windows"]` so no console window appears.
- Embed the icon and a version resource.
- Keep the application manifest with `asInvoker` and system DPI awareness.
- Keep the C# mutex name (`micro-task-pager-win-app`) for release builds so the two implementations
  cannot run simultaneously and fight over topmost/pinning. Use a different name for development
  builds if side-by-side comparison is needed.
- Log to `%LOCALAPPDATA%\MicroTaskPager\logs`.
- Decide early whether the Rust binary replaces `MicroTaskPager.exe` or ships under a different name;
  the release archive layout depends on it.

## 14. General Rust Development Guidelines

1. Prefer safe Rust APIs over direct FFI.
2. Keep `unsafe` blocks small and document their safety requirements.
3. Encapsulate all undocumented COM interfaces behind the safe `VirtualDesktopApi` trait.
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

`winsafe` is not a dependency and would not remove the need for `unsafe`: it is a wrapper for ordinary
Win32 UI work, while the virtual desktop layer uses undocumented COM vtables. Keep `unsafe` confined
to the FFI modules (`desktop/com`, `render/gdi`, `window`, `windows`, `monitors`, and the small Win32
calls in `app.rs`), and give every block a safety comment. Layout, snapshot models, settings, and the
supervisor should remain safe Rust.

## 15. Important Architectural Decisions

| Decision | Direction |
|---|---|
| Repository layout | Separate `rust/` directory |
| Rust project | Single Cargo crate initially |
| Main window | Classic Win32 |
| Rendering v1 | GDI behind a `Painter` trait |
| Rendering later | Direct2D via `windows-canvas`, only after a spike |
| Window previews | Scaled geometries + shell icons |
| Live thumbnails | Not in v1 |
| Settings UI | Not in v1; `winsafe` and `windows-reactor` not used |
| Settings storage | C#-compatible registry values |
| Virtual desktops | One safe API + Win10/Win11 layout modules |
| Threading | Single STA UI thread, `WM_TIMER` driven |
| Application icons | Windows Shell APIs |
| Application state | Pure Rust |
| .NET dependency | Removed in the final implementation |

## 16. Instructions for AI Coding Assistants

When assisting with this project:

- Read this architecture document and `AGENTS.md` before proposing structural changes.
- Prefer Microsoft's official Rust crates.
- Verify current crate APIs against their actual documentation and source code; pin versions.
- Do not invent APIs, methods, COM interfaces, or vtable layouts.
- Pay particular attention to undocumented Windows interfaces; never assume a COM interface definition
  is identical across Windows builds.
- Explain all required `unsafe` operations and their safety assumptions.
- Keep version-specific Windows compatibility code isolated.
- Do not add a settings UI, DWM thumbnails, or Direct2D unless explicitly asked.
- Avoid unnecessary architectural complexity.
- Prefer small, independently testable implementation steps.
- Preserve existing application behavior unless a deviation is listed in section 10 or explicitly
  requested.
- When porting existing C# functionality, use the existing implementation as the behavioral reference.

The primary objective is a reliable, maintainable, fully native Windows application implemented in
Rust, not merely a line-by-line translation of the original C# code.
