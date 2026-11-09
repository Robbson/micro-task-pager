# UI Framework Options for the Rust Port

Decision-support notes for the UI stack of the Rust implementation. This is not a commitment to
change the current design; it records the alternatives, their dependency and code-size trade-offs,
and what a switch would cost.

Companion documents:

- [`COMPARISON.md`](COMPARISON.md) - measured C# WinForms vs current Rust WinUI 3 deployment and
  resource comparison.
- [`rust-winui/ARCHITECTURE.md`](rust-winui/ARCHITECTURE.md) - current behavior contract and UI direction.
- [`AGENTS.md`](AGENTS.md) - current repository rule: all UI goes through `windows-reactor`.

Measured on 2026-11-09. Estimates for alternative stacks are order-of-magnitude figures, not
measurements, and are marked as such.

## 1. What the 119 MiB actually is

The Rust executable is 3.36 MiB. The 119.4 MiB figure is the x64 **Windows App Runtime framework
package**, a shared MSIX framework that WinUI 3 applications load at startup. It is not bundled into
the executable and it is not paid per application. That figure is for 2.6.5; the minimum supported
2.4 package measured 111.1 MiB on the same machine, so budget roughly 111-119 MiB depending on the
runtime version.

The framework package contains more than WinUI 3: the WinUI XAML stack and composition engine,
DWriteCore, the Windows App SDK app-lifecycle and deployment pieces, and MRTCore. WinUI and its
composition stack are the largest contributors, but "119 MiB for WinUI" is a useful shorthand rather
than a precise decomposition.

Consequences:

- Once the matching framework package is installed, another WinUI application adds almost nothing to
  disk usage.
- A different runtime version adds a side-by-side package. On the measurement machine, all 2.x
  framework packages together occupied 903.1 MiB because several runtime versions were retained.
- The installer/provisioning step is the real friction for a portable utility. Whether it needs
  administrator rights depends on how the runtime is provisioned.
- Self-contained Windows App SDK deployment would move roughly the same payload into the application
  folder. That trades an install step for a much larger portable distribution and does not make the
  dependency disappear.

The C# implementation has a comparable shared dependency, .NET Framework 4.8, but it is an OS
component and is already present on Windows 10 1903+ and Windows 11. The Rust runtime is not inbox.

## 2. The trilemma

For a declarative UI, only two of these three are possible at once:

1. **Small app file and no separate install + declarative UI.** The framework must be shared and
   installed separately (WinUI 3, WebView2).
2. **Small app file and no separate install + no system runtime.** The framework is statically linked,
   so the binary grows (egui, iced, Slint, FLTK).
3. **No system runtime and a native look.** You write more imperative Win32 code, or accept a
   non-native look.

This application is unusual in a helpful way: the pager is a single custom-drawn surface, not a
form-heavy application. That makes the imperative Win32 path much smaller than it would be for a
typical GUI.

## 3. Comparison at a glance

App size and memory for alternatives are rough estimates for a stripped, LTO release build with this
application's feature set. Only the WinUI row is measured.

