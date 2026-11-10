# micro-task-pager-core – Shared Platform Architecture

Status: shared contract for every Rust Micro Task Pager front end. The core crate is deliberately
**UI-toolkit independent**: it owns COM, virtual desktops, window enumeration, the registry,
monitors, foreground tracking, logging, the single-instance guard, and the pure dialog helpers.
Rendering, input, widgets, and dialog windows belong to a front end.

The front-end documents describe the toolkit-specific layers:

- [`../../rust-winsafe/ARCHITECTURE.md`](../../rust-winsafe/ARCHITECTURE.md) – stable native Win32
  front end (`winsafe` + GDI).
- [`../../rust-winui/ARCHITECTURE.md`](../../rust-winui/ARCHITECTURE.md) – experimental WinUI 3
  front end (`windows-reactor`).

This document and [`README.md`](README.md) must stay consistent. The README is the quick onboarding
map; this document is the architectural contract and behavior reference.

## 1. Purpose and Boundary

### 1.1 Workspace

The three crates form one Cargo workspace rooted at the repository `Cargo.toml`:

| Directory | Package | Library | Binary |
| --- | --- | --- | --- |
| `crates/micro-task-pager-core/` | `micro-task-pager-core` | `micro_task_pager_core` | none |
| `rust-winsafe/` | `micro-task-pager-winsafe` | `micro_task_pager_winsafe` | `micro-task-pager-winsafe` |
| `rust-winui/` | `micro-task-pager-winui` | `micro_task_pager_winui` | `micro-task-pager-winui` |

Shared build configuration lives at the workspace root: `Cargo.toml` (membership and release
profile), `.cargo/config.toml` (static MSVC runtime), `rust-toolchain.toml`, `target/`, and
`Cargo.lock`. Running `cargo` from a member directory selects that package; output still lands in
the root `target/`.

### 1.2 Where a module goes

- **Core** if it only needs Win32, COM, the registry, or plain Rust and would be useful to a
  different UI toolkit.
- **Front end** if it needs the toolkit's types, renderer, layout units (DIPs vs device pixels), or
  window handles.

The pager layout is the clearest example: `rust-winsafe` works in device pixels with GDI rectangles,
while `rust-winui` works in DIPs with pixel-snapped floating-point geometry. They serve the same
behavior contract but stay in their own crates. The settings draft and dialog placement are pure and
shared.

Shared platform behavior never gets copied into a front end. If a front end needs a new platform
capability, add it to the core crate and re-export it.

### 1.3 Module ownership

| Module | Owns |
| --- | --- |
| `com` | RAII single-threaded COM apartment on the UI thread. |
| `desktop` | Safe virtual-desktop API, layout selection, restart supervisor, error model. |
| `desktop::com` (private) | GUIDs, `#[repr(C)]` vtable prefixes, and `QueryService` wiring. |
| `windows` | Top-level window enumeration, filtering, snapshot, change hash, icon handle cache. |
| `settings` | C#-compatible registry values, defaults, pager location, view-mode bytes. |
| `monitors` | Physical monitor enumeration and left-to-right/top-to-bottom ordering. |
| `foreground` | `EVENT_SYSTEM_FOREGROUND` hook and per-desktop last-active tracking. |
| `single_instance` | Named-mutex guard shared by all implementations. |
| `logging` | Daily file logger in `%LOCALAPPDATA%\MicroTaskPager\logs` with 14-day retention. |
| `dialogs::placement` | Pure Settings window placement in physical pixels. |
| `dialogs::settings_draft` | Pure Settings working copy: revert, reset, clamps. |

Everything except the COM modules is safe Rust. `unsafe` is confined to `desktop/com`, `windows`,
`monitors`, `foreground`, `single_instance`, and `logging`, and every block carries a `SAFETY`
comment.

## 2. Application Behavior Contract

This is the behavior every front end must preserve. It is the common part of the C# behavioral
reference; UI-specific behavior is documented in the front-end files.

### 2.1 Main Window

`MainForm.cs` is a single borderless, always-on-top window with no control box:

- The pager is one window; virtual desktops are drawn inside it, not as child windows.
- Its width is the sum of all desktop widths; its height is the configured pager height scaled by
  DPI.
