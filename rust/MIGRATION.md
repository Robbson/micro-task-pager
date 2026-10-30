# Rust Migration – Progress and Phase Log

This is the living execution document for the Rust migration. `ARCHITECTURE.md` is the contract and
the source of truth for design; this file tracks progress, verification evidence, and decisions that
are not already architectural.

Keep this document short. Update it when a phase starts or completes, when a Windows build is verified,
or when an open question is resolved. Do not duplicate architecture content; link to the relevant
section instead.

## Phase status

| Phase | Status | Started | Completed | Notes |
|---|---|---|---|---|
| 1. Scaffold | Complete | 2026-10-30 | 2026-10-30 | Core scaffold done; remaining items re-scoped to Phase 2 and Phase 4 |
| 2. Virtual desktop subsystem | Complete | 2026-10-30 | 2026-10-30 | Win10/Win11 layouts, supervisor, logging, and probe implemented; older-build verification postponed |
| 3. Window model | Complete | 2026-10-30 | 2026-10-30 | Enumeration, filters, icons, z-order, failure handling, and snapshot hash implemented |
| 4. Main window and rendering | Complete | 2026-10-30 | 2026-10-30 | Win32 window, monitor layout, GDI renderer, interactions, manifest, icon, and version resource implemented |
| 5. Always-on behavior | Complete | 2026-10-30 | 2026-10-30 | Pinning, topmost recovery, foreground hook, and last-active restore implemented |
| 6. Parity and deployment | Complete | 2026-10-30 | 2026-10-30 | Release build/deployment verified on 26200, single monitor; older builds and multi-monitor postponed; C# retained |

## Phase 1: Scaffold

- [x] Cargo crate in `rust/` with toolchain channel and `rustfmt`/`clippy`.
- [x] `rust/.gitignore` for `target/`.
- [x] Single-instance mutex with the release mutex name.
- [x] COM STA initialization guard.
- [x] Settings model, defaults, and tolerant registry import.
- [x] `cargo fmt`, `cargo clippy -D warnings`, `cargo build`, and `cargo test` pass.

Phase 1 is closed with the following items moved so they are not lost:

- Logging moves to Phase 2 because the COM supervisor needs it.
- Manifest, icon, version resource, and the Win32 message loop move to Phase 4.
- The per-desktop/screen reshape of `RendererViewModes` moves to Phase 4 because it depends on the
  monitor layout.

## Phase 2: Virtual Desktop Subsystem

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

Implementation is complete. Verification on Windows 10, Win10LTSC, and Windows 11 21H2/22H2 is
postponed until suitable images are available; it is tracked in the Postponed section and does not
block Phase 3.

## Phase 3: Window Model

- [x] Immutable `WindowInfo` / `WindowSnapshot` model, sorted by window handle.
- [x] `EnumWindows` filtering: shell window, invisible windows, empty titles, class blacklist, own
      process, blacklisted handles.
- [x] Desktop index per window with failure counters and a blacklist.
- [x] Z-order, foreground/active state, title, class, process ID, and rectangle.
- [x] Icon fallback chain (`WM_GETICON`, class icon, `IDI_APPLICATION`) plus a per-window cache.
- [x] Snapshot hash for change detection.
- [x] Probe `snapshot` command verified on build 26200.
- [x] Renderer consumes the snapshot (Phase 4).

## Phase 4: Main Window and Rendering

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
- [x] Always-on behavior moved to Phase 5 and implemented.

## Phase 5: Always-On Behavior

- [x] Stable Application User Model ID and pinning to all desktops when `ShowAppInTaskbar` is enabled.
- [x] Retry pinning on the secondary timer until it succeeds; tolerate shell failures.
- [x] Covered-window watchdog using `WindowFromPoint`, `ShowWindowAsync`, and topmost reassertion.
- [x] `EVENT_SYSTEM_FOREGROUND` hook with an atomic handoff to the UI thread.
- [x] Remember the last active window per desktop and restore it after a pager switch, with a 150 ms
      retry for the switch animation.
- [x] Respect `ShowAppInTaskbar` when choosing `WS_EX_APPWINDOW`.

