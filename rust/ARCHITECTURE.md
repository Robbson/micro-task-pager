# Rust Migration – Architecture and Development Guidelines

Status: active architecture. The **target UI is WinUI 3 through `windows-reactor`**. The native Win32
v1 implementation is complete and remains in the repository as the behavioral reference and shipping
implementation until the WinUI cutover. New UI work must follow this document, not extend the native
Win32 UI.

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
- Keep the code compact, readable, and easy to maintain.
- Provide a modern Windows look and feel through WinUI 3.
- Keep runtime overhead low: no permanent render loop; refresh only when the snapshot changes.

Microsoft's official `windows-rs` ecosystem is preferred wherever practical. `windows-reactor` is the
UI framework; the raw `windows` crate is for platform interop only.

The existing C# implementation is the behavioral reference. When in doubt, read the C# code rather
than guessing what the application does.

### 1.1 Non-Goals

- DWM live thumbnails or screen capture. Window previews remain scaled geometry maps.
- A custom GPU renderer. WinUI composition is used instead of Direct2D/GDI.
- A home-grown UI framework or widget toolkit. `windows-reactor` is the UI abstraction.
- ARM64. The x64 build runs under emulation on ARM64 Windows 11.
- Desktop creation, removal, or renaming.
- Installer, auto-update, and code signing.
- XAML files, XAML compilation, or a XAML designer. Reactor views are Rust code.

### 1.2 Architecture Priority

When goals conflict, use this order:

1. Compact, readable, maintainable code.
2. Modern Windows behavior and appearance.
3. Behavioral parity with the C# application.
4. Dependency independence.

The project accepts the Windows App SDK runtime dependency. It is small relative to the .NET
Framework deployment it replaces and removes hundreds of lines of native UI assembly code from this
repository.

## 2. What the Existing Application Actually Does

This section describes the behavior that has to survive the migration. It is the behavioral contract.

### 2.1 Main Window

`MainForm.cs` is a single borderless, always-on-top window with no control box:

- The pager is one window; virtual desktops are drawn inside it, not as child windows.
- Its width is the sum of all desktop widths; its height is the configured pager height scaled by DPI.
- Default position is bottom-center of the primary screen's working area. The user can move it with
  the middle mouse button or Shift + left mouse button, and can save, restore, or reset the position
  from the context menu (stored in the registry).
- Right-click opens a context menu: Toggle View Mode, Position (Restore/Save/Default), Settings,
  About, Exit. Exit must remain, because the window has no system menu.
- Background opacity is the window's alpha (`Form.Opacity`), i.e. a layered window. The target
  implementation uses `WS_EX_LAYERED` with `SetLayeredWindowAttributes` on the WinUI window.
- The C# process is system DPI aware. The WinUI target is DPI aware through WinUI's DIP layout.

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
DWM thumbnail and no screen capture** anywhere in the current implementation, and the migrated
application must not introduce one.

**Micro Taskbar** draws a centered grid of application icons for the desktop/monitor. It computes
columns and rows from the available size and icon padding, and reserves the last cell for a `+N`
overflow label when not all windows fit. Active windows get a highlighted tile background.

### 2.3 Settings and Persistence

`AppSettings` is loaded from and saved to `HKEY_CURRENT_USER\SOFTWARE\MicroTaskPager`
(`AppSettingsStore.cs`). The pager position is stored under the same key (`RegistryAccess.cs`).

The C# settings dialog is a separate window built programmatically with WinForms. It has three tabs:
Layout, Theme, and Misc. It supports per-option revert, reset per tab, live preview where safe, and
numeric limits. The Rust target ports this behavior as a reactor component.

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

The WinUI target keeps this behavior. Pinning and topmost are applied through the `native_window`
shim because `windows-reactor` does not expose a public HWND/AppWindow accessor.

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

The native implementation has no render loop. Two polling loops (200 ms primary, 500 ms secondary)
build a window snapshot, compute a change-detection hash, and invalidate the window only when the hash
changes. Desktop count and monitor configuration are checked on the slower loop and cause a full
rebuild of all desktop and COM objects.