- Default position is bottom-center of the primary screen's working area. The user moves it with
  the middle mouse button or Shift + left mouse button and can save, restore, or reset the position
  from the context menu (registry-backed).
- Right-click opens a context menu: Toggle View Mode, Position (Restore/Save/Default), Settings,
  About, Exit. Exit must remain because the window has no system menu.
- Background opacity is the whole window's alpha, i.e. a layered window. Do not simulate it by
  pre-multiplying every color; that breaks icons and text.

### 2.2 View Modes

Each desktop is drawn as a row of one renderer per physical monitor. Each renderer independently
uses one of two modes, remembered per desktop/screen:

**Preview** is a scaled geometry map, not a live screen image. For every visible window on the
desktop and monitor it:

1. Takes the window rectangle in screen coordinates and subtracts the monitor origin.
2. Scales the rectangle into pager space.
3. Fills it with a solid color (active/inactive) and draws the application icon centered in it.
4. Draws a border.

Windows are drawn in z-order (back to front). Clicking a miniature selects that window. There is
no DWM thumbnail and no screen capture anywhere, and a front end must not introduce one.

**Micro Taskbar** draws a centered grid of application icons for the desktop/monitor. It computes
columns and rows from the available size and icon padding and reserves the last cell for a `+N`
overflow label when not all windows fit. Active windows get a highlighted tile background.

### 2.3 Always-On Behavior

- Pinning itself to all virtual desktops (only possible when the app appears in the taskbar; a
  failed auto-pin at startup is expected and tolerated).
- Topmost plus a 500 ms watchdog: if `WindowFromPoint` at the pager's center returns another
  window, the pager re-shows itself with `ShowWindowAsync(SW_SHOWNOACTIVATE)` and re-applies
  topmost.
- A single-instance mutex so two pagers cannot fight over topmost and pinning.
- The pager restores the last active window per desktop after a desktop switch (see 2.5).

### 2.4 Refresh Model

There is no render loop. Two polling intervals (default 200 ms primary, 500 ms secondary) build a
window snapshot, compute a change-detection hash, and repaint only when the hash changes. Desktop
count and monitor configuration are checked on the slower tick and cause a full rebuild of state and
COM objects.

The snapshot fetches the desktop GUID list once per tick and resolves every window against that list
with `window_desktop_id`. Calling `index_of_window` per window re-enumerates the desktop list through
COM for every window, which dominates idle CPU. The change-detection hash itself is cheap and is not
the limiting factor.

How the polling loop is expressed is toolkit-specific: `rust-winsafe` uses `WM_TIMER`; `rust-winui`
uses a self-rescheduling reactor tick. In both cases the work runs on the UI thread.

### 2.5 Last Active Window per Desktop

`ActiveWindowTracker` installs a `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` hook and remembers the
last real foreground window for each desktop. When the user switches back to a desktop, the pager
restores that window, retrying once after 150 ms because the desktop switch animation can still be
in progress.

## 3. COM and Threading Contract

### 3.1 Apartment Lifecycle

The virtual-desktop COM objects are apartment-threaded:

1. Create `com::Apartment` on the thread that will use the desktop API.
2. Keep that guard alive for as long as any desktop object exists.
3. Keep every `desktop` call, and every `windows::WindowSnapshotter::capture` call, on that same
   thread.

`Apartment` is an RAII guard: `init` calls `CoInitializeEx` with `COINIT_APARTMENTTHREADED`; `Drop`
calls `CoUninitialize`. The types are not `Send`. In `rust-winsafe` the owning thread is the winsafe
message-loop thread; in `rust-winui` it is the `windows-reactor` UI thread.

### 3.2 Thread Ownership

- One UI thread owns the COM objects, the virtual-desktop API, window enumeration, the window host
  or component, and drawing.
- Background work may sleep, perform pure work, or post a message. It never calls shell COM or
  touches application state.
- `SetWinEventHook` with `WINEVENT_OUTOFCONTEXT` delivers callbacks on the installing thread's
  message loop. The callback stores only a handle in an atomic; the next tick does the COM lookup.
- Do not block the UI thread with `SendMessage` to other processes. Icon retrieval uses
  `SendMessageTimeoutW` with a short timeout.

### 3.3 FFI Safety Rules

