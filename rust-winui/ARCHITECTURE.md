# Rust WinUI 3 – Architecture

Status: experimental implementation. The UI is **WinUI 3 through `windows-reactor`**. This front end
currently matches the `rust-winsafe` feature set, but it is not planned to be updated in the near
future because `windows-reactor` is not stable yet; it also uses considerably more resources and a
large runtime dependency. The stable implementation is `rust-winsafe`, and `csharp-winforms` has the
latest features.

This document covers only what is specific to the WinUI front end. The shared platform contract,
application behavior, COM rules, window model, settings, and the parity checklist live in
[`../crates/micro-task-pager-core/ARCHITECTURE.md`](../crates/micro-task-pager-core/ARCHITECTURE.md).
Phase status and verification evidence live in [`MIGRATION.md`](MIGRATION.md).

## 1. Scope and Shared Core

### 1.1 What the front end owns

`rust-winui` owns the WinUI 3 user interface and the few native-window properties WinUI does not
expose:

- the reactor pager component,
- the DIP layout, hit testing, and pointer state machine,
- the WinUI icon conversion (`HICON` → encoded PNG),
- the About and Settings reactor windows,
- `native_window.rs` (HWND lookup, styles, topmost, opacity, position, context menu, wheel hook),
- the application manifest and resources,
- the Windows App Runtime bootstrap and names.

Everything else comes from `micro-task-pager-core`: COM bootstrap, virtual desktops, window
enumeration, settings, logging, single instance, monitor/foreground tracking, the settings draft, and
dialog placement. The core modules are re-exported from `src/lib.rs`, so internal paths such as
`crate::desktop` and `crate::settings` keep working.

### 1.2 Relationship to `rust-winsafe`

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
- WinUI phase status and open questions: [`MIGRATION.md`](MIGRATION.md)
- The stable native counterpart: [`../rust-winsafe/ARCHITECTURE.md`](../rust-winsafe/ARCHITECTURE.md)

## 2. WinUI 3 UI Architecture

### 2.1 Decision

`windows-reactor` is the application's UI framework. The pager, About, and Settings are reactor
components. Native Win32 is an implementation detail used only for shell interop and the few window
properties reactor does not expose.

Do not build a framework, widget toolkit, layout engine, dialog runtime, or drawing abstraction on
top of Win32. That approach was tried for the native About dialog and produced a 650-line file for a
static window; the equivalent reactor component is about 85 lines.

The dependency trade is accepted:

- `windows-reactor` is the UI dependency, not an experiment. It is a normal dependency.
- The Windows App SDK runtime is required (framework package).
- This is still dramatically smaller than a .NET Framework deployment and removes the largest source
  of accidental UI complexity.

### 2.2 Why Reactor

- Declarative views make the structure of a screen obvious.
- WinUI provides modern controls, layout, input, focus, accessibility, and DPI handling.
- It supports secondary windows, so About and Settings are normal application windows without a
  native dialog runtime.
- It supports context menus as flyouts, pointer capture, pointer positions, keyed lists, and drag
  operations. Version 0.100 `Menu` is a Button/SplitButton flyout, not a right-click context menu;
  the pager therefore uses a native `TrackPopupMenu` in `native_window` and revisits true context
  menus with a later reactor release.
- It is code-first; no XAML files or XAML compiler are needed.

### 2.3 Component Model

Each screen is one `Component` with a small message type:

```rust
impl Component for Pager {
    type Input = ();
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

### 2.4 Window Model

- The pager is the root component started by `pager::run`.
- About and Settings are secondary windows opened with `ComponentContext::open_window`.
- Secondary windows can be opened and closed repeatedly; the one-shot `Application.Start` lifecycle
  is not a problem because the pager application remains alive.
- There are no native `DialogBox` calls in the UI. The context menu is the one temporary exception:
  `native_window::show_context_menu` uses `TrackPopupMenu` because reactor 0.100 has no right-click
  context menu. Replace it when reactor exposes transient menus.
- `pager/dialog_state.rs` owns the lifecycle of the single secondary window. Reactor does not report
  a reliable close notification, so the state machine moves from `Closed` to `Opening` to `Open` and
  back by watching the HWND disappear.

### 2.5 Reactivity and Timers

Reactor does not expose a timer API. Use a self-rescheduling tick:

```rust
fn schedule_refresh(&self, context: &ComponentContext<Self>) {
    let delay = self.settings.primary_update_delay;
    _ = context.spawn_background(move |_token| {
        std::thread::sleep(std::time::Duration::from_millis(delay));
        Message::PrimaryTick
    });
}
```

The background closure only sleeps and returns a message. The `update` handler performs COM and
window enumeration on the UI thread, where the STA lives.

### 2.6 Native Window Shim

`windows-reactor` 0.100 does not expose a public HWND or AppWindow accessor. `native_window.rs` owns
that gap:

- Find the pager HWND by process ID and window title.
- Apply `WS_POPUP`, remove caption/thick-frame/system-menu styles, and set `WS_EX_TOOLWINDOW` or
  `WS_EX_APPWINDOW` according to `ShowAppInTaskbar`.
- Reapply the target client size in DIPs after the frame change. Reactor resizes its client before
  the shim removes the caption, so the first outer size still includes the old non-client area.
- Apply `WS_EX_LAYERED` and `SetLayeredWindowAttributes` for window opacity.
- Set the initial/saved position.
- Reapply topmost on the tick. WinUI can reapply its own styles after a style change, so topmost is
  treated as a state to maintain, not a one-time setting.
- Pass the HWND to the virtual-desktop pinning API.
- Provide the temporary context menu, activation helper, drag cursor helper, and a `WH_MOUSE_LL`
  wheel hook.

This shim is deliberately small. If it starts growing into a UI abstraction, stop and move the
missing capability upstream to `windows-reactor` instead.

Validated on build 26200: a reactor window can be made borderless, tool-window, layered with alpha,
and topmost. The title bar disappears and the desktop shows through the alpha. WinUI reset topmost
once during the probe; reasserting it works.

### 2.7 Threading with Reactor

- Reactor owns the UI thread and runs the WinUI application loop.
- Initialize the COM apartment on that same thread before starting the reactor app.
- All virtual-desktop COM and window enumeration calls stay on that thread.
- Background reactor tasks only sleep, perform pure work, or return a message. They never call shell
  COM or touch application state.
- `SetWinEventHook` callbacks still deliver to the UI thread. Store only the handle and let the next
  tick do the COM lookup.

### 2.8 Readability Rules

- A screen component should read top-to-bottom: state, messages, `update`, then `view`.
- Keep `view` declarative. If it needs a loop, loop over model data, not over Win32 calls.
- Put pure geometry in `pager/layout.rs`, pure input transitions in `pager/input.rs`, and dialog
  lifecycle in `pager/dialog_state.rs`.
- Split a module before it exceeds a few hundred lines.
- Prefer named structs (`Rect`, `Cell`, `WindowTile`) over tuples and parallel arrays.
- Do not create a generic abstraction for one call site.
- Do not reintroduce a native UI path. Dual UI implementations are the opposite of maintainable.

## 3. Technology Stack

| Area | Technology | Status |
| --- | --- | --- |
| Programming language | Rust (stable, MSVC toolchain) | decided |
| UI framework | `windows-reactor` 0.100 (WinUI 3) | decided |
| Native Windows bindings | `windows` 0.62 / `windows-core` | decided |
| Shared platform | `micro-task-pager-core` (path dependency) | decided |
| Main window | Reactor root component + `native_window` shim | decided |
| Rendering | WinUI composition (`Canvas`, `Grid`, `Border`, `Image`, `TextBlock`) | decided |
| Window previews | Scaled window rectangles + shell icons (no DWM) | decided |
| Application icons | Shell `HICON` converted to reactor image sources | decided |
| Context menu | Native `TrackPopupMenu` in `native_window` (temporary) | decided |
| Settings UI | Reactor secondary window | decided |
| About | Reactor secondary window | decided |
| Settings storage | Windows registry through `micro-task-pager-core` | decided |
| Runtime | Windows App Runtime 2.x | decided |
| Minimum OS | Windows 10 1809 (build 17763) | decided |
| Logging | Core crate: daily file, 14-day retention | decided |
| Build system | Cargo workspace | decided |
| Testing | `cargo test`, reactor component tests, shared `probe` example | decided |

Do not add another GUI framework (`winsafe`, `egui`, another WinUI wrapper) alongside reactor. Do not
extend a native Win32 UI.

## 4. Repository Structure

The WinUI front end lives in `rust-winui/`. The shared core lives in
`crates/micro-task-pager-core/`; the native winsafe front end lives in `rust-winsafe/`.

```
rust-winui/
  Cargo.toml
  app.manifest                  # PerMonitorV2 DPI awareness
  app.rc                        # icon and version resource
  build.rs                      # embeds resources and the manifest
  ARCHITECTURE.md
  COMPARISON.md
  MIGRATION.md
  UI-FRAMEWORK-COMPARISON.md
  src/
    main.rs                     # logging, COM apartment, single instance, pager
    lib.rs                      # re-exports the core modules
    pager/
      mod.rs                    # Pager component: state, messages, update
      view.rs                   # declarative Canvas/Grid view
      layout.rs                 # pure layout in DIPs, unit-tested
      input.rs                  # pure hit testing and drag state
      icons.rs                  # HICON -> reactor image source
      dialog_state.rs           # lifecycle of the single secondary dialog
    dialogs/
      mod.rs                    # DialogLink/events
      about.rs                  # reactor About window
      settings/
        mod.rs                  # Settings component and preview loop
        view.rs                 # Pivot/cards view and color dialog
    native_window.rs            # HWND lookup and pager-specific window styles
  examples/
    reactor_about.rs            # reactor About reference / regression sample
    reactor_settings.rs         # standalone Settings window runner
    reactor_pager.rs            # reactor pager runner