The WinUI implementation keeps the same polling intervals but expresses them as reactor messages. A
`spawn_background` task sleeps and returns a tick message; the component then refreshes on the UI
thread and schedules the next tick. No COM work runs on the thread-pool thread.

## 3. UI Architecture: WinUI 3 via windows-reactor

### 3.1 The Decision

`windows-reactor` is the application's UI framework. The pager, About, and Settings are reactor
components. Native Win32 is an implementation detail used only for shell interop and the few window
properties that reactor does not expose.

Do **not** build a framework, widget toolkit, layout engine, dialog runtime, or drawing abstraction
on top of Win32. That approach was tried for the native About dialog and produced a 650-line file for
a static window. The equivalent reactor component is about 85 lines.

The dependency trade is accepted:

- `windows-reactor` is the UI dependency, not an experiment. It is a dev-dependency today and becomes
  a normal dependency in Phase 8.
- The Windows App SDK runtime is required (framework package) or bundled (self-contained).
- This is still dramatically smaller than a .NET Framework deployment and removes the largest source
  of accidental UI complexity in this repository.

### 3.2 Why Reactor

- Declarative views make the structure of a screen obvious.
- WinUI provides modern controls, layout, input, focus, accessibility, and DPI handling.
- It supports secondary windows, so About and Settings can be normal application windows without a
  native dialog runtime.
- It supports context menus, pointer capture, pointer positions, keyed lists, and drag operations.
- It is code-first; no XAML files or XAML compiler are needed.

### 3.3 Component Model

Each screen is one `Component` with a small message type:

```rust
impl Component for Pager {
    type Input = PagerInput;
    type Message = Message;

    fn create(input: &Self::Input, context: &ComponentContext<Self>) -> Self;
    fn update(&mut self, message: Message, context: &ComponentContext<Self>);
    fn view(&self, input: &Self::Input, context: &mut ViewContext<Self>) -> View;
}
```

Rules:

- `view` is declarative and contains layout, styling, and event wiring only.
- `update` contains state transitions and side effects.
- Platform work is called from `update` through safe modules, never from `view`.
- One screen equals one component. Do not split a screen into a custom widget hierarchy unless the
  split has a clear behavioral reason.
- Use keys for desktop/window collections so identity survives refreshes.

### 3.4 Window Model

- The pager is the root component started by `App::run_component::<Pager>`.
- About and Settings are secondary windows opened with `ComponentContext::open_window`.
- Secondary windows can be opened and closed repeatedly; the one-shot `Application.Start` lifecycle
  is not a problem because the pager application remains alive.
- There are no native `DialogBox` / `TrackPopupMenu` calls in the target UI.

### 3.5 Reactivity and Timers

Reactor does not expose a timer API. Use a self-rescheduling tick:

```rust
fn schedule_refresh(&self, context: &ComponentContext<Self>) {
    let delay = self.settings.primary_update_delay;
    _ = context.spawn_background(move |_token| {
        std::thread::sleep(std::time::Duration::from_millis(delay));
        Message::Tick
    });
}
```

The background closure only sleeps and returns a message. The `update` handler performs COM and window
enumeration on the UI thread, where the STA lives.

### 3.6 Native Window Shim

`windows-reactor` 0.100 does not expose a public HWND or AppWindow accessor. A small
`native_window.rs` module owns the gap:

- Find the pager HWND by process ID and window title.
- Apply `WS_POPUP`, remove caption/thick-frame/system-menu styles, and set `WS_EX_TOOLWINDOW` or
  `WS_EX_APPWINDOW` according to `ShowAppInTaskbar`.
- Apply `WS_EX_LAYERED` and `SetLayeredWindowAttributes` for window opacity.
- Set the initial/saved position.
- Reapply topmost on the tick. WinUI can reapply its own styles after a style change, so topmost is
  treated as a state to maintain, not a one-time setting.
- Pass the HWND to the virtual-desktop pinning API.

This shim is deliberately small. If it starts growing into a UI abstraction, stop and move the
missing capability upstream to `windows-reactor` instead.

Validated on build 26200: a reactor window can be made borderless, tool-window, layered with alpha,
and topmost. The title bar disappears and the desktop shows through the alpha. WinUI reset topmost
once during the probe; reasserting it works.

### 3.7 Threading with Reactor