- No `unwrap`/`expect`/panics inside `extern "system"` callbacks. A panic must not unwind across
  the FFI boundary; wrap a boundary that can panic in `catch_unwind`.
- Every Win32 handle and COM pointer gets an RAII wrapper (`HWND`, `HICON`, `HBITMAP`, `HBRUSH`,
  `HFONT`, `HMENU`, mutex handle, `IUnknown`-derived pointer).
- Check `HRESULT` explicitly. Do not convert a failed COM call into a default value.
- Document the safety contract of every `unsafe` block: which pointer must be valid, which thread it
  belongs to, and what owns it.
- Keep `unsafe` out of the supervisor, settings, and dialog helpers.

## 4. Virtual Desktop Subsystem

### 4.1 One Safe API

Do not port the C# file structure. The four C# files duplicate wrapper logic. The core crate uses
one safe trait:

```rust
pub trait VirtualDesktopApi {
    fn count(&self) -> Result<u32, ApiError>;
    fn current_index(&self) -> Result<u32, ApiError>;
    fn desktop_ids(&self) -> Result<Vec<GUID>, ApiError>;
    fn current_desktop_id(&self) -> Result<Option<GUID>, ApiError>;
    fn window_desktop_id(&self, hwnd: HWND) -> Result<Option<WindowDesktopId>, ApiError>;
    fn index_of_window(&self, hwnd: HWND) -> Result<Option<u32>, ApiError>;
    fn switch_to(&self, index: u32) -> Result<(), ApiError>;
    fn move_window_to(&self, hwnd: HWND, index: u32) -> Result<(), ApiError>;
    fn is_application_pinned(&self, hwnd: HWND) -> Result<bool, ApiError>;
    fn pin_application(&self, hwnd: HWND) -> Result<PinOutcome, ApiError>;
    fn unpin_application(&self, hwnd: HWND) -> Result<(), ApiError>;
}
```

One generic implementation of the trait uses the selected layout module. The layout modules contain
only GUIDs, `#[repr(C)]` vtable prefixes, and `QueryService` wiring; there is no application logic
in them.

`desktop::open()` is the normal entry point. It returns a `SupervisedDesktop` that implements the
trait and recreates the backend after a restartable failure.

### 4.2 Layouts and Version Selection

The C# selector distinguishes `Win10LTSC`, `Win10`, `Win11`, and `Win11_`. The Rust port uses two
layouts because the methods it calls share the same GUIDs and vtable prefix inside each pair:

- Win10 layout: build 10240 through 21999 (Win10 and Win10LTSC).
- Win11 layout: build 22000 and later.

| Build | Rust layout | Notes |
| --- | --- | --- |
| `< 10240` | none | Not supported by the virtual-desktop backend |
| `10240 – 21999` | Win10 | Manager `F31574D6`, desktop `FF72FFDD` |
| `>= 22000` | Win11 | Manager `53F5CA0B`, desktop `3F07F4BE` |

Selection happens once at startup from the build number in `desktop::com`, which keeps the threshold
logic unit-testable. Version detection uses `windows-version`, not registry parsing.

The virtual-desktop backend supports build 10240 and later. The WinUI **application** requires
Windows 10 1809 (17763) or later because of the Windows App Runtime; `rust-winsafe` supports the
same backend floor as the C# build. Verification on Windows 10 and Windows 11 21H2/22H2 remains
postponed.

### 4.3 COM Interop Notes

These details are easy to get wrong and are not visible in the C# declarations:

- Class factories are activated with `CLSIDFromString` + `CoCreateInstance`, or created through
  `IServiceProvider10::QueryService` on the immersive shell instance. `QueryService` takes the
  service GUID and the requested interface IID.
- `IApplicationView` is declared `InterfaceType(ComInterfaceType.InterfaceIsIInspectable)` in all
  four C# variants. Its vtable starts with `IUnknown`, then `IInspectable` (`GetIids`,
  `GetRuntimeClassName`, `GetTrustLevel`), and only then `SetFocus`. Treating it as a plain
  `IUnknown`-derived interface shifts every method.
