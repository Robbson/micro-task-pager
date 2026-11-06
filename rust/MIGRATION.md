# Rust Migration – Progress and Phase Log

This is the living execution document for the Rust migration. `ARCHITECTURE.md` is the contract and
the source of truth for design; this file tracks progress, verification evidence, and decisions that
are not already architectural.

Keep this document focused on progress and evidence. Update it when a phase starts or completes, when
a Windows build is verified, or when an open question is resolved. Do not duplicate architecture
content; link to the relevant section instead.

## Phase status

| Phase | Status | Started | Completed | Notes |
|---|---|---|---|---|
| 1. Scaffold | Complete | 2026-10-30 | 2026-10-30 | Core scaffold done; remaining items re-scoped to Phase 2 and Phase 4 |
| 2. Virtual desktop subsystem | Complete | 2026-10-30 | 2026-10-30 | Win10/Win11 layouts, supervisor, logging, and probe implemented; older-build verification postponed |
| 3. Window model | Complete | 2026-10-30 | 2026-10-30 | Enumeration, filters, icons, z-order, failure handling, and snapshot hash implemented |
| 4. Main window and rendering | Complete | 2026-10-30 | 2026-10-30 | Native v1 Win32 window, GDI renderer, interactions, manifest, icon, and version resource |
| 5. Always-on behavior | Complete | 2026-10-30 | 2026-10-30 | Pinning, topmost recovery, foreground hook, and last-active restore implemented |
| 6. Parity and deployment | Complete | 2026-10-30 | 2026-10-30 | Native v1 release verified on 26200, single monitor; C# retained |
| 7. WinUI architecture and docs | Complete | 2026-10-30 | 2026-10-30 | Adopted WinUI 3 via `windows-reactor`; architecture, agent rules, and notices updated |
| 8. Reactor pager shell | Complete | 2026-11-06 | 2026-11-06 | Borderless/tool-window/layered/topmost reactor window, HWND shim, pinning, and ticks verified on 26200; native v1 remains the main binary until Phase 11 |
| 9. Reactor pager content | Complete | 2026-11-06 | 2026-11-06 | DIP layout/hit testing, Preview and Micro views, crisp display-sized icons, pointer drag/click, low-level Shift+wheel hook, and compact native context menu verified on 26200; drag cursor feedback is a follow-up, Settings/About wait for Phase 10 |
| 10. Reactor dialogs | Pending | | | About and Settings |
| 11. Cutover and cleanup | Pending | | | Reactor main binary; remove native UI and GDI |
| 12. Verification and packaging | Pending | | | Checklist, OS floor, runtime packaging, supported builds |

The native v1 phases 1-6 are a completed baseline, not the target UI architecture. They remain
runnable and are the behavioral reference until the WinUI cutover.

## Native v1 Baseline (Phases 1-6)

### Phase 1: Scaffold

- [x] Cargo crate in `rust/` with toolchain channel and `rustfmt`/`clippy`.
- [x] `rust/.gitignore` for `target/`.
- [x] Single-instance mutex with the release mutex name.
- [x] COM STA initialization guard.
- [x] Settings model, defaults, and tolerant registry import.
- [x] `cargo fmt`, `cargo clippy -D warnings`, `cargo build`, and `cargo test` pass.

### Phase 2: Virtual Desktop Subsystem

- [x] Safe `VirtualDesktopApi` trait and error model.
- [x] COM apartment assumption documented; the probe initializes STA.
- [x] Win11 layout backend for build 22000 and later.
- [x] Win10 layout backend for build 10240 through 21999 (Win10 and Win10LTSC).
- [x] Probe read-only: count, current index, desktop GUIDs, and window-to-desktop index.
- [x] Probe pin state through `IApplicationViewCollection`, `IApplicationView`, and
       `IVirtualDesktopPinnedApps`.
- [x] Probe `switch` verified in both directions.
- [x] Probe `move`, `pin`, and `unpin` verified on the probe's own throwaway window.
- [x] Logging to `%LOCALAPPDATA%\MicroTaskPager\logs` with 14-day retention.
- [x] Supervisor that recreates the COM objects after a restartable failure.

Verification on Windows 10, Win10LTSC, and Windows 11 21H2/22H2 is postponed; see the Postponed
section.

### Phase 3: Window Model

- [x] Immutable `WindowInfo` / `WindowSnapshot` model, sorted by window handle.
- [x] `EnumWindows` filtering: shell window, invisible windows, empty titles, class blacklist, own
       process, blacklisted handles.
- [x] Desktop index per window with failure counters and a blacklist.
- [x] Z-order, foreground/active state, title, class, process ID, and rectangle.
- [x] Icon fallback chain (`WM_GETICON`, class icon, `IDI_APPLICATION`) plus a per-window cache.
- [x] Snapshot hash for change detection.
- [x] Probe `snapshot` command verified on build 26200.