- Reactor owns the UI thread and runs the WinUI application loop.
- Initialize the COM apartment on that same thread before starting the reactor app.
- All virtual-desktop COM and window enumeration calls stay on that thread.
- Background reactor tasks only sleep, perform pure work, or return a message. They never call shell
  COM or touch application state.
- `SetWinEventHook` callbacks still deliver to the UI thread. Store only the handle and let the next
  tick do the COM lookup.

### 3.8 Readability Rules

These rules exist to keep the application understandable without building a custom framework:

- A screen component should read top-to-bottom: state, messages, `update`, then `view`.
- Keep `view` declarative. If it needs a loop, loop over model data, not over Win32 calls.
- Put pure geometry in `layout.rs` and pure input transitions in `input.rs`.
- Split a module before it exceeds a few hundred lines. About should be roughly 80-100 lines; a pager
  screen with its view, layout, and input split should stay well below the native implementation.
- Prefer named structs (`Rect`, `Cell`, `WindowTile`) over tuples and parallel arrays.
- Do not create a generic abstraction for one call site. Introduce a helper only when it removes real
  repetition without hiding control flow.
- Delete the native path as soon as the reactor path passes its checklist. Dual UI implementations are
  the opposite of maintainable.

## 4. Technology Stack

| Area | Technology | Status |
|---|---|---|
| Programming language | Rust (stable, MSVC toolchain) | decided |
| UI framework | `windows-reactor` 0.100 (WinUI 3) | decided |
| Native Windows bindings | `windows` 0.62 / `windows-core` | decided |
| COM interoperability | `windows-core` and hand-written shell vtables | decided |
| Main window | Reactor root component + `native_window` shim | decided |
| Rendering | WinUI composition (`Canvas`, `Grid`, `Border`, `Image`, `TextBlock`) | decided |
| Window previews | Scaled window rectangles + shell icons (no DWM) | decided |
| Application icons | Shell APIs converted to reactor image sources | decided |
| Context menu | Reactor `Menu` | decided |
| Settings UI | Reactor secondary window | decided |
| About | Reactor secondary window | decided |
| Settings storage | Windows registry, C#-compatible | decided |
| Runtime | Windows App Runtime 2.x | decided |
| Minimum OS | Windows 10 1809 (build 17763) | decided |
| Logging | In-house file logger, daily file, 14-day retention | decided |
| Build system | Cargo | decided |
| Testing | `cargo test`, reactor component tests, `examples/probe` | decided |
| GDI / `Painter` / Direct2D | Removed after the WinUI cutover | deprecated |
| Native Win32 dialogs | Removed after the WinUI cutover | deprecated |

Do not add another GUI framework (`winsafe`, `egui`, another WinUI wrapper) alongside reactor. Do not
extend the native Win32 UI.

## 5. Repository Structure

The Rust implementation lives in `rust/` inside the existing Git repository. The C# implementation
remains available during migration and is the reference for behavior.

The target module layout is:

```
rust/
  Cargo.toml
  Cargo.lock                    # committed: this is a binary crate
  rust-toolchain.toml
  ARCHITECTURE.md
  MIGRATION.md
  src/
    main.rs                     # logging, COM apartment, single instance, reactor bootstrap
    lib.rs
    pager/
      mod.rs                     # Pager component: state, messages, update
      view.rs                    # declarative Canvas/Grid view
      layout.rs                  # pure layout in DIPs, unit-tested
      input.rs                   # pure hit testing and drag state
      icons.rs                   # HICON -> reactor image source
    dialogs/
      about.rs                   # reactor About window
      settings.rs                # reactor Settings window
    native_window.rs             # HWND lookup and pager-specific window styles
    settings.rs                  # model, defaults, registry import/export
    logging.rs
    com.rs
    single_instance.rs
    foreground.rs
    desktop/
      mod.rs
      com/mod.rs
      com/core.rs
      com/win10.rs
      com/win11.rs
    windows/
      mod.rs
      icons.rs
    monitors.rs
  examples/
    probe/                       # standalone COM verification binary
    reactor_about.rs             # reactor About reference / regression sample
```

The native v1 modules `window.rs`, `render/`, and `about.rs` are transitional. They are removed after
the reactor pager reaches parity.