| Option                                | System dependency                         |            App size | UI model                  | Native look       | Accessibility / IME                                    | Effort      | Main risk                                                    |
| ------------------------------------- | ----------------------------------------- | ------------------: | ------------------------- | ----------------- | ------------------------------------------------------ | ----------- | ------------------------------------------------------------ |
| WinUI 3 + `windows-reactor` (current) | Windows App Runtime 2.4+, 111-119 MiB x64 | 3.36 MiB (measured) | Declarative retained tree | Yes, WinUI/Fluent | Inherited from WinUI                                   | Done        | Young reactor API; runtime install                           |
| Win32 + GDI                           | OS DLLs only                              |        ~0.5-1.5 MiB | Imperative drawing        | Yes, custom-drawn | Manual for custom surface; native controls for dialogs | Medium      | Drawing, hit testing, DPI are manual (largely already built) |
| Win32 + Direct2D/DirectWrite          | OS DLLs only                              |            ~1-2 MiB | Imperative drawing        | Yes, custom-drawn | Same as GDI                                            | Medium-high | Direct2D resource/COM boilerplate                            |
| Thin declarative layer over Win32     | OS DLLs only                              |            ~1-2 MiB | Declarative-ish wrapper   | Yes               | Native controls help                                   | High        | You own focus, layout, theming, accessibility glue           |
| `egui` / `eframe`                     | OS DLLs + GPU driver                      |           ~5-12 MiB | Immediate mode            | No                | Limited                                                | Medium      | Non-native look; repaint model                               |
| `iced`                                | OS DLLs + GPU driver                      |           ~8-20 MiB | Retained, Elm-style       | No                | Limited                                                | Medium      | Size; non-native look                                        |
| `slint`                               | OS DLLs only                              |           ~4-10 MiB | Declarative markup        | No                | Limited                                                | Medium      | Licensing; non-native look                                   |
| `fltk-rs`                             | OS DLLs, FLTK linked                      |            ~2-5 MiB | Imperative/widgets        | No                | Basic native controls                                  | Medium      | Dated look; C++ library                                      |
| WebView2 / Tauri / Dioxus desktop     | WebView2 runtime, ~100+ MiB shared        |            ~1-3 MiB | HTML/CSS declarative      | No                | Web content                                            | High        | Browser process model; non-native                            |

## 4. Option notes

### 4.1 WinUI 3 with `windows-reactor` (current)

Pros:

- Declarative view code, keyed children, WinUI controls, native Fluent look.
- Accessibility, IME, high contrast, and text rendering come from WinUI.
- The measured idle footprint is 99 MiB working set / 64 MiB private and 0.63 % of total system CPU.
- The view layer is compact: roughly 830-1,000 code lines out of ~5,300 application lines.

Cons:

- The Windows App Runtime is a separate install and is not inbox.
- Side-by-side framework versions accumulate; the runtime package is large.
- `windows-reactor` 0.100 is young, and the pager already needs native shims for topmost, layering,
  pinning, taskbar style, the context menu, and DPI, which are outside the reactor API.

Choose this when install friction is acceptable and minimizing UI code and native behavior is the
priority.

### 4.2 Win32 + GDI or GDI+

Pros:

- Zero runtime dependency beyond Windows DLLs; with the static MSVC runtime there is no VC++
  redistributable either.
- The pager is already custom-drawn. The layout model already produces rectangles and hit-test areas;
  drawing them with GDI is a direct port of what the C# renderer does.
- Very small binary and the lowest memory footprint of the realistic options.
- Full control over layered alpha, topmost, taskbar style, DPI, and icon drawing. Most of those
  already exist in `native_window.rs`.

Cons:

- No declarative tree. A `WM_PAINT` handler draws the whole surface, and the Settings window must be
  built from native controls or a dialog template.
- GDI text and scaling are less refined than DirectWrite/Direct2D, especially on high-DPI monitors.
- Accessibility for the custom-drawn pager must be added manually if it is required; the dialogs get
  it from native controls.

Choose this when the runtime dependency is the dominant concern and a low-level draw path is
acceptable.

### 4.3 Win32 + Direct2D/DirectWrite

Pros:

- Same zero-install dependency story as GDI: Direct2D, DirectWrite, DXGI, and DWM are OS components.
- Better antialiasing, geometry, and text than GDI, and good per-monitor DPI behavior.
- Still a direct fit for the pager's draw-list architecture.
- No managed runtime, no XAML, no WebView.

Cons:

- More COM boilerplate than GDI: device resources, target bitmaps, brushes, text formats, and
  handling device loss.
- Slightly larger code surface and binary than GDI.
- Dialogs still need native controls or custom drawing.

This is the strongest "native, dependency-free, reasonable code size" candidate if the project leaves
WinUI.

### 4.4 Thin declarative layer over Win32

Options in this space:

- `native-windows-gui` gives macros and a declarative-ish DSL over native Win32 controls. Small
  runtime, native look, but a small ecosystem and uncertain long-term maintenance.
- A purpose-built view enum plus a small diff/update loop over Win32 controls. This is essentially
  rebuilding a tiny Reactor over Win32; it can be tailored to this app, but it is UI-framework work
  and conflicts with the current "do not build a UI framework" rule.

Choose this only if a declarative dialogs layer is considered essential and the team is willing to
own a small in-house toolkit.