- `IVirtualDesktop` and `IVirtualDesktopManagerInternal` have different GUIDs and different vtables
  per build family. Win10/Win10LTSC use `IVirtualDesktop`
  `FF72FFDD-BE7E-43FC-9C03-AD81681E88E4` with only `IsViewVisible`/`GetId`; Win11/Win11_ use
  `3F07F4BE-B107-441A-AF0F-39D82529072C` with `GetName`/`GetWallpaperPath`/`IsRemote` appended.
  Win10/Win10LTSC use internal manager GUID `F31574D6-B682-4CDC-BD56-1827860ABEC6` and omit methods
  such as `MoveDesktop`, `SetDesktopName`, and `GetDesktopSwitchIncludeExcludeViews`; Win11/Win11_
  use `53F5CA0B-158F-4124-900C-057158060B27`. Win11_ additionally inserts
  `SwitchDesktopAndMoveForegroundView` after `SwitchDesktop`. Never share one vtable definition
  across build families.
- `IVirtualDesktop::GetName` and `GetWallpaperPath` return `HSTRING` on Win11 and Win11_ only.
- C# declarations that return `bool` without `[PreserveSig]` are native `HRESULT` plus an out
  `BOOL` (`IsAppIdPinned`, `IsViewPinned`, `IsRemote`). Declare them with the out parameter;
  treating them as a native `BOOL` return reads garbage.
- `IObjectArray::GetAt(index, riid, &mut out)` takes an IID and returns a raw interface pointer.
  Release the array and every object retrieved from it.
- `IVirtualDesktopManager::GetWindowDesktopId` returns GUIDs that can be `APP_ON_ALL_DESKTOPS` or
  `WINDOW_ON_ALL_DESKTOPS`; those windows belong to the current desktop for pager purposes.
  `window_desktop_id` keeps that case explicit as `WindowDesktopId::AllDesktops`.
- Prefer resolving indices by comparing desktop GUIDs over `FindDesktop`, which has produced
  `0x800706F4` in practice.
- Use the generated `windows::Win32::UI::Shell::IVirtualDesktopManager` for the documented
  interface. Hand-define only the internal interfaces.

### 4.4 Failure Handling and the Supervisor

`ApiError` has variants for `NotSupported`, `Windows(Error)`, `UnexpectedNull`,
`DesktopOutOfRange`, `DesktopNotFound`, `WindowNotFound`, and `InvalidString`.

`SupervisedDesktop` watches the backend:

- `ApiError::is_restartable` recognizes RPC errors (`0x8001xxxx`), `E_FAIL`, `E_UNEXPECTED`, and
  `0x800706F4`.
- On a restartable failure it drops the COM backend, creates a fresh one, and retries once per call.
  Recreating the singleton is behavior, not an implementation detail.
- Non-restartable errors are returned immediately. They are usually per-window lookup failures, which
  the snapshotter counts and blacklists; logging every one would flood the file.
- Unsupported Windows builds fail with a clear, user-visible message, not a panic.

The retry logic is unit-tested with an injected backend factory, so the supervisor tests do not need
real COM objects.

## 5. Window Model

`windows::WindowSnapshotter::capture` enumerates top-level windows and produces an immutable
`WindowSnapshot`. It draws nothing; it is the layer between the COM desktop API and the UI.

### 5.1 Filters

- Skip the shell window, invisible windows, windows without a title, and the
  `Windows.UI.Core.CoreWindow` class (Start menu).
- Skip windows owned by the pager process.
- Skip windows whose virtual desktop cannot be determined. Failures are counted per handle and the
  handle is blacklisted after 10 failures (snapped-window helper windows trigger this).

### 5.2 Snapshot Data

Read title, class, process ID, rectangle, foreground state, and z-order. Each `WindowInfo` also
records `desktop_index`, `is_active`, `is_minimized`, and a borrowed icon handle. Windows are sorted
by handle for a stable list order.

Z-order comes from `EnumWindows` order (top to bottom); the snapshot normalizes it so larger values
are closer to the top. The relative order matches the C# `GetWindow(GW_HWNDNEXT)` walk.

### 5.3 Desktop Resolution and Blacklist

`capture` fetches `desktop_ids` once, resolves the current index from `current_desktop_id`, and maps
each window with `window_desktop_id`. Pinned windows (`AllDesktops`) follow the current desktop.

A per-handle failure counter is pruned using every window that reaches the desktop lookup. The C#
code prunes against successfully added windows only, so a failing window loses its counter on every
snapshot and never reaches the blacklist; the Rust behavior fixes that.