### Phase 4: Native Main Window and Rendering

This phase describes the native v1. The target UI replaces the window procedure and GDI renderer with
reactor components.

- [x] Win32 popup window, message loop, primary/secondary timers, and layered-window opacity.
- [x] Monitor enumeration and per-desktop/screen layout, ordered left-to-right then top-to-bottom.
- [x] GDI double-buffered renderer for Preview and Micro Taskbar, including icons, borders, separators,
       active highlighting, and `+N` overflow.
- [x] Hit testing, click to switch/focus, internal drag to move a window, Shift+wheel mode toggle.
- [x] Context menu: Toggle View Mode, Position (Restore/Save/Default), About, Exit.
- [x] Application manifest with `asInvoker` and system DPI awareness.
- [x] Application icon and version resource.
- [x] Replaced the temporary console entry point with the Win32 message loop.
- [x] Reshaped the flat `RendererViewModes` registry value into the per-desktop/screen model.

### Phase 5: Always-On Behavior

- [x] Stable Application User Model ID and pinning to all desktops when `ShowAppInTaskbar` is enabled.
- [x] Retry pinning on the secondary timer until it succeeds; tolerate shell failures.
- [x] Covered-window watchdog using `WindowFromPoint`, `ShowWindowAsync`, and topmost reassertion.
- [x] `EVENT_SYSTEM_FOREGROUND` hook with an atomic handoff to the UI thread.
- [x] Remember the last active window per desktop and restore it after a pager switch, with a 150 ms
       retry for the switch animation.
- [x] Respect `ShowAppInTaskbar` when choosing `WS_EX_APPWINDOW`.

### Phase 6: Parity and Deployment

- [x] Walked the behavioral checklist in `ARCHITECTURE.md` §11 on build 26200, single monitor.
- [x] Release build is x64 Windows GUI, uses a static MSVC runtime, and has no .NET or VC++
       redistributable dependency.
- [x] Embedded icon, version resource, and system-DPI manifest verified.
- [x] Logs use `%LOCALAPPDATA%\MicroTaskPager\logs`; 14-day purge verified with an old file.
- [x] Clean shutdown through `WM_CLOSE`; no lingering process and no startup error in the log.
- [x] Deployment from a clean directory works.
- [x] Pinning and cross-desktop visibility verified; covered-window recovery and last-active restore
       are implemented and exercised manually.
- [x] About is a native modal dialog matching the C# layout: logo, version, author, acknowledgments,
       centered Close button, themed controls, and system-DPI scaling. Superseded by the WinUI About
       window in Phase 10.
- [ ] Older Windows builds and multi-monitor layouts (postponed).
- [ ] Remove the C# implementation (only after explicit user instruction and older-build parity).

## WinUI 3 Migration (Phases 7-12)

### Phase 7: WinUI Architecture and Documentation

- [x] Adopt WinUI 3 through `windows-reactor` as the UI architecture.
- [x] Record the window-shim and layered-alpha probe results.
- [x] Update `ARCHITECTURE.md` to the target UI, pager, dialog, settings, and packaging design.
- [x] Update `AGENTS.md` so future work does not extend the native UI.
- [x] Update dependency notices for `windows-reactor` and the Windows App Runtime.
- [x] Supersede the earlier "no GUI framework", "no settings UI", and "native About" decisions.
- [x] Keep the native v1 runnable; no application behavior changes in this phase.

### Phase 8: Reactor Pager Shell

- [x] Add `windows-reactor` as a normal dependency.
- [x] Create the `pager/` component and `native_window.rs`.
- [x] Start the reactor root window and apply borderless, tool-window, layered alpha, topmost, and
      saved position.
- [x] Find the pager HWND and pin to all desktops; reapply topmost on the secondary tick.
- [x] Add the self-rescheduling tick and refresh the window snapshot on the UI thread.
- [x] Acceptance: pinned, topmost, opacity works, no title bar, clean shutdown, no COM off the UI
      thread.

The reactor shell is runnable as `cargo run --example reactor_pager`; `src/main.rs` stays on the
native v1 until the Phase 11 cutover.

### Phase 9: Reactor Pager Content

- [x] Port layout and hit testing to DIPs; keep them pure and unit-tested.
- [x] Preview view with desktop/window rectangles, icons, borders, and active highlighting.
- [x] Micro Taskbar view with icon grid, wrapping, and `+N` overflow.
- [x] Click-to-switch/focus, pager drag, window drag, Shift+wheel, and native context menu
      (temporary until reactor exposes one).