## 6. Native Windows Integration

### 6.1 Crates and Features

Use the official crates:

- `windows = "0.62"` for Win32, COM, shell, and logging APIs.
- `windows-core` for COM traits and error types.
- `windows-reactor = "0.100"` for UI.
- `windows-registry = "0.100"` and `windows-version = "0.100"` where applicable.

`windows-reactor` uses the 0.100 line of `windows-core`; the application's raw `windows` bindings use
0.62. The two can coexist. Keep the versions pinned and verify APIs against the actual crate source.

Enable only the `windows` features that are used. After the cutover, the UI-related features shrink to
the platform modules: COM, shell, threading, library loader, accessibility, file system, registry,
monitors, and the small `native_window` surface.

### 6.2 Native Surface

Native Win32 code is limited to:

- `desktop/` - undocumented virtual-desktop COM.
- `windows/` - window enumeration, filtering, icons, snapshot.
- `foreground.rs` - WinEvent hook.
- `single_instance.rs` - named mutex.
- `monitors.rs` - monitor geometry and work areas.
- `logging.rs` - file logging and file-system calls.
- `native_window.rs` - pager window styles, topmost, opacity, position, HWND lookup.
- `main.rs` - process bootstrap and COM apartment.

UI components must not contain raw handles, message constants, or `unsafe` blocks.

### 6.3 Threading and Message Model

- Reactor owns the UI thread and the WinUI message loop.
- Initialize the COM apartment on the UI thread before `App::run_component`.
- All virtual-desktop COM calls happen on that thread.
- Use the self-rescheduling reactor tick instead of `WM_TIMER`.
- `SetWinEventHook` with `WINEVENT_OUTOFCONTEXT` delivers callbacks on the installing thread's message
  loop, which is the UI thread. Keep the callback free of blocking COM calls.
- Do not block the UI thread with `SendMessage` to other processes. Use `SendMessageTimeoutW` with a
  short timeout for `WM_GETICON`.
- Never touch application state or COM interfaces from two threads.

### 6.4 FFI Safety Rules

- No `unwrap`/`expect`/panics inside `extern "system"` callbacks. A panic must not unwind across the
  FFI boundary; wrap a boundary that can panic in `catch_unwind`.
- Every Win32 handle and COM pointer gets an RAII wrapper (`HWND`, `HICON`, `HBITMAP`, `HBRUSH`,
  `HFONT`, `HMENU`, mutex handle, `IUnknown`-derived pointer).
- Check `HRESULT` explicitly. Do not convert a failed COM call into a default value.
- Document the safety contract of every `unsafe` block: which pointer must be valid, which thread it
  belongs to, and what owns it.
- Keep `unsafe` out of reactor components, layout, hit testing, settings, and the supervisor.

### 6.5 WinUI Bootstrap and Packaging

- `windows-reactor` bootstraps the Windows App Runtime 2.x framework package.
- The target minimum OS is Windows 10 1809 (build 17763), the minimum supported by the runtime.
- Keep the application manifest `asInvoker`. WinUI 3 provides DPI-aware layout; verify the final
  DPI-awareness manifest entry during Phase 12 instead of relying on the native v1 system-DPI setting.
- Decide between framework-dependent (small app, runtime installed) and self-contained (larger app)
  during the packaging phase. The architecture supports either.

## 7. Virtual Desktop Subsystem

### 7.1 Structure: One Safe API, Two Layout Modules

Do not port the C# file structure. The four C# files duplicate roughly 300 lines of wrapper logic each.
Use instead:

- One safe trait:

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

### 7.2 COM Interop Notes

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

### 7.3 Version Selection

| Build | Rust layout | Notes |
|---|---|---|
| `< 10240` | none | Not supported by the virtual-desktop backend |
| `10240 – 21999` | Win10 | Covers Win10 and Win10LTSC; manager `F31574D6`, desktop `FF72FFDD` |
| `>= 22000` | Win11 | Covers 21H2 and later; manager `53F5CA0B`, desktop `3F07F4BE` |

The virtual-desktop backend still supports build 10240 and later. The **application** requires Windows
10 1809 (17763) or later because of the Windows App Runtime. Builds below 17763 cannot run the WinUI
target; document this in the user-facing requirements at cutover.