### 4.5 Immediate-mode toolkits: `egui` / `eframe`

Pros:

- Extremely terse, declarative-in-practice UI code.
- Self-contained binary; no system runtime.
- Good for tools, inspectors, and overlays.

Cons:

- Non-native look and interaction model.
- Immediate mode likes to redraw continuously; an always-on-top pager should idle and only redraw on
  changes, which needs care.
- Accessibility, IME, and native context menus are weaker than WinUI or native controls.
- Still needs native shims for layered alpha, topmost, taskbar style, and desktop pinning.
- The binary is larger than a Win32 build.

A plausible fit for the Settings window, less so for the pager's native behavior.

### 4.6 Retained Rust toolkits: `iced`, `slint`, `fltk-rs`

- **`iced`** is a retained, Elm-style toolkit. Good developer experience and a clean state model, but
  a wgpu-based renderer makes the binary larger and adds GPU driver assumptions. Look is non-native.
- **`slint`** uses declarative `.slint` markup and supports a software renderer, so it can stay small
  and GPU-free with that backend; the Skia/femtovg backends add GPU assumptions. It is the most
  interesting cross-platform declarative option for a small app. Verify licensing (GPLv3/commercial
  plus a royalty-free option under conditions), accessibility, IME, and native window behaviors.
- **`fltk-rs`** wraps FLTK, which is small and statically linkable under the LGPL static-linking
  exception. The look is dated and non-native; the C++ build dependency is an extra toolchain cost.

All three would replace the WinUI dependency with a static runtime cost and a non-native look.

### 4.7 WebView-based options: WebView2, Tauri, Dioxus desktop

Pros:

- HTML/CSS is the most declarative option and familiar to many developers.
- The Rust shell is tiny.

Cons:

- WebView2 is a shared runtime that is present on Windows 11 and most Windows 10 machines via Edge,
  but it is not guaranteed, especially offline or on stripped-down images.
- A browser engine and its helper processes are heavy for a topmost strip of icons.
- Native behaviors (layered alpha, topmost, taskbar pinning, click-through decisions) still need
  native shims.
- Non-native look and interaction.

Not recommended for this application.

### 4.8 DIY composition: `winit` + `softbuffer` / `tiny-skia`

Pros:

- No system runtime, small binary, full control.
- `softbuffer` presents a CPU-rendered buffer; `tiny-skia` provides a 2D path.

Cons:

- You own the event loop integration, painting, and all widget behavior for the dialogs.
- The native context menu, layered alpha, topmost, and pinning remain native shims.
- More total code than the Direct2D path for this app, with no native control benefits.

An option only if the project deliberately wants a portable software-rendered surface.

## 5. Fit for Micro Task Pager

Current Rust source split (application code, excluding generated files and build output):

| Part                                                   | Code lines | Backend-specific?          |
| ------------------------------------------------------ | ---------: | -------------------------- |
| Pager state, ticks, and reactor glue (`pager/mod.rs`)  |       ~836 | Partly                     |
| Layout and hit testing (`pager/layout.rs`)             |       ~633 | No                         |
| Input state machine (`pager/input.rs`)                 |       ~327 | No                         |
| Icon cache (`pager/icons.rs`)                          |      small | Partly (image source type) |
| Dialog lifecycle (`pager/dialog_state.rs`)             |      small | No                         |
| Declarative pager view (`pager/view.rs`)               |       ~250 | Yes                        |
| Settings state/model/placement                         |       ~800 | No                         |
| Declarative Settings view (`dialogs/settings/view.rs`) |       ~482 | Yes                        |
| About view (`dialogs/about.rs`)                        |       ~100 | Yes                        |
| Windows, monitors, foreground, native shim             |     ~1,365 | Platform, reusable         |
| COM interop (`desktop/`)                               |       ~710 | No                         |

The declarative Reactor view code is roughly 830-1,000 code lines. The remainder - snapshotting,
layout, hit testing, input, dialog state, settings, COM interop, pinning, topmost, and the native
context menu - is independent of the UI toolkit and would survive a backend change.

That is why the Win32/Direct2D path is credible here: it replaces the smallest, most isolated part of
the codebase while keeping the parts that took the most work.