## Phase 6: Parity and Deployment

- [x] Walked the behavioral checklist in `ARCHITECTURE.md` §9 on build 26200, single monitor.
- [x] Release build is x64 Windows GUI, uses a static MSVC runtime, and has no .NET or VC++
      redistributable dependency.
- [x] Embedded icon, version resource, and system-DPI manifest verified.
- [x] Logs use `%LOCALAPPDATA%\MicroTaskPager\logs`; 14-day purge verified with an old file.
- [x] Clean shutdown through `WM_CLOSE`; no lingering process and no startup error in the log.
- [x] Deployment from a clean directory works.
- [x] Pinning and cross-desktop visibility verified; covered-window recovery and last-active restore
      are implemented and exercised manually.
- [ ] Older Windows builds and multi-monitor layouts (postponed).
- [ ] Remove the C# implementation (only after explicit user instruction and older-build parity).

## Postponed

These items are not scheduled and do not block the next phase.

| Item | Reason | Revisit when |
|---|---|---|
| Run the probe on Windows 10 and Win10LTSC | No older images or virtualization available | A Windows 10 image or VM is available |
| Run the probe on Windows 11 21H2 and 22H2 | Same | A suitable image is available |
| Validate pinning with `ShowInTaskbar` variants | Needs manual UI testing alongside the pager window | Phase 5 |

## Decision log

| Date | Decision | Rationale | Reference |
|---|---|---|---|
| 2026-10-30 | GDI renderer first, Direct2D deferred | Layered-window alpha and crate maturity outweigh GPU benefits | ARCHITECTURE §7.1, §7.3 |
| 2026-10-30 | No settings UI in v1 | Product decision; keeps the port small and avoids a GUI framework | ARCHITECTURE §8.1 |
| 2026-10-30 | One safe virtual-desktop API plus per-build vtable modules | The C# variants duplicate logic; only the layouts differ | ARCHITECTURE §6.1 |
| 2026-10-30 | Use `windows` 0.62 and the 0.100 native-app crates where applicable | The umbrella `windows` crate has not moved to 0.100 | ARCHITECTURE §5.1 |
| 2026-10-30 | Use the `stable` toolchain channel for now | The installed `1.99.1` rustup toolchain is missing `cargo.exe`; stable is also 1.99.1 | Open question below |
| 2026-10-30 | Implement the Win11_ backend and probe first | The current machine runs build 26200; validate the vtable strategy before porting older variants | Phase 2 |
| 2026-10-30 | Collapse the four C# variants into two Rust layouts | The methods the app uses have identical GUIDs and vtable prefixes within the Win10/Win10LTSC pair and the Win11/Win11_ pair | ARCHITECTURE §6.3 |
| 2026-10-30 | Use a small in-house file logger | Avoids a dependency and matches the existing log directory and 14-day retention | ARCHITECTURE §3 |
| 2026-10-30 | Close Phase 1 with re-scoped items instead of finishing it first | Manifest, icon, message loop, and view-mode reshape depend on Phase 4; only logging was pulled into Phase 2 | Phase 1 |
| 2026-10-30 | Postpone older-build verification | No Windows 10 or older Windows 11 images are available; code is in place and the matrix can be filled later | Postponed section |
| 2026-10-30 | Derive z-order from `EnumWindows` order | Avoids the C# O(n²) `GetWindow` walk; relative order is equivalent |
| 2026-10-30 | Prune desktop-lookup failure counters using every window that reaches the lookup | The C# code prunes against successful windows only, so a failing window loses its counter each snapshot and never reaches the blacklist | ARCHITECTURE §10 |
| 2026-10-30 | Use GDI `DrawIconEx` instead of converting every `HICON` to a bitmap | Fewer GDI objects and no manual icon-bitmap lifetime; the pager redraws only on changes |
| 2026-10-30 | Pre-blend the separator color | `FillRect` has no alpha, so the GDI color is the result of blending C#'s translucent light blue over the default background |
| 2026-10-30 | Use the common-controls image-list drag image for window drags | Matches the usual Windows ghost icon and avoids implementing OLE drag source/target interfaces |
| 2026-10-30 | Pin after the window is visible and retry on the secondary timer | Pinning needs an application view; the shell can reject an early attempt |
| 2026-10-30 | Let the foreground hook store only the handle | The callback has no access to the desktop API; mapping on the next timer tick keeps COM on the UI thread |
| 2026-10-30 | Use the pager center for the covered-window check | Matches the C# watchdog and is cheap enough for the 500 ms timer |
| 2026-10-30 | Statically link the MSVC runtime | The release executable must run without the Visual C++ redistributable |
| 2026-10-30 | Keep the C# implementation for now | Older Windows builds and multi-monitor parity are still postponed; removal needs explicit instruction |
| 2026-10-30 | Use screen cursor coordinates while dragging the pager | Client coordinates change as the window moves, which caused a feedback loop and flicker |
| 2026-10-30 | Drop topmost and suspend its timer reassertion while the context menu is open | A topmost pager otherwise covers its own popup menu |
| 2026-10-30 | Keep unsafe confined to FFI modules instead of adopting `winsafe` | `winsafe` is a UI wrapper, not a safe abstraction for undocumented shell COM; the official `windows` crate is the right binding |