### 5.4 Change Detection

`WindowSnapshot::hash` covers every field that affects rendering: desktop count and GUIDs, current
desktop, foreground, and every window's handle, title, class, process, rectangle, desktop, z-order,
active/minimized state, and icon handle. A UI skips layout and icon work when the hash is unchanged.

## 6. Settings and Persistence

### 6.1 Registry Path

Path: `HKEY_CURRENT_USER\SOFTWARE\MicroTaskPager`. Keep the existing value names and types so the
C#, winsafe, and WinUI implementations share one key:

| Value | Type | Default |
| --- | --- | --- |
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
version. Missing or malformed values fall back to defaults instead of failing. Write the same types
the C# code writes.

### 6.2 Settings Model

`Settings` is independent of any UI:

- Colors are plain `u32` ARGB values, matching the integers the C# application writes.
- `ViewMode` and `DesktopBorderStyle` are small enums with C#-compatible registry values.
- Defaults are implemented as `Default`, not scattered constants.
- `renderer_view_modes` is a flat `Vec<ViewMode>` because the registry stores one byte per renderer.
  Each front end maps it to its own `(desktop, screen)` order once monitor enumeration is available
  and writes it back flat through `save_view_modes`.

`Settings::save` writes every value, including the flat view-mode bytes. A front end that owns live
per-desktop modes must set `renderer_view_modes` from its current layout before saving, so a later
settings write cannot discard a view-mode toggle.

`load_pager_location` and `save_pager_location` use the same key.

## 7. Monitors and Foreground Tracking

### 7.1 Monitors

`monitors::enumerate` uses `EnumDisplayMonitors` and returns `MonitorInfo` values with the full
monitor rectangle, the work rectangle, and the primary flag. Monitors are sorted left-to-right and
then top-to-bottom, matching the C# `OrderBy(Bounds.Left).ThenBy(Bounds.Top)`.

Monitor rectangles are always physical virtual-screen pixels. Each front end decides how to convert
them into its own layout units. A single-screen fallback keeps the pager usable if enumeration
returns nothing.

### 7.2 Foreground Tracking

`foreground::ForegroundTracker` owns the hook and the per-desktop last-active map. The hook callback
cannot touch the desktop API, so it stores the window handle in a process-wide atomic.
`take_pending` consumes it on the UI thread; `remember` maps it to a desktop; `restore` re-activates
the remembered window after a switch. Tracking ignores the pager's own window, other windows of the
pager process, and windows without a title.

The front end calls `restore` immediately after a switch and again after a 150 ms retry.

## 8. Process Helpers

### 8.1 Single Instance

`single_instance::acquire` creates the named mutex `micro-task-pager-win-app` with
`bInitialOwner = true`. A second process gets `ERROR_ALREADY_EXISTS`, shows the shared
"already running" message with `show_already_running`, and exits before creating a window. This
prevents two implementations from fighting over topmost and desktop pinning.

The release mutex name is shared with the C# application. A development build may use a different
name for side-by-side comparison.

### 8.2 Logging

`logging::init` creates `%LOCALAPPDATA%\MicroTaskPager\logs`, removes files older than 14 days, and
writes a session marker. `info`, `warn`, and `error` append one line to today's `log-YYYY-MM-DD.txt`.
The logger opens the file per message; there is no background writer, and a logging failure never
takes down the application.

The process start order in every front end is:

```text
logging::init() -> com::Apartment::init() -> single_instance::acquire() -> UI loop
```

## 9. Dialog Helpers

Both helpers are pure Rust with unit tests. They contain no Win32 drawing or window creation.

### 9.1 Placement

`dialogs::placement::find_placement` mirrors the C# `AppSettingsWindow.FindPlacement` sequence:
prefer the right of the pager, then the left, below, above, and finally center over the pager. A
candidate is accepted only when the whole dialog fits inside one monitor's work area. Inputs and
outputs are physical-pixel `Rect` values; the front end applies the result to its window.

### 9.2 Settings Draft

`dialogs::settings_draft::SettingsDraft` is the editable working copy behind the Settings dialog.
It keeps:

- the values the dialog opened with (`original`),
- the current working copy,
- per-option revert to the opening values,
- per-section reset to built-in defaults,
- the C# clamp rules and the minimum pager height above the icon padding.

The dialog never edits the live `Settings` value. Only OK persists; Cancel and window close restore
the opening values. The draft is toolkit-independent; each front end maps controls to its
`set_number`, `set_index`, `set_check`, `set_color`, and `set_opacity_percent` methods.

## 10. Icons

`WindowInfo::icon` is a borrowed `WindowIcon` wrapper around a shell `HICON`. The window, its class,
or the system owns the handle.

- `WindowIcon::handle()` returns the raw handle for the front end's renderer.
- Do **not** call `DestroyIcon` on it.
- If a front end needs a scaled or converted copy (a GDI bitmap for winsafe, a PNG for WinUI), it
  must make its own copy first.
- `WindowSnapshotter` caches only the handle. Encoding or bitmap conversion belongs in the front end,
  because it is toolkit-specific.

The lookup chain is `WM_GETICON` (`ICON_SMALL2`), then the window class icon (`GCLP_HICON`), then
`IDI_APPLICATION`. `SendMessageTimeoutW` with a 100 ms timeout prevents a hung application from
blocking the pager.

## 11. Behavioral Parity Checklist

Every item must be verified against the C# build for both front ends, except where a front-end
document records an intentional deviation.

### 11.1 Shared Intentional Deviations from C#

These deviations are implemented in the core crate and therefore apply to both Rust front ends:

- Use one uniform scale factor per monitor for preview rectangles. The C# code computes width and
  height percentages separately, but both reduce to `pagerHeight / monitorHeight`, so the result is
  identical.
- Use `SendMessageTimeoutW` for `WM_GETICON` instead of a potentially blocking `SendMessage`.
- Compute foreground/active state once per snapshot instead of once per window.
- Prune desktop-lookup failure counters using every window that reaches the lookup (see 5.3).
- Derive z-order from `EnumWindows` order and normalize it after enumeration, instead of walking
  `GetWindow(GW_HWNDNEXT)` for every window. The relative order is the same.
- Fetch the desktop GUID list once per snapshot and resolve every window against it, instead of
  re-enumerating the desktop list per window.
- Use `(desktop, screen)` view-mode indexing in the front ends instead of the C# flat, colliding
  index; the registry format stays flat.
- Hit-test using each renderer's actual mode, not the global default mode.
- Clip each monitor's preview to its pager rectangle, so a window that extends across monitors cannot
  draw into the neighboring screen.
- Clamp Micro Taskbar columns to at least one to avoid a division by zero in very narrow layouts.
- Count Micro Taskbar windows per screen. The C# dialog counts windows from other screens, which
  leaves secondary-screen icons uncentered.
- Keep the C# limitation for vertically arranged monitors until the layout model is redesigned.
- Read settings tolerantly and fall back to defaults for missing or malformed values.

### 11.2 Checklist

- [ ] Single instance (mutex) with a user-visible message on second start.
- [ ] Borderless, topmost, non-resizable pager sized from desktop count and pager height.
- [ ] Default position bottom-center above the taskbar; restore saved position; save/reset from
      menu.
- [ ] Drag the pager with middle mouse button or Shift + left mouse button.
- [ ] One renderer per physical monitor per desktop; per-screen view mode.
- [ ] Preview mode: scaled window rectangles, z-order, active highlighting, centered icon, borders.
- [ ] Micro Taskbar mode: icon grid, wrapping, `+N` overflow tile, active highlight.
- [ ] Left-click on another desktop switches to it.
- [ ] Left-click on the active desktop focuses the clicked window.
- [ ] Drag a miniature/icon onto another desktop moves the window there.
- [ ] Right-click menu with Toggle View Mode, Position, Settings, About, Exit.
- [ ] Mouse wheel (with Shift) toggles the view mode under the cursor and persists it.
- [ ] Window enumeration filters (shell window, invisible, empty title, CoreWindow, own process,
      unresolvable desktop) with failure blacklist.
- [ ] Icon fallback chain and icon cache.
- [ ] Window z-order and foreground/active state in the snapshot.
- [ ] Last-active-window restore on desktop switch, with one retry after 150 ms.
- [ ] Pin the app to all desktops when it appears in the taskbar; tolerate pin failure.
- [ ] Re-show and re-apply topmost when the pager is covered.
- [ ] Detect desktop count changes and monitor configuration changes; rebuild state and COM
      objects.