```

The retired native v1 modules (`app.rs`, `window.rs`, `render/`, `about.rs`) and the native UI were
removed at the Phase 11 cutover. `src/main.rs` starts the reactor pager.

## 5. Pager UI

### 5.1 View Model

The pager component consumes the immutable `WindowSnapshot` and produces a `View`. The view model is
pure data:

- Desktop columns.
- One screen per monitor per desktop.
- Window rectangles, icons, active state, and z-order.
- Per-screen view mode.
- Hover/drag state (drag ghost in `input.rs`; hover highlighting is deferred).

Layout and hit testing operate in DIPs and are unit-tested without WinUI.

### 5.2 Preview Mode

Preview mode is a `Canvas` with one child per visible window. Each child is a small `Grid` or
`Border` composition containing:

- a `Border` for the window rectangle with the configured background and border colors,
- an `Image` for the centered application icon,
- a highlight for the active window,
- children ordered back-to-front to match z-order.

Do not use `Border` layout to reproduce the scaled geometry; compute the rectangles in
`pager/layout.rs` and position them with `Canvas::left`/`Canvas::top`.

### 5.3 Micro Taskbar Mode

Micro Taskbar is a `Grid` or `Canvas` of icon tiles:

- Compute columns, rows, icon size, and the `+N` overflow tile in `pager/layout.rs`.
- Render each tile as a `Border` containing an `Image` and optional text.
- Highlight the active window tile.

### 5.4 Input and Interaction

- Use reactor pointer events for press/move/release and capture.
- Left-click on another desktop switches to it.
- Left-click on the active desktop focuses the clicked window.
- Middle-drag or Shift+left-drag moves the pager; use screen coordinates and `native_window` to move
  the HWND, matching the C# behavior and avoiding client-coordinate feedback loops.
- Drag a miniature/icon to another desktop to move the window. The drag ghost is a reactor element
  in a `Canvas`; the drop uses the existing `move_window_to` API.
- Shift+wheel toggles the view mode under the cursor and persists it. Reactor 0.100 has no wheel
  event, and WinUI delivers wheel input to its child site window rather than the top-level HWND, so
  `native_window` installs a `WH_MOUSE_LL` hook on the UI thread. It only consumes Shift+wheel when
  the cursor is over the pager.
- Keep a drag gesture alive through reactor pointer-capture loss. The release still carries the final
  coordinates, so losing capture must not turn the drop into a desktop-click switch.
- Right-click opens the native context menu with Toggle View Mode, Position, Settings, About, and
  Exit, using the C# glyph labels (`↔`, `⌖`, `⚙`, `ℹ`, `✕`) as leading menu text.
- Drop topmost while the modal menu is open and restore it before returning, so the pager cannot
  cover its own popup.

### 5.5 Refresh and Identity

- Use the tick pattern from 2.5.
- Recompute the snapshot and layout only when the primary tick fires.
- Use the change-detection hash to skip layout and icon work; reactor's reconciler skips unchanged
  native properties, so the declarative view can still be rebuilt cheaply.
- Use keys for desktop and window collections so reactor can preserve element identity.

### 5.6 Icons

The shared icon cache returns a borrowed `HICON`. Reactor `Image` needs image data:

- `pager/icons.rs` encodes each icon to a PNG with WIC, including the mask-derived alpha fallback for
  older icons. It first copies the icon to the exact display pixel size with
  `CopyImage`/`LR_COPYFROMRESOURCE`, so WinUI never downscales a large icon frame.
- The encoded `EncodedImage` is cached by `(hwnd, hicon)`, cleared on DPI changes, and pruned when the
  window disappears.
- Do not decode icons per frame or per view publication.

### 5.7 Opacity and Topmost

- Apply `BackgroundOpacity` through `WS_EX_LAYERED` and
  `SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA)` in `native_window`.
- Do not simulate opacity by pre-multiplying every color; it breaks icons and text.
- Reapply `HWND_TOPMOST` on the secondary tick. WinUI may reset window styles after style changes.
- The covered-window watchdog remains: `WindowFromPoint`, `ShowWindowAsync(SW_SHOWNOACTIVATE)`, then
  reapply topmost.

### 5.8 DPI and Multi-Monitor

- Reactor and WinUI use DIPs; the pager's own layout still snaps to whole device pixels.
- The manifest declares `PerMonitorV2`, as WinUI 3 requires. The pager computes its pixel grid from
  the pager window DPI and rebuilds the layout and icon cache when that DPI changes, so moving the
  pager to a monitor with another scale keeps its intended DIP size.
- WinUI handles per-monitor DPI. This is an intentional improvement over the C# system-DPI behavior.
- Keep monitor enumeration from the core crate. Monitor rectangles are always physical pixels; only
  the window DPI scale changes.
- Keep the C# limitation for vertically arranged monitors until the layout model is redesigned.

## 6. Dialogs

### 6.1 About

The About dialog is a reactor secondary window:

- 600 x 480 DIP client area.
- Logo image from the embedded `logo.png`.
- Version, author, acknowledgments, and a Close button.
- Title "About Micro Task Pager".
- Light theme, white background, centered content.

`examples/reactor_about.rs` is the reference runner. The component lives in `src/dialogs/about.rs`
and is opened by the pager as a secondary window.

### 6.2 Settings

The Settings window is a reactor secondary window:

- Sections corresponding to the C# tabs: Layout, Theme, Misc, shown through a `Pivot`.
- A working copy of `Settings`; edits are previewed live but only OK persists. The pure
  `SettingsDraft` from `micro-task-pager-core` keeps the opening values for per-option revert and the
  clamp rules.
- OK and Cancel plus reset-per-section. OK persists and closes; Cancel restores the opening values
  without persisting, and closing the window behaves like Cancel.
- Per-option revert where the C# dialog supports it. The revert target is always the value the dialog
  opened with.
- Live preview for safe changes (opacity, colors, height, view mode) by publishing a preview settings
  object to the pager component. Edits are coalesced at 80 ms so a held NumberBox arrow or slider drag
  does not rebuild the pager layout for every increment.
- Numeric clamps and validation before writing, implemented in the shared draft.
- Registry write through the shared `settings` model; keep C#-compatible value names and types. OK
  also writes the live per-desktop view modes so a later settings save cannot discard a Shift+wheel
  toggle.
- Placement uses the shared `find_placement` sequence (right, left, below, above, center) in physical
  pixels. The pager retries every millisecond, cloaks the HWND through DWM, moves it, and reveals it,
  so the default position is not presented.
- Rows use standard WinUI controls: `ComboBox`, `NumberBox`, `Slider`, `CheckBox`, `Button`, and a
  `ContentDialog` hosting a `ColorPicker`. Sections are cards rather than recreated WinForms group
  boxes.

The component is split into `src/dialogs/settings/mod.rs` (state and update loop) and `view.rs`
(declarative view). The pure working-copy model and placement algorithm are shared with
`rust-winsafe` in `crates/micro-task-pager-core/src/dialogs/`.

## 7. Native Windows Integration

### 7.1 Crates and Features

- `windows = "0.62"` for the Win32 surface that reactor does not wrap: window styles, layered alpha,
  DWM cloaking, the shell context menu, the low-level mouse hook, WIC icon conversion, and GDI-free
  icon copying.
- `windows-core` for COM traits and error types.
- `windows-reactor = "0.100"` for all UI.
- `micro-task-pager-core` for the shared platform layer and the focused `windows-registry` /
  `windows-version` crates it uses.

Enable only the `windows` features that are used. The UI-related features are limited to the platform
modules plus the small `native_window` surface: COM, shell, DWM, GDI/imaging (icon conversion),
threading, library loader, accessibility, and file system.

### 7.2 Native Surface

Native Win32 code in this crate is limited to:

- `native_window.rs` – pager window styles, topmost, opacity, position, HWND lookup, context menu,
  wheel hook, drag cursor, and DWM cloaking.
- `main.rs` – process bootstrap and COM apartment creation through the core crate.
- `pager/icons.rs` – WIC and `CopyImage` conversion of a borrowed core `HICON`.

UI components must not contain raw handles, message constants, or `unsafe` blocks. Virtual-desktop
COM, window enumeration, monitors, and foreground tracking are in the core crate and are called on
the same UI thread.

### 7.3 WinUI Bootstrap and Packaging

- `windows-reactor` bootstraps the Windows App Runtime 2.x framework package.
- The minimum OS is Windows 10 1809 (build 17763), the minimum supported by the runtime.
- Keep the application manifest `asInvoker`. It declares `PerMonitorV2`; WinUI 3 requires per-monitor
  DPI awareness, and a manifest default would make reactor's `SetProcessDpiAwarenessContext` call
  fail.
- Release packaging is framework-dependent: ship the executable and require Windows App Runtime 2.4 or
  later. Reactor resolves the installed framework package and prompts when it is missing.

## 8. Known Deviations from C#

These deviations are specific to the WinUI front end. The shared deviations common to both Rust front
ends are listed in `../crates/micro-task-pager-core/ARCHITECTURE.md` §11.1.

- Replace the native WinForms UI with WinUI 3; the look and control behavior differ by design.
- Use WinUI DIPs and per-monitor DPI instead of the C# system-DPI scaling.
- Require Windows 10 1809 (17763) or later because of the Windows App Runtime.
- Ship with a Windows App Runtime dependency (framework package).
- No COM during painting; the view is built from the cached snapshot.
- Replace OLE drag-and-drop with an internal pointer-capture drag: the payload is only a window handle
  and a source desktop, so `IDropSource`/`IDropTarget` registration is unnecessary. The drag ghost is
  a reactor element. Keep the gesture alive through pointer-capture loss; reactor's release event
  still carries the final coordinates.
- Use a low-level mouse hook for Shift+wheel because WinUI consumes wheel input in its child site
  window. The hook only acts when Shift is held and the cursor is over the pager.
- Encode icons at the exact display pixel size with `CopyImage` before PNG conversion, so WinUI does
  not blur them by downscaling a larger frame.
- Drag cursor feedback is best-effort: `SetCursor` and `WM_SETCURSOR` are reset by WinUI. Revisit when
  reactor exposes a cursor property; the functional drag/drop path does not depend on it.
- Keep global window opacity through the layered-window shim; do not approximate it by pre-multiplying
  colors.
- Use a native `TrackPopupMenu` in `native_window` until reactor exposes transient menus; drop
  topmost while the modal menu is open and restore it afterward.
- About and Settings are independent secondary windows owned by the pager, not modal `ShowDialog`
  calls. The pager prevents a second dialog while one is open, and the context menu grays both
  entries.
- Settings uses OK/Cancel with transient live preview; only OK persists. The C# dialog uses
  Save/Cancel, so there is no extra Apply step.
- Color editing uses a reactor `ContentDialog` with a `ColorPicker` instead of the WinForms
  `ColorDialog`. Alpha is not editable, matching the C# color dialog's result.
- While a dialog is open, the pager skips reasserting its own topmost and reasserts the dialog's
  topmost by window title. Closing the dialog restores the pager topmost. A title/watchdog check
  clears the dialog flag if the close notification is lost.
- Numeric rows have explicit minus/plus buttons because `windows-reactor` 0.100 does not expose
  `NumberBox.SpinButtonPlacementMode`. The buttons send a one-step edit to the working copy and are
  disabled at the clamp limits.

## 9. Migration Strategy

The native Win32 v1 completed Phases 1-6 and was removed at the Phase 11 cutover. The current
application is the reactor pager. Phase status and acceptance evidence are tracked in
[`MIGRATION.md`](MIGRATION.md); this section only records the shape of the migration.

- Phases 1-6 built and verified the native v1 baseline (COM, window model, GDI renderer, always-on
  behavior, packaging).
- Phase 7 adopted WinUI 3 via `windows-reactor` and updated the architecture and agent rules.
- Phase 8 created the reactor shell and `native_window` shim.
- Phase 9 ported the pager content to DIPs, icons, pointer input, and the context menu.
- Phase 10 promoted About and built the reactor Settings window.
- Phase 11 made the reactor pager the main binary and deleted the native UI.
- Phase 12 verifies behavior and packages the framework-dependent release.

Do not reintroduce the native UI. Keep `examples/reactor_about.rs` as a regression sample. Keep the
C# implementation until parity is proven; remove it only on explicit instruction.

## 10. Testing and Verification

- Unit-test pure logic: DIP layout, hit testing, Micro Taskbar grid/overflow, pointer state
  transitions, dialog lifecycle, icon conversion, and the shared core models.
- Use reactor's component test facilities where they fit for view/message tests. Do not build a
  custom UI test framework.
- Keep the shared `probe` example for COM and version-matrix verification.
- Add screenshot comparison or at least deterministic layout tests for both view modes.
- Add manual UI smoke tests for borderless/topmost/opacity/pinning after style changes.
- Verify no COM or window enumeration happens on a background thread.
- Run `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and
  `cargo test --workspace` before declaring work complete.