## Verified Windows builds

Record every build on which the probe runs. Do not infer results from a different edition.

| Build | Edition | Selected variant | Probe result | Date | Notes |
|---|---|---|---|---|---|
| 26200 | Windows 11 25H2 | Win11 layout | Count 2, current index, desktop GUIDs, window-to-desktop index, switch 0 -> 1 -> 0, move 0 -> 1 -> 0, pin state, pin, and unpin work | 2026-10-30 | Win10 layout and 21H2/22H2 are untested (postponed) |

## Open questions

| Question | Why it matters | Owner | Status |
|---|---|---|---|
| Does `IVirtualDesktopPinnedApps` pinning still require `ShowInTaskbar` on current builds? | Always-on behavior depends on it | | Postponed to Phase 5 |
| Should the toolchain be pinned to an exact version? | `1.99.1` is currently installed without `cargo.exe`, so `stable` (also 1.99.1) is used | | Open |

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

## Code map and glossary

The code is deliberately split into a few small layers. Read `src/lib.rs` first; its module
documentation mirrors this table.

| Path | What it does |
|---|---|
| `src/main.rs` | Process entry point: logging, COM apartment, single instance, then the message loop. |
| `src/lib.rs` | Library root shared by the binary and the probe. |
| `src/app.rs` | Application state, timers, snapshot refresh, layout rebuild, mouse and command handling. |
| `src/window.rs` | Win32 window, window procedure, timers, context menu. |
| `src/monitors.rs` | Monitor enumeration and work areas. |
| `src/com.rs` | RAII guard that initializes the COM apartment on the calling thread. |
| `src/single_instance.rs` | Named-mutex guard that prevents two pagers from running. |
| `src/settings.rs` | Registry-compatible settings model, defaults, and position/view-mode writers. |
| `src/logging.rs` | Daily file logger with 14-day retention. |
| `src/foreground.rs` | Foreground WinEvent hook and last-active-window tracking per desktop. |
| `src/desktop/mod.rs` | Public safe API (`VirtualDesktopApi`), errors, and `SupervisedDesktop`. |
| `src/desktop/com/core.rs` | Shared COM interface definitions and the generic backend. |
| `src/desktop/com/win10.rs` | Win10 layout (build 10240–21999). |
| `src/desktop/com/win11.rs` | Win11 layout (build 22000 and later). |
| `src/windows/mod.rs` | Window enumeration, filtering, snapshot model, and change hash. |
| `src/windows/icons.rs` | Icon fallback chain and per-window icon cache. |
| `src/render/mod.rs` | Layout model, hit testing, drawing instructions, and `Painter` trait. |
| `src/render/gdi.rs` | Double-buffered GDI backend. |
| `app.rc`, `app.manifest`, `build.rs` | Icon, version resource, DPI/asInvoker manifest, and resource embedding. |
| `.cargo/config.toml` | Static MSVC runtime for self-contained release builds. |
| `examples/probe/main.rs` | Standalone probe; not part of the pager binary. |