Native behaviors that any replacement must keep:

- borderless topmost window that does not steal focus,
- layered-window opacity,
- taskbar style / application pinning to all desktops,
- native context menu with the same commands and disabled states,
- per-monitor DPI with a rebuilt device-pixel grid,
- click, drag-window-between-desktops, and pager dragging,
- last-active-window restore,
- dialog placement next to the pager,
- snapshot/icon pipeline and desktop-switch recovery.

WinUI and the custom Win32 options already do all of these. Cross-platform toolkits would need
verification for several of them.

## 6. Expected cost of a Win32/Direct2D backend

Rough estimates, not measurements:

| Metric                |                   Current WinUI |                            Win32/Direct2D estimate |
| --------------------- | ------------------------------: | -------------------------------------------------: |
| Executable            |                        3.36 MiB |                                            1-2 MiB |
| Runtime prerequisite  | 119.4 MiB x64 framework (2.6.5) |                                none beyond Windows |
| Application code      |                    ~5,300 lines |                                 ~5,500-6,500 lines |
| Working set           |                          99 MiB |                                   likely 20-40 MiB |
| Private bytes         |                          64 MiB |                                   likely 10-25 MiB |
| Startup               |                 248-286 ms warm |                                likely below 150 ms |
| Declarative view code |                ~830-1,000 lines | replaced by ~1,000-1,800 lines of draw/dialog code |

The code estimate holds only if the existing layout, input, state, settings, and COM modules are
reused. A from-scratch Win32 rewrite would be a different project.

## 7. Recommendation

1. **If the runtime install is the dominant concern:** prototype Win32 + Direct2D for the pager and
   native dialog templates (or `TaskDialogIndirect` for About) for the dialogs. Reuse everything below
   the view layer. This is the best dependency-to-code ratio for this application.
2. **If minimizing UI code and maximizing native behavior is the priority:** keep WinUI 3. The
   runtime is shared, the marginal per-app cost is near zero after the first install, and the
   declarative view surface is small.
3. **If a single portable file is the hard requirement and a non-native look is acceptable:** evaluate
   `slint` first (software renderer, declarative markup), then `egui` (most terse). Verify accessibility,
   IME, layered alpha, topmost, and taskbar pinning with a spike.
4. **Avoid WebView2/Tauri** for this UI shape.

None of these changes the undocumented COM interop risk. That layer is independent of the UI choice.

## 8. Suggested spike

If the project wants to explore the Win32/Direct2D direction, the smallest useful experiment is:

1. Add a Cargo feature for the UI backend and keep the current WinUI build as the default.
2. Implement only the pager window: native borderless topmost window, layered opacity, draw the
   existing layout rectangles and icons with Direct2D, and wire the existing input and snapshot
   modules.
3. Keep About as `TaskDialogIndirect` and Settings out of the first spike, or build it from native
   controls.
4. Measure the same metrics as `COMPARISON.md`: executable size, runtime prerequisites, startup,
   working set, private bytes, handles, GDI/USER objects, idle CPU over 60 s, and application code
   lines.
5. Verify the native-behavior checklist in section 5 before judging the result on size alone.

Acceptance criteria should include per-monitor DPI, mixed-DPI monitors, topmost behavior across
desktop switches, layered opacity, taskbar pinning, and the context menu - not just binary size.

Because the current repository rules mandate `windows-reactor`, adopting a different backend requires
an explicit architecture decision and updates to `ARCHITECTURE.md` and `AGENTS.md`. The spike should
therefore stay behind a feature flag until that decision is made.

## 9. How much of the Rust code is Reactor/WinUI-coupled?

Only 6 of the 28 source files in `rust-winui/src/` import `windows_reactor`:

| File                       | Approx. code lines | Reactor role                                                              | Coupling                                                  |
| -------------------------- | -----------------: | ------------------------------------------------------------------------- | --------------------------------------------------------- |
| `pager/mod.rs`             |               ~836 | `Component` lifecycle, scheduling, opening dialogs, `LocalSender`         | Partial; most of the file is state and Win32 window logic |
| `pager/view.rs`            |               ~250 | Builds the entire pager `View`                                            | Full                                                      |
| `pager/icons.rs`           |                ~60 | Wraps PNG bytes in `EncodedImage`                                         | Trivial; one type                                         |
| `dialogs/settings/mod.rs`  |               ~200 | `Component` lifecycle, `Color`, `ContentDialogResult`, preview scheduling | Mostly adapter logic                                      |
| `dialogs/settings/view.rs` |               ~482 | Builds the Settings `View`                                                | Full                                                      |
| `dialogs/about.rs`         |               ~100 | `Component` plus the About `View`                                         | Full                                                      |

Everything else has no Reactor dependency:

- `pager/layout.rs`, `pager/input.rs`, `pager/dialog_state.rs` - geometry, hit testing, and state machines.
- `dialogs/settings/draft.rs`, `dialogs/placement.rs`, `dialogs/mod.rs` - settings draft, placement, and the `DialogLink` callback.
- `settings.rs`, `windows/`, `monitors.rs`, `foreground.rs`, `desktop/`, `com.rs`, `single_instance.rs`, `logging.rs`, `native_window.rs` - platform and application services.

Rough split:

- **~1,200 code lines (~23 % of ~5,300)** are adapter-specific: views, `Component` glue, Reactor color/result types, and `EncodedImage`.
- **~4,100 code lines** are independent of the UI framework.
- `pager/mod.rs` is the swing file: ~200 lines of it are Reactor lifecycle, the rest is the application state that currently happens to live inside a `Component`.

Leaky types that need a neutral replacement before a second backend:

| Current type           | Where                                    | Neutral replacement                         |
| ---------------------- | ---------------------------------------- | ------------------------------------------- |
| `EncodedImage`         | `pager/icons.rs`, `pager/view.rs`, About | `IconImage { bytes: Arc<[u8]>, size_px }`   |
| `Color`                | Settings color editor                    | `u32` ARGB, which `Settings` already stores |
| `ContentDialogResult`  | Settings color picker                    | A small `ColorPickerResult` enum            |
| `ComponentContext`     | Pager ticks, dialogs, close              | An effect list plus a `UiHost`/timer port   |
| `LocalSender<Message>` | Pager message queue                      | `DialogLink`-style callback or event sink   |
| `View` / `ViewContext` | Views                                    | Adapter-only; never leaves the backend      |

The Win32 window shim (`native_window.rs`, `PagerWindow`, `WheelHook`, `ForegroundTracker`) is
**not** Reactor coupling. It is Win32 platform code and can be shared by a Win32 backend; only a
non-Win32 toolkit would need a different window host.

## 10. A feature-selectable UI boundary

The clean target is:

```text
app core (events, state, effects)  ->  UI adapter  ->  Reactor/WinUI, Win32, ...
        |                                  |
        +-- snapshot, layout, input,       +-- windows, views, timers,
            settings, dialog state,            dialogs, icon presentation
            desktop COM, window shim
```

The core should be loop-agnostic:

```rust
enum AppEvent {
    Tick(TickKind),
    Pointer(PointerEvent),
    Wheel { x: f64, y: f64, shift: bool },
    Dialog(DialogEvent),
    RestoreLastActive(u32),
}

enum Effect {
    Schedule { delay: Duration, event: AppEvent },
    OpenDialog(DialogKind),
    CloseWindow,
    MovePager { x: i32, y: i32 },
    // ...
}

impl PagerCore {
    fn update(&mut self, event: AppEvent, host: &mut dyn UiHost) -> Vec<Effect> {
        /* ... */
    }
}
```

The adapter owns the message loop, timers, window creation, painting, and dialog widgets. The core
owns the pager state machine and returns effects; it never mentions `View`, `ComponentContext`, or
`EncodedImage`.

### Packaging options

**Option A - one crate with Cargo features (quickest):**

```toml
[features]
default = ["ui-reactor"]
ui-reactor = ["dep:windows-reactor"]
ui-win32 = []
```

```rust
#[cfg(all(feature = "ui-reactor", feature = "ui-win32"))]
compile_error!("select exactly one UI backend");

#[cfg(feature = "ui-reactor")]
mod reactor_backend;
#[cfg(feature = "ui-win32")]
mod win32_backend;
```