## 11. Deployment and Packaging

- x64 MSVC build; no .NET runtime and no installer.
- `#![windows_subsystem = "windows"]` so no console window appears.
- Statically link the MSVC runtime (`.cargo/config.toml` at the workspace root) so the executable does
  not require the Visual C++ redistributable.
- Windows App Runtime 2.4 or later is required. Packaging is framework-dependent: reactor resolves the
  installed framework package and shows an install prompt when it is missing. The executable does not
  bundle the runtime.
- Minimum OS: Windows 10 1809 (build 17763).
- Embed the icon and version resource.
- Keep the application manifest `asInvoker` with `PerMonitorV2` DPI awareness.
- Keep the C# mutex name (`micro-task-pager-win-app`) for release builds so the implementations cannot
  run simultaneously and fight over topmost/pinning. Use a different name for development builds if
  side-by-side comparison is needed.
- Log to `%LOCALAPPDATA%\MicroTaskPager\logs` through the core crate.
- The release artifact is `target/release/micro-task-pager-winui.exe`. A distribution folder may
  rename it to `MicroTaskPager.exe`; the embedded icon/version/manifest stay intact.

## 12. Rust Development Guidelines

1. Prefer safe Rust APIs over direct FFI.
2. Keep `unsafe` blocks small and document their safety requirements.
3. Encapsulate all undocumented COM interfaces behind the core `VirtualDesktopApi` trait.
4. Avoid leaking raw Windows handles across application layers.
5. Use RAII wrappers for COM and other Windows resources.
6. Handle `HRESULT` errors explicitly; no silent fallbacks.
7. Respect COM apartment and thread-affinity requirements; use one STA thread.
8. Optimize for readability first. Compact, declarative code beats clever abstraction.
9. One screen equals one component; `view` is the only layout code.
10. Do not build a UI framework or widget toolkit on Win32. Use `windows-reactor`.
11. Do not add Win32 UI code to components. Platform behavior lives in `native_window` or the core
    crate.