- [ ] Continue running when a COM call fails; recreate the COM objects.
- [ ] Registry import/export compatible with the C# app.
- [ ] Logs in `%LOCALAPPDATA%\MicroTaskPager\logs`, rotated, retained for 14 days.
- [ ] About content matches the C# dialog (logo, version, author, acknowledgments, Close).
- [ ] Settings exposes the same options, limits, revert, and reset-per-section behavior as the C#
      dialog.

## 12. Testing and the Probe

The core crate has unit tests for:

- the supervisor and its restart decision,
- layout-selection thresholds,
- settings decoding, opacity parsing, and defaults,
- snapshot hashing and desktop resolution,
- dialog placement,
- the settings draft (revert, reset, clamps).

Run them with `cargo test -p micro-task-pager-core`.

The standalone COM probe lives in `examples/probe` and is shared by every front end. It initializes
COM itself and exercises the same code path as the pager:

```powershell
cargo run -p micro-task-pager-core --example probe
cargo run -p micro-task-pager-core --example probe -- snapshot
cargo run -p micro-task-pager-core --example probe -- index 0x1234
cargo run -p micro-task-pager-core --example probe -- pinned 0x1234
cargo run -p micro-task-pager-core --example probe -- switch 1
cargo run -p micro-task-pager-core --example probe -- window 60
cargo run -p micro-task-pager-core --example probe -- move 0x1234 1
cargo run -p micro-task-pager-core --example probe -- pin 0x1234
cargo run -p micro-task-pager-core --example probe -- unpin 0x1234
```

Run the read-only commands first. `move`, `pin`, `unpin`, and `switch` change real window or shell
state. Use `window` to create a throwaway probe-owned window instead of moving a real application.

Virtual-desktop interop is undocumented and build-sensitive. Verify changes on real Windows 10 and
Windows 11 builds rather than assuming one COM layout.

## 13. Adding or Extending a Front End

1. Create the crate directory and add it to `members` in the root `Cargo.toml`.
2. Depend on the core crate by path and pick a unique package, library, and binary name:

   ```toml
   [package]
   name = "micro-task-pager-egui"
   version = "1.5.1"
   edition = "2024"
   publish = false

   [lib]
   name = "micro_task_pager_egui"
   path = "src/lib.rs"

   [[bin]]
   name = "micro-task-pager-egui"
   path = "src/main.rs"

   [dependencies]
   micro-task-pager-core = { path = "../crates/micro-task-pager-core" }
   ```

3. Start the process in the same order: `logging::init()` → `com::Apartment::init()` →
   `single_instance::acquire()` → the UI loop, with the apartment guard outliving the loop.
4. Re-export the core modules if the front end wants the existing `crate::desktop` /
   `crate::settings` paths:

   ```rust
   pub use micro_task_pager_core::{
       com, desktop, foreground, logging, monitors, settings, single_instance, windows,
   };
   ```

5. Add the pure dialog helpers explicitly if the front end uses them:

   ```rust
   use micro_task_pager_core::dialogs::{placement, settings_draft};
   ```

6. Keep the UI in the new crate. Do not add rendering, widgets, or dialog windows to the core crate.

## 14. Development Guidelines

1. Prefer safe Rust APIs over direct FFI.
2. Keep `unsafe` blocks small and document their safety requirements.
3. Encapsulate all undocumented COM interfaces behind the safe `VirtualDesktopApi` trait.
4. Avoid leaking raw Windows handles across application layers.
5. Use RAII wrappers for COM and other Windows resources.
6. Handle `HRESULT` errors explicitly; no silent fallbacks.
7. Respect COM apartment and thread-affinity requirements; use one STA thread.
8. Avoid unnecessary dependencies.
9. Keep application logic independent of the window and the renderer.
10. Write compatibility tests for undocumented Windows APIs.
11. Do not assume Windows implementation details remain stable across updates.
12. Prefer native Windows functionality over unnecessary abstractions.
13. No panics across FFI boundaries.
14. Keep the settings model free of Win32 types where practical.
15. Put shared behavior in the core crate, not in a front end.