The C# selector distinguishes `Win10LTSC`, `Win10`, `Win11`, and `Win11_`. The Rust port uses two
layouts because the methods it calls share the same GUIDs and vtable prefix inside each pair. The
Win11 layout has been exercised on build 26200 only. Verification on Windows 10 and on Windows 11
21H2/22H2 is postponed until suitable images are available; record results in `MIGRATION.md`, not
here.

Version detection should use `windows-version` or `RtlGetVersion`, not registry parsing. Keep the
build-number thresholds in one place.

### 7.4 Failure Handling

The C# app catches an exception during painting and recreates both COM singletons; window enumeration
feeds a per-handle failure counter. Model this explicitly:

- `ApiError` with variants for `NotSupported`, `Windows(Error)`, `UnexpectedNull`,
  `DesktopOutOfRange`, `DesktopNotFound`, `WindowNotFound`, and `InvalidString`.
- A supervisor that drops and recreates the COM objects on `RPC_E_*`, `E_FAIL`, or known shell
  failures. Recreating the singleton is behavior, not an implementation detail. `SupervisedDesktop`
  retries once per call and logs the failure.
- Keep the "blacklist a window after 10 desktop-index failures" rule; it prevents log spam from snapped
  window helper windows.
- Unsupported Windows builds fail with a clear, user-visible message, not a panic.

## 8. Pager UI

### 8.1 View Model

The pager component consumes the immutable snapshot and produces a `View`. The view model is pure data:

- Desktop columns.
- One screen per monitor per desktop.
- Window rectangles, icons, active state, and z-order.
- Per-screen view mode.
- Hover/drag state.

Keep the layout and hit-testing code separate from the view. They operate in DIPs and are unit-tested
without WinUI.

### 8.2 Preview Mode

Preview mode is a `Canvas` with one child per visible window. Each child is a small `Grid` or
`Border` composition containing:

- A `Border` for the window rectangle with the configured background and border colors.
- An `Image` for the centered application icon.
- A highlight for the active window.
- Children ordered back-to-front to match z-order.

Do not use `Border` layout to reproduce the scaled geometry; compute the rectangles in
`pager/layout.rs` and position them with `Canvas::left`/`Canvas::top`.

### 8.3 Micro Taskbar Mode

Micro Taskbar is a `Grid` or `Canvas` of icon tiles:

- Compute columns, rows, icon size, and the `+N` overflow tile in `pager/layout.rs`.
- Render each tile as a `Border` containing an `Image` and optional text.
- Highlight the active window tile.

### 8.4 Input and Interaction

- Use reactor pointer events for press/move/release and capture.
- Left-click on another desktop switches to it.
- Left-click on the active desktop focuses the clicked window.
- Middle-drag or Shift+left-drag moves the pager; use screen coordinates and `native_window` to move
  the HWND, matching the C# behavior and avoiding client-coordinate feedback loops.
- Drag a miniature/icon to another desktop to move the window. The drag ghost is a reactor element in a
  `Canvas`; the drop uses the existing `move_window_to` API.
- Shift+wheel toggles the view mode under the cursor and persists it.
- Right-click opens a reactor `Menu` with Toggle View Mode, Position, Settings, About, and Exit.
- While the menu is open, suspend topmost reassertion so the pager does not cover its own popup.

### 8.5 Refresh and Identity

- Use the tick pattern from section 3.5.
- Recompute the snapshot and layout only when the primary tick fires.
- Use the change-detection hash to skip republishing identical views.
- Use keys for desktop and window collections so reactor can preserve element identity.

### 8.6 Icons

The icon cache currently returns `HICON`. Reactor `Image` needs image data:

- Convert each cached icon to a PNG or bitmap source once, then cache the converted source.
- Keep the `HICON` fallback chain and cache key `(hwnd, hicon)`.
- Prune converted sources when the window disappears.
- Do not decode icons per frame or per view publication.

### 8.7 Opacity and Topmost

- Apply `BackgroundOpacity` through `WS_EX_LAYERED` and
  `SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA)` in `native_window`.