Pros: small change, no workspace churn. Cons: Cargo features are additive, so the boundary is
enforced only by the `compile_error!` and review discipline; a dependency can accidentally pull in
both backends.

**Option B - workspace crates (recommended if two backends are real):**

- `micro-task-pager-core` - state, events, effects, layout, input, settings, snapshot, COM.
- `micro-task-pager-platform` - Win32 window shim, monitors, foreground, logging.
- `micro-task-pager-ui-reactor` - current views and components.
- `micro-task-pager-ui-win32` - future native backend.
- `micro-task-pager` - thin binary selecting the adapter.

The core crate simply does not depend on `windows-reactor`, so the boundary is enforced at compile
time. The cost is a workspace split and updating the examples.

The goal is UI-framework independence, not OS independence. The core may keep using the `windows`
crate and `HWND`; a Win32 adapter already needs them. A non-Windows toolkit would need a port for the
window host, but that is a much larger project and is not required for the Reactor/Win32 split.

**Option C - separate binaries, no exclusive features:**

Build `pager-reactor` and `pager-win32` binaries from the same core. This avoids mutually exclusive
feature combinations entirely and can produce both backends in one build. It is the least surprising
option for users and CI.

## 11. Difficulty, risk, and effort

| Step                                         | What it involves                                                                                    | Relative effort | Risk                                                    |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------- | --------------- | ------------------------------------------------------- |
| 1. Extract `PagerCore` from `Pager`          | Move ~300-500 lines of state and tick logic out of the `Component`; keep Reactor behavior unchanged | Medium          | Borrow/ownership around `ComponentContext`; tick timing |
| 2. Define neutral events, effects, and ports | `AppEvent`, `Effect`, `UiHost`, `HostWindow`, `DialogHost`, `IconImage`                             | Small-medium    | Finding all the leaks listed above                      |
| 3. Reactor adapter                           | Thin `Component` that delegates to the core and renders `view.rs`                                   | Small           | Regression in window/dialog lifecycle                   |
| 4. Win32 pager backend                       | HWND and message loop, GDI/Direct2D paint, DPI, timers, context menu                                | Large           | Native behavior parity                                  |
| 5. Dialog backends                           | About via `TaskDialogIndirect`; Settings from native controls or a dialog template                  | Large           | This is the biggest UI chunk; accessibility/IME         |
| 6. Build matrix and CI                       | Feature or binary matrix, tests per backend, behavior checklist                                     | Small           | Every backend needs the full manual checklist           |

Assessment:

- **Decoupling the core from Reactor without adding a second backend: moderate, roughly one focused
  week.** It is an invasive but mostly mechanical refactor. The pure modules and 52 tests stay; the
  risk is concentrated in the `Component` lifecycle and timer semantics.
- **A production Win32 backend including Settings: high, roughly two to four additional weeks
  depending on how much native dialog polish is required.** The pager itself is the easier half
  because the layout and input models already exist.
- **Difficulty rating: 5/10 for the boundary, 8/10 for shipping two polished backends.**

Recommendation: do steps 1-3 only if multiple backends are genuinely likely. That split improves
testability and keeps the Reactor build working, with no user-visible change. The pure state machines
already have unit tests; after the split, add a headless test harness that drives `PagerCore` with
events and asserts the returned effects, so the core stays covered without a UI. Do not build the
Win32 backend until the Windows App Runtime dependency is a confirmed product blocker; the backend,
not the decoupling, is the expensive part.

## 12. Open questions

- Does the target audience already have the Windows App Runtime installed, or is it a fresh-machine
  install every time?
- Is a non-native look acceptable for the Settings window, or is WinUI/Fluent a product requirement?
- Is accessibility for the custom-drawn pager a requirement, or only for the dialogs?
- How important are IME, high contrast, and screen readers for the Settings window?
- Is a single-file portable executable a hard distribution requirement, or is an installer acceptable?
- Does the project want to keep support for Windows 10 builds before 1809? Dropping WinUI would remove
  the Windows App Runtime floor, although the virtual-desktop backend still targets 10240+.
- Is the extra toolchain cost of a C/C++ dependency (FLTK) acceptable for the build?