Terms that appear in the code:

| Term | Meaning |
|---|---|
| COM apartment / STA | A threading rule for COM objects. The shell objects must be created and used on a single-threaded apartment. `Apartment` enforces the initialization step; the code must stay on that thread. |
| `VirtualDesktopApi` | The only trait the rest of the application should use. It hides COM pointers and version differences behind plain Rust methods and `Result`. |
| `SupervisedDesktop` | A small resilience wrapper. It owns one backend and forwards calls. If a call fails with a restartable COM error, it throws the backend away, creates a new one, and retries once. "Supervised" means the wrapper watches the backend, not that a separate thread runs. |
| Backend / layout | The actual COM implementation for a Windows build family. There are two layouts instead of four C# variants because the used vtables are identical inside each pair. |
| Probe | A separate binary built by Cargo from `examples/probe` (`cargo run --example probe`). It exercises the COM layer without starting the pager UI. |
| HWND | An opaque Windows window handle. The probe accepts it in decimal or `0x` hexadecimal. |
| Snapshot | An immutable view of desktops and windows at one moment. The renderer will draw from it, and its `hash` tells the UI whether a repaint is needed. |
| Z-order | Stacking order. In the snapshot, larger values are closer to the top. |
| Failure counter / blacklist | A window whose desktop lookup keeps failing is skipped after 10 attempts, mostly to avoid snapped-window helper windows spamming the log. |
| HICON | A borrowed icon handle supplied by the shell. It must not be destroyed by the pager. |
| Foreground hook | A `SetWinEventHook` callback that reports which window became active. |
| Last active window | The real application window remembered for a desktop and restored when the pager switches back to it. |
| Covered-window watchdog | A 500 ms check that re-shows the pager and reapplies topmost if another window covers its center. |
| Pinning | Telling the shell to show the pager's application on every virtual desktop. It works by AUMID. |
| `PagerLayout` | The computed geometry for one frame: desktop columns, screens, clickable cells, and the `+N` overflow tile. Drawing and hit testing both use it. |
| `Painter` | A tiny drawing interface. `GdiPainter` implements it with Win32 GDI; layout code never calls GDI directly. |
| GDI | The classic Win32 drawing API. The pager draws into a memory DC and blits once per paint. |
| AUMID | Application User Model ID. Windows uses it to pin an application to all virtual desktops. The probe sets one so pin/unpin can be tested safely. |
| vtable | A table of function pointers that makes up a COM object. The `#[repr(C)]` structs in `core.rs` mirror the operating system's table layout. |

## Unsafe code policy

`winsafe` is not a dependency. It is a higher-level wrapper for ordinary Win32 UI work, but it does
not make undocumented shell COM interfaces or raw vtables safe. The project uses Microsoft's official
`windows` crate and keeps `unsafe` at the FFI boundary:

- Raw COM vtables and pointers live in `src/desktop/com/`.
- GDI calls live in `src/render/gdi.rs`.
- Win32 window/message/monitor calls live in `window.rs`, `windows/`, `monitors.rs`, and `app.rs`.
- Layout, snapshot models, settings, and the supervisor are safe Rust and contain no raw pointers.

Every `unsafe` block has a nearby safety comment explaining the pointer/thread/handle invariant. A
future safety audit can tighten the wrappers, but switching to a UI framework would not remove the
unsafe COM layer that this application depends on.

## How to use this document

- Check off tasks only when the change is merged and verified, not when it is planned.
- Add new rows to the decision log only for decisions that constrain future phases; routine choices
  belong in the commit message.
- Add a row to the Windows matrix only after running the probe on that build; include the date.
- When a phase completes, summarize the outcome in one or two sentences in the phase status table.
- When a deviation from the C# behavior is implemented, add it to `ARCHITECTURE.md` §10 and reference
  it here.
- Do not create one progress file per phase. Phase documents go stale and duplicate the checklist;
  use commits and the checklists in this file instead.