12. Keep layout and hit testing pure and testable.
13. Use DIPs in UI code; do not reintroduce manual DPI scaling.
14. Keep the settings model independent of the UI.
15. Do not assume Windows implementation details remain stable across updates.
16. No panics across FFI boundaries.
17. Put shared platform behavior in `micro-task-pager-core`.

`winsafe` is not a dependency here. It would not remove the `unsafe` needed for the undocumented
virtual-desktop COM layer, and it would compete with reactor. Keep `unsafe` confined to
`native_window`, the WinUI icon conversion, and the COM bootstrap in `main.rs`, and give every block a
safety comment. Components, layout, hit testing, dialog state, settings, and the core supervisor stay
safe Rust.

## 13. Important Architectural Decisions

| Decision | Direction |
| --- | --- |
| Repository layout | Separate `rust-winui/` directory in a Cargo workspace |
| Shared platform | `micro-task-pager-core` |
| UI framework | WinUI 3 through `windows-reactor` |
| Main window | Reactor root component + native window shim |
| Rendering | WinUI composition |
| Window previews | Scaled geometries + shell icons |
| Live thumbnails | Not planned |
| Context menu | Native `TrackPopupMenu` in `native_window` (temporary) |
| Settings UI | Reactor secondary window |
| About | Reactor secondary window |
| Native Win32 | Platform glue only |
| Virtual desktops | Core: one safe API + Win10/Win11 layout modules |
| Threading | Reactor UI thread, single STA for COM |
| Application icons | Shell APIs converted to reactor image sources |
| Application state | Pure Rust |
| Runtime | Windows App Runtime 2.x |
| Minimum OS | Windows 10 1809 (17763) |
| .NET dependency | Removed in the final implementation |
| C# implementation | Latest features; behavioral reference |
| Rust winsafe | Stable implementation; fewer features, updates delayed |
| Rust WinUI | Experimental; feature set frozen, not actively updated |

## 14. Instructions for AI Coding Assistants

When assisting with this project:

- Read this document, the shared
  [`core architecture`](../crates/micro-task-pager-core/ARCHITECTURE.md), `MIGRATION.md`, and
  `AGENTS.md` before proposing structural changes.
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
- Keep version-specific Windows compatibility code isolated in the core crate.
- Do not add DWM thumbnails or a custom Direct2D/GDI renderer.
- Put shared platform behavior in `micro-task-pager-core`, not in this front end.
- Avoid unnecessary architectural complexity.
- Prefer small, independently testable implementation steps.
- Preserve existing application behavior unless a deviation is listed in section 8 or explicitly
  requested.
- When porting existing C# functionality, use the existing implementation as the behavioral reference.

The primary objective is a reliable, maintainable, compact Windows application implemented in Rust,
with a modern WinUI 3 UI and no .NET runtime dependency.