- Do not simulate opacity by pre-multiplying every color; it breaks icons and text.
- Reapply `HWND_TOPMOST` on the secondary tick. WinUI may reset window styles after style changes.
- The covered-window watchdog remains: `WindowFromPoint`, `ShowWindowAsync(SW_SHOWNOACTIVATE)`, then
  reapply topmost.

### 8.8 DPI and Multi-Monitor

- Reactor and WinUI use DIPs; remove the manual `scale()` helpers from UI layout.
- WinUI handles per-monitor DPI. This is an intentional improvement over the C# system-DPI behavior.
- Keep the monitor enumeration used to compute pager width and preview rectangles.
- Keep the C# limitation for vertically arranged monitors until the layout model is redesigned.

## 9. Dialogs

### 9.1 About

The About dialog is a reactor secondary window:

- 600 x 480 DIP client area.
- Logo image from the embedded `logo.png`.
- Version, author, acknowledgments, and a Close button.
- Title "About Micro Task Pager".
- Light theme, white background, centered content.

`examples/reactor_about.rs` is the reference implementation. Promote it into
`src/dialogs/about.rs` during Phase 10 and delete the native `src/about.rs`.

### 9.2 Settings

The Settings dialog is a reactor secondary window:

- Sections corresponding to the C# tabs: Layout, Theme, Misc.
- A working copy of `Settings`; edits do not touch the live settings until Apply/OK.
- Apply, OK, Cancel, and reset-per-section.
- Per-option revert where the C# dialog supports it.
- Live preview for safe changes (opacity, colors, height, view mode) by publishing a preview settings
  object to the pager component.
- Numeric clamps and validation before writing.
- Registry write through the existing `settings.rs` model; keep C#-compatible value names and types.

Preferred WinUI controls: `NumberBox`, `Slider`, `ColorPicker` or color fields, `ToggleSwitch`,
`ComboBox`, `Button`, and `TabView` or a sectioned `NavigationView`. Do not recreate WinForms controls
or build custom widgets.

## 10. Settings and Persistence

### 10.1 Settings UI

The settings dialog is part of the WinUI target. It is no longer out of scope.

### 10.2 Registry Compatibility

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

### 10.3 Settings Model

Keep the model independent of any UI:

- `Color` as a plain `u32` ARGB value or a small `Rgba` struct.
- View modes as `Vec<Vec<ViewMode>>` (desktop, screen), not a flat list. The C# flat index
  (`desktop + screen`) collides when a desktop spans multiple monitors; fix this in the model and
  translate only when importing/exporting the flat registry binary.
- Defaults implemented as `Default`, not scattered constants.
- Settings changes are values, not UI controls. The pager component receives settings and rebuilds the
  view; it never reads registry values during layout.

## 11. Behavioral Parity Checklist

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
- [ ] Right-click menu with Toggle View Mode, Position, Settings, About, Exit.
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
- [ ] About content matches the C# dialog (logo, version, author, acknowledgments, Close).
- [ ] Settings exposes the same options, limits, revert, and reset-per-section behavior as the C# dialog.
- [ ] DPI scaling is correct on high-DPI and mixed-DPI monitors.
- [ ] WinUI look and feel: themed controls, focus visuals, keyboard navigation, accessibility names.

## 12. Known Deviations from the C# Implementation

These are intentional fixes, simplifications, or platform changes. Document any additional deviation
as it is made.

- Replace the native WinForms UI with WinUI 3; the look and control behavior differ by design.
- Use WinUI DIPs and per-monitor DPI instead of the C# system-DPI scaling.
- Require Windows 10 1809 (17763) or later because of the Windows App Runtime.
- Ship with a Windows App Runtime dependency (framework package or self-contained).
- Use one uniform scale factor per monitor for preview rectangles, with the monitor rectangle converted
  to the same DIP space. The C# code computes width and height percentages separately, but both reduce
  to `pagerHeight / monitorHeight`, so the result is identical.
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
- No COM during painting; the view is built from the cached snapshot.
- Clip each monitor's preview to its pager rectangle, so a window that extends across monitors cannot
  paint into the neighboring screen.
