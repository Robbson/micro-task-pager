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
| 4. Main window and rendering | Not started | | | |
| 5. Always-on behavior | Not started | | | |
| 6. Parity and deployment | Not started | | | |

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
- [ ] Renderer consumes the snapshot (Phase 4).

## Phase 4: Main Window and Rendering

Moved out of Phase 1 because each item depends on the window or monitor layout.

- [ ] Application manifest with `asInvoker` and system DPI awareness.
- [ ] Application icon and version resource.
- [ ] Replace the temporary console entry point with the Win32 message loop.
- [ ] Reshape the flat `RendererViewModes` registry value into the per-desktop/screen model.

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

## Verified Windows builds

Record every build on which the probe runs. Do not infer results from a different edition.

| Build | Edition | Selected variant | Probe result | Date | Notes |
|---|---|---|---|---|---|
| 26200 | Windows 11 25H2 | Win11 layout | Count 2, current index, desktop GUIDs, window-to-desktop index, switch 0 -> 1 -> 0, move 0 -> 1 -> 0, pin state, pin, and unpin work | 2026-10-30 | Win10 layout and 21H2/22H2 are untested (postponed) |

## Open questions

| Question | Why it matters | Owner | Status |
|---|---|---|---|
| Which manifest/icon embedding approach: `embed-resource` or linker args from `build.rs`? | Affects build dependencies and reproducibility | | Open |
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

## Code map and glossary

The code is deliberately split into a few small layers. Read `src/lib.rs` first; its module
documentation mirrors this table.

| Path | What it does |
|---|---|
| `src/main.rs` | Temporary console entry point. Initializes logging and COM, enforces single instance, loads settings, and exits. The Win32 window arrives in Phase 4. |
| `src/lib.rs` | Library root shared by the binary and the probe. |
| `src/com.rs` | RAII guard that initializes the COM apartment on the calling thread. |
| `src/single_instance.rs` | Named-mutex guard that prevents two pagers from running. |
| `src/settings.rs` | Registry-compatible settings model and defaults. |
| `src/logging.rs` | Daily file logger with 14-day retention. |
| `src/desktop/mod.rs` | Public safe API (`VirtualDesktopApi`), errors, and `SupervisedDesktop`. |
| `src/desktop/com/core.rs` | Shared COM interface definitions and the generic backend. |
| `src/desktop/com/win10.rs` | Win10 layout (build 10240–21999). |
| `src/desktop/com/win11.rs` | Win11 layout (build 22000 and later). |
| `src/windows/mod.rs` | Window enumeration, filtering, snapshot model, and change hash. |
| `src/windows/icons.rs` | Icon fallback chain and per-window icon cache. |
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
| AUMID | Application User Model ID. Windows uses it to pin an application to all virtual desktops. The probe sets one so pin/unpin can be tested safely. |
| vtable | A table of function pointers that makes up a COM object. The `#[repr(C)]` structs in `core.rs` mirror the operating system's table layout. |

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