- [x] Convert cached `HICON`s to reactor image sources; keep keyed lists for identity.
- [x] Acceptance: pager UI checklist in `ARCHITECTURE.md` §11.

The context menu is a native `TrackPopupMenu` in `native_window` because `windows-reactor` 0.100
`Menu` is a Button/SplitButton flyout, not a right-click context menu. This is a documented temporary
deviation; topmost is dropped while the menu is open and restored afterward. Settings and About are
present but grayed until Phase 10.

Follow-up fixes after user testing:

- WinUI delivers wheel input to its child site window, so Shift+wheel now uses a `WH_MOUSE_LL` hook on
  the UI thread instead of a top-level `WM_MOUSEWHEEL` subclass.
- Drag/drop keeps the gesture alive through reactor pointer-capture loss; a drop on another desktop
  moves the window and no longer switches the current desktop on mouse-up.
- Icons are copied to the exact display pixel size with `CopyImage` before PNG conversion, removing
  the WinUI downscale blur at 100% DPI.
- Layout geometry is computed on whole device pixels (desktop widths truncate like the C# renderer)
  and icons are placed in their own pixel-aligned canvas slots. XAML no longer antialiases 1 px
  borders across two pixels or resamples icons that sat on fractional DIP coordinates.
- Desktop switching and window focus require a release that follows a left press. A right press keeps
  suppressing releases while the native context menu runs, so choosing a menu item no longer switches
  to the desktop under the cursor.
- The native menu uses `MNS_NOCHECK` and the C# glyphs as leading labels, reducing its width.
- Drag cursor feedback is deferred: WinUI resets `SetCursor`/`WM_SETCURSOR`. The functional drag path
  does not depend on it.

### Phase 10: Reactor Dialogs

- [ ] Promote `examples/reactor_about.rs` to `src/dialogs/about.rs`.
- [ ] Build `src/dialogs/settings.rs` with Layout/Theme/Misc sections.
- [ ] Implement working copy, Apply/OK/Cancel, per-option revert, reset per section, validation, and
       live preview.
- [ ] Delete the native `src/about.rs`; the native v1 never had a Settings dialog.
- [ ] Acceptance: About and Settings match the C# behavior and look modern.

### Phase 11: Cutover and Cleanup

- [ ] Make the reactor pager the main binary.
- [ ] Delete `window.rs`, `render/`, native `about.rs`, and unused `windows` UI features.
- [ ] Remove GDI/Painter references from docs and tests.
- [ ] Keep the C# implementation until parity is proven.

### Phase 12: Verification and Packaging

- [ ] Walk the full checklist against the C# build.
- [ ] Verify on supported Windows builds and mixed-DPI monitors.
- [ ] Decide framework-dependent vs self-contained Windows App Runtime packaging.
- [ ] Update user-facing requirements for Windows 10 1809 (17763) and the runtime.
- [ ] Check shutdown, handle/COM leaks, log output, and release size.
- [ ] Remove the C# implementation only on explicit instruction.

## Postponed

These items are not scheduled and do not block the next phase.

| Item | Reason | Revisit when |
|---|---|---|
| Run the probe on Windows 10 and Win10LTSC | No older images or virtualization available | A Windows 10 image or VM is available |
| Run the probe on Windows 11 21H2 and 22H2 | Same | A suitable image is available |
| Validate pinning with `ShowInTaskbar` variants | Needs manual UI testing alongside the pager window | Phase 12 |
| Verify WinUI topmost across explorer restarts and shell restarts | Requires manual shell testing | Phase 12 |
| Choose framework-dependent vs self-contained runtime packaging | Product/packaging decision | Phase 12 |

## Decision log

| Date | Decision | Rationale | Reference |
|---|---|---|---|
| 2026-10-30 | GDI renderer first, Direct2D deferred | Layered-window alpha and crate maturity outweighed GPU benefits for the native v1 | Superseded by WinUI 3 |
| 2026-10-30 | No settings UI in v1 | Kept the native port small and avoided a GUI framework | Superseded by WinUI 3 |
| 2026-10-30 | One safe virtual-desktop API plus per-build vtable modules | The C# variants duplicate logic; only the layouts differ | ARCHITECTURE §7.1 |
| 2026-10-30 | Use `windows` 0.62 and the 0.100 native-app crates where applicable | The umbrella `windows` crate has not moved to 0.100 | ARCHITECTURE §6.1 |
| 2026-10-30 | Use the `stable` toolchain channel for now | The installed `1.99.1` rustup toolchain is missing `cargo.exe`; stable is also 1.99.1 | Open question below |
| 2026-10-30 | Implement the Win11_ backend and probe first | The current machine runs build 26200; validate the vtable strategy before porting older variants | Phase 2 |
| 2026-10-30 | Collapse the four C# variants into two Rust layouts | The methods the app uses have identical GUIDs and vtable prefixes within the Win10/Win10LTSC pair and the Win11/Win11_ pair | ARCHITECTURE §7.3 |
| 2026-10-30 | Use a small in-house file logger | Avoids a dependency and matches the existing log directory and 14-day retention | ARCHITECTURE §4 |
| 2026-10-30 | Close Phase 1 with re-scoped items instead of finishing it first | Manifest, icon, message loop, and view-mode reshape depend on Phase 4; only logging was pulled into Phase 2 | Phase 1 |
| 2026-10-30 | Postpone older-build verification | No Windows 10 or older Windows 11 images are available; code is in place and the matrix can be filled later | Postponed section |
| 2026-10-30 | Derive z-order from `EnumWindows` order | Avoids the C# O(n²) `GetWindow` walk; relative order is equivalent | ARCHITECTURE §12 |
| 2026-10-30 | Prune desktop-lookup failure counters using every window that reaches the lookup | The C# code prunes against successful windows only, so a failing window loses its counter each snapshot and never reaches the blacklist | ARCHITECTURE §12 |
| 2026-10-30 | Use GDI `DrawIconEx` instead of converting every `HICON` to a bitmap | Fewer GDI objects and no manual icon-bitmap lifetime; the pager redraws only on changes | Superseded by reactor `Image` sources |
| 2026-10-30 | Pre-blend the separator color | `FillRect` has no alpha, so the GDI color is the result of blending C#'s translucent light blue over the default background | Superseded by WinUI colors |
| 2026-10-30 | Use the common-controls image-list drag image for window drags | Matches the usual Windows ghost icon and avoids implementing OLE drag source/target interfaces | Superseded by a reactor drag ghost |
| 2026-10-30 | Pin after the window is visible and retry on the secondary timer | Pinning needs an application view; the shell can reject an early attempt | ARCHITECTURE §2.5, §8.7 |
| 2026-10-30 | Let the foreground hook store only the handle | The callback has no access to the desktop API; mapping on the next timer tick keeps COM on the UI thread | ARCHITECTURE §3.7 |
| 2026-10-30 | Use the pager center for the covered-window check | Matches the C# watchdog and is cheap enough for the secondary tick | ARCHITECTURE §8.7 |
| 2026-10-30 | Statically link the MSVC runtime | The release executable must run without the Visual C++ redistributable | ARCHITECTURE §15 |
| 2026-10-30 | Keep the C# implementation for now | Older Windows builds and multi-monitor parity are still postponed; removal needs explicit instruction | ARCHITECTURE §17 |
| 2026-10-30 | Use screen cursor coordinates while dragging the pager | Client coordinates change as the window moves, which caused a feedback loop and flicker | ARCHITECTURE §8.4 |
| 2026-10-30 | Drop topmost and suspend its timer reassertion while the context menu is open | A topmost pager otherwise covers its own popup menu | ARCHITECTURE §8.4 |
| 2026-10-30 | Keep unsafe confined to FFI modules instead of adopting `winsafe` | `winsafe` is a UI wrapper, not a safe abstraction for undocumented shell COM; the official `windows` crate is the right binding | ARCHITECTURE §16 |
| 2026-10-30 | Implement About as a native modal Win32 dialog instead of a message box | Matched the C# layout and kept the native release dependency-free; the logo is an embedded PNG decoded with WIC | Superseded by WinUI 3 |
| 2026-10-30 | Keep `windows-reactor` behind an example-only dev-dependency | WinUI 3 needs the Windows App SDK runtime and owns its own application lifetime, so it seemed unsuitable inside the Win32 loop | Superseded by the architecture pivot |
| 2026-10-30 | **Adopt WinUI 3 via `windows-reactor` as the UI architecture** | The native About dialog is about 650 lines versus about 85 for the reactor version. Compact, readable, maintainable code and a modern look outweigh the Windows App SDK dependency. Secondary windows solve the earlier modal-lifecycle concern | ARCHITECTURE §3, §17 |
| 2026-10-30 | Make `windows-reactor` a normal dependency in Phase 8 | The UI, About, and Settings use it; it is no longer an experiment | Phase 8 |
| 2026-10-30 | Keep a small `native_window` shim for HWND/pinning/topmost/opacity | Reactor 0.100 has no public HWND/AppWindow accessor; the pager needs those window properties | ARCHITECTURE §3.6, §6.2 |
| 2026-10-30 | Preserve opacity with `WS_EX_LAYERED` and `SetLayeredWindowAttributes` on the WinUI window | A live probe confirmed the alpha works and the desktop shows through; pre-multiplying colors would be wrong | ARCHITECTURE §8.7 |
| 2026-10-30 | Reapply topmost on the tick instead of once | The live probe showed WinUI reset topmost after a style change | ARCHITECTURE §3.6 |
| 2026-10-30 | Use a self-rescheduling `spawn_background` tick instead of `WM_TIMER` | Reactor has no timer API; the background task only sleeps and returns a message, so COM stays on the UI thread | ARCHITECTURE §3.5 |
| 2026-10-30 | Raise the application minimum OS to Windows 10 1809 (17763) | Windows App Runtime 2.x requires 17763; the virtual-desktop backend still supports 10240+, but the WinUI app cannot | ARCHITECTURE §7.3, §15 |
| 2026-10-30 | Keep the native v1 and C# implementation until the WinUI cutover | The migration needs a behavioral reference and a working fallback | ARCHITECTURE §13 |
| 2026-11-06 | Keep the native v1 as the main binary through Phase 8 | The reactor shell is runnable as an example, but the pager content and dialogs do not exist yet; cutting over now would remove working behavior | Phase 11 |
| 2026-11-06 | Find the pager HWND by process ID and window title | Reactor 0.100 exposes no public HWND accessor, and the component declares a stable title before the native lookup runs | ARCHITECTURE §3.6 |
| 2026-11-06 | Reapply the DIP client size after removing the WinUI frame | Reactor's `ResizeClient` runs before the shim removes the caption, so the first outer size still includes the old non-client area | ARCHITECTURE §3.6 |
| 2026-11-06 | Retry the HWND lookup every 50 ms until the window appears | The window and title are created after the component's first publication; a full primary-tick delay produced a visible title-bar flash | Phase 8 |
| 2026-11-06 | Encode cached `HICON`s to PNG with WIC and cache by `(hwnd, hicon)` | Reactor `Image` needs encoded data; encoding once per icon change keeps the 200 ms refresh cheap | ARCHITECTURE §8.6 |
| 2026-11-06 | Use a low-level mouse hook for Shift+wheel | WinUI delivers wheel input to its child site window, so a top-level `WM_MOUSEWHEEL` subclass never sees it. `WH_MOUSE_LL` runs on the UI thread and only consumes Shift+wheel over the pager | ARCHITECTURE §8.4, §12 |
| 2026-11-06 | Encode icons at the display pixel size with `CopyImage` | Drawing a 32/48 px shell icon down to 16 DIP blurred the pager at 100% scale; copying to the target size first keeps icons crisp | ARCHITECTURE §8.6, §12 |
| 2026-11-06 | Keep drag gestures alive through pointer-capture loss | Reactor can lose capture while rebuilding the view after a pointer move; cancelling the drag turned the drop into a desktop click | ARCHITECTURE §8.4, §12 |
| 2026-11-06 | Defer drag cursor feedback | WinUI resets `SetCursor`/class cursor/`WM_SETCURSOR`; a reactor cursor API or element access is needed. The drag itself works without it | ARCHITECTURE §12 |
| 2026-11-06 | Use a native `TrackPopupMenu` for the Phase 9 context menu | The secondary reactor dialog looked poor; `windows-reactor` 0.100 has no right-click context menu. The native menu is a documented temporary shim and topmost is dropped while it is open. Revisit with a later reactor transient-menu API | ARCHITECTURE §8.4, §12 |

## Verified Windows builds

Record every build on which the probe or a UI spike runs. Do not infer results from a different
edition.

| Build | Edition | Selected variant | Result | Date | Notes |
|---|---|---|---|---|---|
| 26200 | Windows 11 25H2 | Win11 layout | Count 2, current index, desktop GUIDs, window-to-desktop index, switch 0 -> 1 -> 0, move 0 -> 1 -> 0, pin state, pin, and unpin work | 2026-10-30 | Win10 layout and 21H2/22H2 are untested (postponed) |
| 26200 | Windows 11 25H2 | Win11 layout | Reactor About rendered; borderless, `WS_EX_TOOLWINDOW`, layered alpha, and topmost reassertion work on a live WinUI window | 2026-10-30 | Topmost was reset once by WinUI after the style change; reasserting worked. Pinning through the HWND shim is not yet tested |
| 26200 | Windows 11 25H2 | Win11 layout | Reactor pager shell: `WS_POPUP` without caption, `WS_EX_APPWINDOW`, `WS_EX_LAYERED`, topmost, saved position, pinned (`probe pinned` = true), alpha 191 at `0.75`, and `WM_CLOSE` exited cleanly | 2026-11-06 | Single monitor, 100% DPI; shell run from `examples/reactor_pager` while the native v1 remains the main binary |
| 26200 | Windows 11 25H2 | Win11 layout | Reactor pager content: mixed Micro/Preview desktops with crisp display-sized icons, borders, active highlighting, and `+N` overflow; compact native `TrackPopupMenu` with glyph labels and Toggle/Position/grayed Settings/About/Exit; real Shift+wheel toggled the mode under the cursor; dragging a probe window from desktop 0 to 1 moved it while the current desktop stayed 0 | 2026-11-06 | Single monitor, 100% DPI; drag cursor feedback is deferred, click-to-switch/focus remains for the Phase 12 live pass |

## Open questions

| Question | Why it matters | Owner | Status |
|---|---|---|---|
| Does `IVirtualDesktopPinnedApps` pinning work on the reactor HWND found by the shim? | Always-on behavior depends on it | | Resolved 2026-11-06: `probe pinned` returned true on build 26200 |
| Does WinUI reapply styles or topmost in other situations (activation, monitor changes, shell restart)? | The pager must stay topmost and borderless | | Topmost reassert verified on the secondary tick; activation, monitor changes, and shell restart remain in Phase 12 |
| Is the converter from cached `HICON` to a reactor image source fast enough for many windows? | The pager refreshes every 200 ms | | Resolved 2026-11-06: 18 icons converted at startup and cached; refreshes reuse the encoded sources and log no failures. Icons are now copied to the display size before encoding |
| How can the pager show a move cursor during drag over WinUI content? | The C# pager uses `Cursors.SizeAll` | | Deferred 2026-11-06 by user request: `SetCursor`, class cursors, and `WM_SETCURSOR` are reset by WinUI; revisit with a reactor cursor API |
| Does a reactor `Menu` behave correctly on a layered, tool-window pager? | Right-click is the only entry point to settings/about/exit | | Resolved 2026-11-06: 0.100 `Menu` is a button flyout. Phase 9 uses a temporary native `TrackPopupMenu`; true menu support waits for a reactor upgrade |
| Should the toolchain be pinned to an exact version? | `1.99.1` is currently installed without `cargo.exe`, so `stable` (also 1.99.1) is used | | Open |
| Framework-dependent or self-contained Windows App Runtime packaging? | Affects release size and prerequisites | | Phase 12 |

## Verification log

Add short entries, one per meaningful session or phase boundary. Include the command result, not a
narrative.

| Date | Phase | Evidence |
|---|---|---|
| 2026-10-30 | 1 | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo build`, `cargo test` pass on stable 1.99.1; 4 unit tests; `cargo run` reads the existing `HKCU\SOFTWARE\MicroTaskPager` values and prints them |
| 2026-10-30 | 2 | Probe on build 26200: count 2, two desktop GUIDs, VS Code HWND maps to desktop 0, pin state read, switch 0 -> 1 -> 0; probe-owned window moved 0 -> 1 -> 0 and pinned/unpinned; fmt, clippy, and 11 tests pass |
| 2026-10-30 | 3 | Probe `snapshot` on build 26200: 16 filtered windows, desktop mapping, z-order, active window, icon present for every window, and a stable hash; fmt, clippy, docs, and 14 tests pass |
| 2026-10-30 | 4 | Pager window on build 26200 rendered both modes (screenshots), click switched 0 -> 1 -> 0, Shift+wheel/menu toggle persisted, position save/default/restore worked, system DPI awareness = 1, version resource present, fmt/clippy/docs and 17 tests pass |
| 2026-10-30 | 4 | User-test fixes: pager drag follows the screen cursor exactly (+20,+10 test), desktop 1 content renders at absolute coordinates, context menu shows Position above the pager and dismisses cleanly; 18 tests pass |
| 2026-10-30 | 4 | Window drag shows a semi-transparent image-list ghost icon with the normal arrow cursor; drag end releases the image-list lock and the pager stays responsive |
| 2026-10-30 | 5 | Pager pinned (`probe pinned` = true), visible on desktop 1 after an external switch, switched back through the pager; no hook or restore errors; 18 tests pass |
| 2026-10-30 | 6 | Release exe: x64 Windows GUI, 680 KB, static CRT (no VCRUNTIME/UCRT), embedded 0.1.0.0 and system-DPI manifest; ran from a clean temp folder, pin true, 14-day log purge worked, `WM_CLOSE` exited cleanly; 18 tests pass |
| 2026-10-30 | 6 | About dialog: native modal, embedded logo decoded through WIC, themed controls after adding the common-controls v6 manifest dependency, centered on the primary screen, and visually compared with the C# layout; `cargo test` (18), `fmt --check`, and `clippy -D warnings` pass |
| 2026-10-30 | 6 | Isolated `examples/reactor_about.rs` built and ran against Windows App Runtime 2.6.4; WinUI 3 rendered the same About content with Fluent window chrome for comparison |
| 2026-10-30 | 7 | Live reactor window probe on build 26200: `WS_POPUP`/no title bar, `WS_EX_TOOLWINDOW`, `WS_EX_LAYERED` alpha, and topmost reassertion applied to a WinUI window; screenshot shows desktop through the content; topmost was reset once and reasserted |
| 2026-10-30 | 7 | Architecture docs updated for the WinUI 3 pivot; native v1 and C# retained; no application behavior changed |
| 2026-11-06 | 8 | Reactor shell on build 26200: `WS_POPUP` with no caption, `WS_EX_APPWINDOW`, `WS_EX_LAYERED`, topmost, saved position, `probe pinned` = true, layered alpha 191 at `0.75`, and `WM_CLOSE` exited the process; `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo build --examples`, and 21 tests pass; the release native binary still imports no Windows App Runtime DLLs |
| 2026-11-06 | 9 | Reactor content on build 26200: DIP layout/hit testing, mixed Micro/Preview desktops, keyed icon cell views, display-sized `HICON`→PNG conversions, compact native context menu with the C# glyph labels (`↔`, `⌖`, `⚙`, `ℹ`, `✕`), view-mode changes persisted, topmost dropped/restored around the menu, real Shift+wheel toggled under the cursor both ways, and a probe window dragged from desktop 0 to 1 without switching the current desktop; `cargo fmt --check`, Clippy, examples build, and 31 tests pass; drag cursor feedback is deferred, click-to-switch/focus remains for the Phase 12 live pass |
| 2026-11-06 | 9 | Crisp-rendering follow-up on build 26200, 3440x1440 at 100%: the layout now works on whole pixels like the C# renderer (pager 214x45 instead of 215x45), 1 px borders no longer antialias across two pixels, and icons sit in pixel-aligned slots; Micro and Preview captures match the C# pager, including the previously soft desktop border and icon edges; `cargo fmt --check`, Clippy, and 32 tests pass |
| 2026-11-06 | 9 | Context-menu follow-up on build 26200: a right press on a non-current desktop followed by Toggle View Mode changed that desktop's mode without switching to it, while a plain left click on the same desktop still switched; switching and focus now require a release that follows a left press, and the press flags survive both the capture release and the modal menu; `cargo fmt --check`, Clippy, and 36 tests pass |

## Code map and glossary

The code is deliberately split into a few small layers. Read `src/lib.rs` first; its module
documentation mirrors this table.

### Current native v1 (transitional)

| Path | What it does |
|---|---|
| `src/main.rs` | Process entry point: logging, COM apartment, single instance, then the Win32 message loop. |
| `src/app.rs` | Application state, timers, snapshot refresh, layout rebuild, mouse and command handling. |
| `src/about.rs` | Native modal About dialog, embedded logo decoding, and DPI-aware layout. Removed at cutover. |
| `src/window.rs` | Win32 window, window procedure, timers, context menu. Removed at cutover. |
| `src/render/mod.rs` | Layout model, hit testing, drawing instructions, and `Painter` trait. Layout and hit testing survive; the Painter goes away. |
| `src/render/gdi.rs` | Double-buffered GDI backend. Removed at cutover. |
| `src/monitors.rs` | Monitor enumeration and work areas. Survives. |
| `src/com.rs` | RAII guard that initializes the COM apartment on the calling thread. Survives. |
| `src/single_instance.rs` | Named-mutex guard that prevents two pagers from running. Survives. |
| `src/settings.rs` | Registry-compatible settings model, defaults, and position/view-mode writers. Survives. |
| `src/logging.rs` | Daily file logger with 14-day retention. Survives. |
| `src/foreground.rs` | Foreground WinEvent hook and last-active-window tracking per desktop. Survives. |
| `src/desktop/mod.rs` | Public safe API (`VirtualDesktopApi`), errors, and `SupervisedDesktop`. Survives. |
| `src/desktop/com/core.rs` | Shared COM interface definitions and the generic backend. Survives. |
| `src/desktop/com/win10.rs` | Win10 layout (build 10240–21999). Survives. |
| `src/desktop/com/win11.rs` | Win11 layout (build 22000 and later). Survives. |
| `src/windows/mod.rs` | Window enumeration, filtering, snapshot model, and change hash. Survives. |
| `src/windows/icons.rs` | Icon fallback chain, per-window icon cache, and PNG encoding for reactor. |

### Target WinUI structure

| Path | What it does |
|---|---|
| `src/pager/mod.rs` | Pager component: state, messages, update, and tick scheduling. |
| `src/pager/view.rs` | Declarative `Canvas`/`Grid` view for Preview and Micro Taskbar. |
| `src/pager/layout.rs` | Pure DIP layout and hit-testing model. |
| `src/pager/input.rs` | Pointer/drag state machines and drag ghost. |
| `src/pager/icons.rs` | Encoded icon cache keyed by `(hwnd, hicon)`. |
| `src/dialogs/about.rs` | Reactor About window, promoted from the example. |
| `src/dialogs/settings.rs` | Reactor Settings window with Layout/Theme/Misc sections. |
| `src/native_window.rs` | HWND lookup, styles, topmost, layered alpha, position, wheel hook, cursor helpers, and pin handoff. |
| `app.rc`, `app.manifest`, `build.rs` | Icon, version resource, manifest, and resource embedding. |
| `.cargo/config.toml` | Static MSVC runtime. |
| `examples/probe/main.rs` | Standalone COM probe; not part of the pager binary. |
| `examples/reactor_about.rs` | Reactor About reference and regression sample. |
| `examples/reactor_pager.rs` | Reactor pager shell/content runner (Phases 8-9). |

Terms that appear in the code:

| Term | Meaning |
|---|---|
| COM apartment / STA | A threading rule for COM objects. The shell objects must be created and used on a single-threaded apartment. `Apartment` enforces the initialization step; the code must stay on that thread. |
| `VirtualDesktopApi` | The only trait the rest of the application should use. It hides COM pointers and version differences behind plain Rust methods and `Result`. |
| `SupervisedDesktop` | A small resilience wrapper. It owns one backend and forwards calls. If a call fails with a restartable COM error, it throws the backend away, creates a new one, and retries once. |
| Backend / layout | The actual COM implementation for a Windows build family. There are two layouts instead of four C# variants because the used vtables are identical inside each pair. |
| Probe | A separate binary built by Cargo from `examples/probe` (`cargo run --example probe`). It exercises the COM layer without starting the pager UI. |
| HWND | An opaque Windows window handle. The probe accepts it in decimal or `0x` hexadecimal. The pager finds its reactor HWND in `native_window`. |
| Snapshot | An immutable view of desktops and windows at one moment. The pager view is built from it. |
| Z-order | Stacking order. In the snapshot, larger values are closer to the top. |
| Failure counter / blacklist | A window whose desktop lookup keeps failing is skipped after 10 attempts. |
| HICON | A borrowed icon handle supplied by the shell. It must not be destroyed by the pager. It is converted to a reactor image source. |
| Foreground hook | A `SetWinEventHook` callback that reports which window became active. |
| Last active window | The real application window remembered for a desktop and restored when the pager switches back to it. |
| Covered-window watchdog | A 500 ms check that re-shows the pager and reapplies topmost if another window covers its center. |
| Pinning | Telling the shell to show the pager's application on every virtual desktop. |
| Component | A `windows-reactor` UI unit with `create`, `update`, and `view`. |
| View | The declarative reactor tree returned by `Component::view`. |
| Tick | A self-rescheduled `spawn_background` message that drives periodic refresh. |
| `native_window` | The small Win32 shim for window properties reactor does not expose. |
| AUMID | Application User Model ID. Windows uses it to pin an application to all virtual desktops. |
| vtable | A table of function pointers that makes up a COM object. The `#[repr(C)]` structs in `core.rs` mirror the operating system's table layout. |

## Unsafe code policy

The project uses Microsoft's official `windows` crate and keeps `unsafe` at the FFI boundary:

- Raw COM vtables and pointers live in `src/desktop/com/`.
- Window enumeration and shell calls live in `src/windows/`, `src/monitors.rs`, `src/foreground.rs`,
  and `src/single_instance.rs`.
- The pager window shim lives in `src/native_window.rs`.
- Logging file-system calls live in `src/logging.rs`.
- The COM apartment bootstrap lives in `src/main.rs`.
- Reactor components, layout, hit testing, settings, and the supervisor are safe Rust and contain no
  raw pointers.

Every `unsafe` block has a nearby safety comment explaining the pointer/thread/handle invariant. The
WinUI migration removes the native window procedure and GDI backend, which removes a large part of the
previous unsafe surface. It does not remove the unsafe COM layer that this application depends on.

## How to use this document

- Check off tasks only when the change is merged and verified, not when it is planned.
- Add new rows to the decision log only for decisions that constrain future phases; routine choices
  belong in the commit message.
- Add a row to the Windows matrix only after running the probe or UI spike on that build; include the
  date.
- When a phase completes, summarize the outcome in one or two sentences in the phase status table.
- When a deviation from the C# behavior is implemented, add it to `ARCHITECTURE.md` §12 and reference
  it here.
- Do not create one progress file per phase. Phase documents go stale and duplicate the checklist;
  use commits and the checklists in this file instead.