- Clamp Micro Taskbar columns to at least one to avoid a division by zero in very narrow layouts.
- Replace OLE drag-and-drop with an internal pointer-capture drag: the payload is only a window handle
  and a source desktop, so `IDropSource`/`IDropTarget` registration is unnecessary. The drag ghost is a
  reactor element.
- Keep the vertical-monitor layout limitation.
- Keep global window opacity through the layered-window shim; do not approximate it by pre-multiplying
  colors.
- Use a reactor `Menu` instead of `TrackPopupMenu`; topmost is suspended while the menu is open.

## 13. Migration Strategy

The native Win32 v1 completed Phases 1-6. The UI migration to WinUI 3 is a new phase sequence. Do not
rewrite everything at once, and keep the native v1 runnable until the reactor pager passes the
checklist.

### Phase 7: Architecture and Documentation

- Update `ARCHITECTURE.md`, `MIGRATION.md`, `AGENTS.md`, and dependency notices for the WinUI direction.
- Record the window-shim and layered-alpha probe results.
- No application behavior changes.

### Phase 8: Pager Shell

- Add `windows-reactor` as a normal dependency.
- Create `pager/` component, `native_window.rs`, and a reactor root window.
- Apply borderless/tool-window/layered/topmost styles and the saved position.
- Find the HWND and pin to all desktops.
- Add the self-rescheduling tick and refresh the snapshot.
- Acceptance: pinned, topmost, opacity works, no title bar, clean shutdown, no COM off the UI thread.

### Phase 9: Pager Content

- Port layout and hit testing to DIPs.
- Preview and Micro Taskbar views with desktop/window cells.
- Click-to-switch/focus, pager drag, window drag, Shift+wheel, context menu.
- Icon conversion and keyed lists.
- Acceptance: checklist items for the pager UI.

### Phase 10: Dialogs

- Promote `examples/reactor_about.rs` to `dialogs/about.rs`.
- Build `dialogs/settings.rs` with tabs, working copy, validation, apply/cancel, and live preview.
- Delete the native `about.rs`; the native v1 never had a Settings dialog.
- Acceptance: About and Settings match the C# behavior and look modern.

### Phase 11: Cutover and Cleanup

- Make the reactor pager the main binary.
- Delete `window.rs`, `render/`, native `about.rs`, and the `windows` UI features that are no longer
  used.
- Remove `Paint`/GDI references from docs and tests.
- Keep `examples/reactor_about.rs` only if it remains a useful regression sample.

### Phase 12: Verification and Packaging

- Walk the section 11 checklist against the C# build.
- Verify on supported Windows builds and mixed-DPI monitors.
- Decide framework-dependent vs self-contained runtime packaging.
- Update user-facing requirements for Windows 10 1809 and the Windows App Runtime.
- Keep the C# implementation until parity is proven; remove it only on explicit instruction.

## 14. Testing and Verification

- Unit-test pure logic: DIP layout, hit testing, Micro Taskbar grid/overflow, change-detection hash,
  icon conversion, registry parsing, settings defaults.
- Use reactor's component test facilities where they fit for view/message tests. Do not build a custom
  UI test framework.
- Keep the `probe` example for COM and version-matrix verification.
- Add screenshot comparison or at least deterministic layout tests for both view modes.
- Add manual UI smoke tests for borderless/topmost/opacity/pinning after style changes.
- Verify no COM or window enumeration happens on a background thread.
- Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and
  `cargo test` before declaring work complete.

## 15. Deployment and Packaging

- x64 MSVC build; no .NET runtime and no installer.
- `#![windows_subsystem = "windows"]` so no console window appears.
- Statically link the MSVC runtime (`rust/.cargo/config.toml`) so the executable does not require the
  Visual C++ redistributable.
- Windows App Runtime 2.x is required. Choose framework-dependent or self-contained during Phase 12.
- Minimum OS: Windows 10 1809 (build 17763).
- Embed the icon and version resource.
- Keep the application manifest `asInvoker`. Verify the final WinUI DPI-awareness manifest entry in
  Phase 12.
- Keep the C# mutex name (`micro-task-pager-win-app`) for release builds so the two implementations
  cannot run simultaneously and fight over topmost/pinning. Use a different name for development
  builds if side-by-side comparison is needed.
- Log to `%LOCALAPPDATA%\MicroTaskPager\logs`.
- The release artifact is `rust/target/release/micro-task-pager.exe`. A distribution folder may rename
  it to `MicroTaskPager.exe`; the embedded icon/version/manifest stay intact.

## 16. Rust Development Guidelines

1. Prefer safe Rust APIs over direct FFI.
2. Keep `unsafe` blocks small and document their safety requirements.
3. Encapsulate all undocumented COM interfaces behind the safe `VirtualDesktopApi` trait.
4. Avoid leaking raw Windows handles across application layers.
5. Use RAII wrappers for COM and other Windows resources.
6. Handle `HRESULT` errors explicitly; no silent fallbacks.
7. Respect COM apartment and thread-affinity requirements; use one STA thread.
8. Optimize for readability first. Compact, declarative code beats clever abstraction.
9. One screen equals one component; `view` is the only layout code.
10. Do not build a UI framework or widget toolkit on Win32. Use `windows-reactor`.
11. Do not add Win32 UI code to components. Platform behavior lives in native modules.
12. Keep layout and hit testing pure and testable.
13. Use DIPs in UI code; do not reintroduce manual DPI scaling.
14. Keep the settings model independent of the UI.
15. Do not assume Windows implementation details remain stable across updates.
16. No panics across FFI boundaries.

`winsafe` is not a dependency. It would not remove the `unsafe` needed for the undocumented
virtual-desktop COM layer, and it would compete with reactor. `windows-reactor` is the chosen UI
framework. Keep `unsafe` confined to the platform modules (`desktop/com`, `windows`, `monitors`,
`foreground`, `single_instance`, `logging`, `native_window`, and the COM bootstrap in `main.rs`) and
give every block a safety comment. Components, layout, hit testing, settings, and the supervisor stay
safe Rust.

## 17. Important Architectural Decisions

| Decision | Direction |
|---|---|
| Repository layout | Separate `rust/` directory |
| Rust project | Single Cargo crate initially |
| UI framework | WinUI 3 through `windows-reactor` |
| Main window | Reactor root component + native window shim |
| Rendering | WinUI composition |
| Window previews | Scaled geometries + shell icons |
| Live thumbnails | Not planned |
| Context menu | Reactor `Menu` |
| Settings UI | Reactor secondary window |
| About | Reactor secondary window |
| Native Win32 | Platform glue only |
| Virtual desktops | One safe API + Win10/Win11 layout modules |
| Threading | Reactor UI thread, single STA for COM |
| Application icons | Shell APIs converted to reactor image sources |
| Application state | Pure Rust |
| Runtime | Windows App Runtime 2.x |
| Minimum OS | Windows 10 1809 (17763) |
| .NET dependency | Removed in the final implementation |
| C# implementation | Reference and shipping until the WinUI cutover |

## 18. Instructions for AI Coding Assistants

When assisting with this project:

- Read this architecture document, `MIGRATION.md`, and `AGENTS.md` before proposing structural changes.
- Treat WinUI 3 through `windows-reactor` as the UI architecture. Do not propose a native Win32
  rewrite, a custom UI framework, `winsafe`, `egui`, or another WinUI wrapper.
- Keep UI components declarative and small. If a screen needs hundreds of lines, split state, layout,
  input, and platform work.
- Do not put `unsafe`, raw handles, or message constants in components.
- Prefer Microsoft's official Rust crates.
- Verify current crate APIs against their actual documentation and source code; pin versions.
- Do not invent APIs, methods, COM interfaces, or vtable layouts.
- Pay particular attention to undocumented Windows interfaces; never assume a COM interface definition
  is identical across Windows builds.
- Explain all required `unsafe` operations and their safety assumptions.
- Keep version-specific Windows compatibility code isolated.
- Do not add DWM thumbnails or a custom Direct2D/GDI renderer.
- Avoid unnecessary architectural complexity.
- Prefer small, independently testable implementation steps.
- Preserve existing application behavior unless a deviation is listed in section 12 or explicitly
  requested.
- When porting existing C# functionality, use the existing implementation as the behavioral reference.

The primary objective is a reliable, maintainable, compact Windows application implemented in Rust,
with a modern WinUI 3 UI and no .NET runtime dependency.
