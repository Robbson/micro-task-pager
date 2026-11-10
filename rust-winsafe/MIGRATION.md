# rust-winsafe - Migration Log

Status: stable implementation. It is a native Win32 pager built with
[`winsafe`](https://crates.io/crates/winsafe) and has **no Windows App Runtime / WinUI dependency**.
It has fewer features than `csharp-winforms`, and new features are added only after a delay.
`csharp-winforms` has the latest features; `rust-winui` is the experimental WinUI 3 implementation.

This document tracks the winsafe variant only. For the WinUI migration see
[`../rust-winui/MIGRATION.md`](../rust-winui/MIGRATION.md); for the framework trade-offs see
[`../rust-winui/UI-FRAMEWORK-COMPARISON.md`](../rust-winui/UI-FRAMEWORK-COMPARISON.md).

## Baseline

The crate was copied from commit `deed611295547cb4d7baeb3766ff3c882c5191ac` (`rust/`), the last
pure-Win32 Rust implementation before the reactor migration. That baseline already had a working
pager, GDI renderer, virtual-desktop COM layer, settings storage, About dialog, and 18 unit tests, but
built the window class and dialog by hand.

The platform modules (`desktop/`, `windows/`, `monitors`, `foreground`, `settings`, `com`,
`single_instance`, `logging`) moved into `crates/micro-task-pager-core` on 2026-11-10. Their
behavior stays compatible with the baseline, and both Rust front ends now share them. See the
"Shared platform crate" section below.

## Source layout

The winsafe crate now mirrors the onboarding-oriented module split of `rust-winui` where the two UI
stacks allow it:

- `src/app.rs` is the coordinator; its methods are grouped by concern (construction, timers,
  snapshot/layout, painting/input, drag/actions, menu, settings/dialogs, window maintenance) with
  `// ---- ... ----` banners.
- `src/render/` is split into `layout.rs` (geometry and hit testing), `paint.rs` (drawing
  instructions through the `Painter` trait), and `gdi.rs` (the GDI backend), matching the
  `rust-winui` `pager/layout`, `pager/view`, and platform split.
- `src/dialogs/` contains both `about.rs` and the `.rc`-backed `settings` dialog.
- `src/window.rs` is the winsafe window host; event handlers only decode input and forward it to
  `App`.

## Status

| Area | Status | Notes |
|---|---|---|
| Crate scaffold, resources, manifest | Done | Resource paths point at `../csharp-winforms/Resources/` |
| Virtual-desktop COM, snapshot, settings | Done (shared core) | Moved to `micro-task-pager-core`; behavior unchanged from the baseline |
| Pager window host | **Migrated to winsafe** | `gui::WindowMain` + typed events, no `WNDCLASSW`/`WndProc`/manual message loop |
| Pager rendering | Copied | Raw GDI `Painter`; winsafe adds no drawing value here |
| Context menu | Copied | Raw `TrackPopupMenu` shim |
| About dialog | **Migrated to winsafe** | `gui::WindowModal` + `Label` + `Button`; PNG decoded with `png`, logo painted with GDI |
| Settings dialog | **Implemented (v1)** | Fixed `.rc` template, winsafe `new_dlg` controls, tab pages, Save/Cancel/Defaults |
| Build pipeline (`build.ps1`, VS Code, AGENTS.md) | **Wired** | `-Target winsafe` stages `prod/winsafe`; VS Code build/run/launch entries added |

## What changed versus the baseline

- Added `winsafe = 0.0.29` (pinned; the crate is `0.0.x` and its API may change). The raw `windows`
  crate is retained for GDI, shell helpers, timers, menus, and layered-window calls; COM and the
  platform layer live in `micro-task-pager-core`.
- Removed the `windows-reactor` dev-dependency and `examples/reactor_about.rs`. This crate is
  reactor-free and does not require the Windows App Runtime.
- `src/window.rs` was rewritten:
  - winsafe creates and owns the main window and message loop;
  - input is handled by typed `wm_l_button_down`, `wm_mouse_move`, `wm_mouse_wheel`,
    `wm_r_button_up`, `wm_timer`, `wm_display_change`, and `wm_capture_changed` events;
  - winsafe stores its own state in `GWLP_USERDATA`, so the `App` pointer lives in a thread-local
    slot instead of the window user data.
- `src/dialogs/about.rs` was rewritten:
  - the hand-built `DLGTEMPLATE`, `about_proc`, `WM_CTLCOLORSTATIC`, and manual `CreateWindowExW`
    control creation are gone;
  - the dialog is a `gui::WindowModal` with winsafe `Label` and `Button` controls;
  - the embedded PNG is decoded with the pure-Rust `png` crate and painted with GDI, because winsafe
    has no image control.
- `App::on_command` and `App::show_about` take the owning `gui::WindowMain`; `about::show` takes that
  parent instead of a raw `HWND`.
- `app.rc` and `build.rs` now reference `../csharp-winforms/Resources/` instead of the old `../src/`
  layout.

## Verification

Run from the repository root (the Rust crates are one Cargo workspace):

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p micro-task-pager-winsafe
cargo test -p micro-task-pager-core
cargo build --release -p micro-task-pager-winsafe
```

Current results (2026-11-09): formatting clean, clippy clean with `-D warnings`, the winsafe package
and the shared core package pass all tests, and the release executable is 1,111,552 bytes
(1.06 MiB).

### Shared platform crate (2026-11-10)

The modules that do not depend on the UI toolkit moved into `crates/micro-task-pager-core`:
`com`, `desktop`, `windows` (window enumeration and icon handle only), `settings`, `logging`,
`single_instance`, `monitors`, `foreground`, `dialogs::placement`, `dialogs::settings_draft`, and the
`probe` example. `rust-winsafe` and `rust-winui` now depend on the core crate and re-export its
modules, so application code keeps using paths such as `crate::settings` and `crate::windows`. The
GDI renderer, winsafe window host, About dialog, `.rc` Settings dialog, and winsafe-specific icon
drawing stay in this crate. See `crates/micro-task-pager-core/ARCHITECTURE.md` for the shared contract
and `crates/micro-task-pager-core/README.md` for the boundary rules.

### Follow-up fixes after the first interaction test (2026-11-09)

The first manual run confirmed the pager and About dialog work, and exposed three visual issues:

- **Desktop/window borders were one pixel short.** GDI treats the right and bottom coordinates of
  `Rectangle` as exclusive, while the C# reference uses GDI+ (`DrawRectangle` path includes the far
  edge) and the layout callers pass the C#-style path. `GdiPainter::stroke_rect` now adds one pixel
  to the right and bottom before calling GDI `Rectangle`. This fixes the one-pixel-short active
  desktop border and the one-pixel fill leak along the right and bottom of preview windows that
  looked like a soft edge.
- **About labels had a gray background on the white dialog.** The modal now uses the `WINDOW` class
  brush, and `WM_CTLCOLORSTATIC` forces black text, a transparent background mode, and the white
  window brush for static labels.
- **The logo loader dominated the About module.** The PNG is now embedded with `include_bytes!` and decoded
  with the pure-Rust `png` crate instead of WIC/COM. The decoded RGBA is converted to premultiplied
  BGRA and downscaled once with an area-average filter to the current DPI size, then blended 1:1 into
  the dialog. The box filter replaced `AlphaBlend`'s jagged nearest-neighbour stretching, which had
  the same soft-edge problem as the reactor SVG avatar. This removed the WIC object plumbing, the
  RCDATA resource, and the logo `build.rs` entry. The `png` dependency adds about 110 KiB to the
  release executable.
- **The context menu had no icons.** It now uses the same leading glyphs as the C# and `rust-winui`
  menus (`↔`, `⌖`, `ℹ`, `✕`) and `MNS_NOCHECK` to remove the empty check column.
- **Idle CPU: the desktop list is now fetched once per snapshot.** The baseline called
  `index_of_window` for every window; that method re-enumerated all desktops through COM each time.
  The snapshot already fetched `desktop_ids` for its own state, so this repeated the expensive
  enumeration once per window. The desktop API now exposes:
  - `current_desktop_id()` so the snapshot can resolve the current index from the list it already
    has, instead of calling `current_index()`, which enumerated the list again;
  - `window_desktop_id()` returning either `WindowDesktopId::Desktop(GUID)` or
    `WindowDesktopId::AllDesktops` from a single `GetWindowDesktopId` call.

  `WindowSnapshotter` fetches `desktop_ids` once, resolves the current index, and maps each window
  against that cached list. Pinned windows still follow the current desktop. This matches the
  optimization that cut idle CPU by 2.5x in `rust-winui` and 3.8x in C#. A unit test covers the
  cached-list resolution (`desktop_index_resolves_against_the_fetched_list`).

### Measured results (2026-11-09)

Runtime and deployment figures only; source-line counts are intentionally omitted for this comparison.

| Metric | rust-winsafe | Reference |
|---|---|---|
| Executable | 1,111,552 bytes (1.06 MiB) | C# 1.49 MiB in 13 files; WinUI 3.37 MiB in 1 file |
| Runtime prerequisite | None beyond Windows (static MSVC runtime) | C# .NET Framework 4.8 (inbox); WinUI Windows App Runtime 2.4+ (~111-119 MiB) |
| Idle CPU, one core | 3.7-4.1 % | C# 7.2 %; WinUI 5.9 % |
| Idle CPU, total system | 0.155-0.170 % of 24 logical CPUs | C# 0.30 %; WinUI 0.25 % |
| Working set | 23.1 MiB | C# 59-62 MiB; WinUI 99 MiB |
| Private bytes | 3.1 MiB | C# 33-37 MiB; WinUI 63-64 MiB |
| Handles / threads | 198 / 8 | C# ~397 / 18; WinUI ~1 736 / 65-66 |
| GDI / USER objects | 10 / 16 | C# 24 / 26; WinUI 9 / 35-36 |
| Startup | 144 ms (one warm run) | C# 220 ms warm; WinUI 236 ms warm |

The idle-CPU figures are 60 s process-CPU averages on the same machine and window set used in
`../COMPARISON.md`: one local run measured 3.72 % / 0.155 %, and an earlier user run measured
4.09 % / 0.170 %. Startup is a single 25 ms-resolution run with a warm file cache. Working set and
private bytes are the 20 s steady-state samples. Source-line counts are intentionally omitted.

### Settings dialog implementation (2026-11-09)

- `settings.rc` defines the fixed `DIALOGEX` main dialog and three page dialogs (Layout, Theme,
  Misc); `settings_ids.h` holds the shared control IDs, and `app.rc` includes the resource.
- `src/dialogs/settings/mod.rs` loads the dialog with `gui::WindowModal::new_dlg`, turns the three
  pages into `gui::TabPage::new_dlg` children of a `gui::Tab`, and wraps every control with the
  matching `new_dlg` constructor. Tab switching and page resizing are handled by winsafe.
- The pure settings working copy and dialog placement model live in
  `crates/micro-task-pager-core/src/dialogs/` (`settings_draft.rs`, `placement.rs`) with their tests,
  shared with `rust-winui`; `Settings::save()` and `DesktopBorderStyle::as_u32()` are in
  `settings.rs` in the same core crate.
- Save/Cancel/Defaults use `IDOK`/`IDCANCEL`; Escape uses `wm_command_acc_menu(co::DLGID::CANCEL)`.
  Per-row revert buttons start hidden and appear when the option differs from the opening value.
- The fixed template is sized to the C# dialog's 520x570 client area (297x304 dialog units for
  Segoe UI 9pt). Revert buttons keep `R` in the template as a fallback and get the `↺` glyph in
  `wm_init_dialog` with `SetWindowText`, because the RC compiler in this build does not reliably
  encode Unicode literals and the page controls do not exist before the dialog is created. Color
  rows pair a separate owner-drawn preview with a `Choose...` button; `WM_DRAWITEM` fills only the
  preview swatch and draws a border, converting the stored ARGB value to `COLORREF`'s BGR order. The
  opacity trackbar uses `TBS_AUTOTICKS` with `TBM_SETTICFREQ` 5 to match the C# slider's tick
  spacing; numeric edits have `msctls_updown32` spinners using `UDS_AUTOBUDDY` + `UDS_SETBUDDYINT`
  and ranges from the draft limits.
- Live preview: every edit, combo, checkbox, color, revert and Defaults action applies the working
  copy to the pager without saving. Visual settings (layout, colors, opacity) update immediately;
  timer and taskbar changes still take effect only on Save. Cancel restores the opening settings.
- On Save the pager updates timers, layered opacity, taskbar style/pinning, layout, and persists the
  settings. Cancel leaves the running settings untouched.
- Help texts mirror the C# dialog: label tooltips for View Mode Default, Pager Height, both icon
  paddings, and both update delays, plus the Defaults button and every per-row revert button.
  winsafe 0.0.29 has no tooltip wrapper, so `src/dialogs/settings/tooltips.rs` creates a native
  `tooltips_class32` control with the raw `windows` crate and registers each control by id.
- The dialog is topmost and placed next to the pager with the reused C# placement algorithm.
- The dialog is fixed-size and does not resize.

Interaction verification is otherwise still pending. The checklist that needs a real desktop session:

- borderless topmost pager appears without stealing focus;
- remembered position is restored;
- layered-window opacity follows the setting;
- click switches desktops and focuses windows;
- pager drag and window drag between desktops work;
- Shift+wheel toggles the view mode;
- context menu commands work, with the glyph labels and no check column;
- About opens, shows a sharp logo on a white background, closes with the button and Escape, and does
  not leave the pager disabled;
- Settings opens next to the pager; Layout/Theme/Misc switch correctly; values match the registry;
  live preview updates layout/colors/opacity while editing; Save applies timers, opacity,
  taskbar/pinning and layout; Cancel restores the opening state; Defaults resets only the active
  page; color swatches, the color picker, spinners, and per-row revert buttons work;
- taskbar pinning / all-desktop pinning still works when `ShowAppInTaskbar` is enabled.

## Known gaps and next steps

1. **Settings dialog polish.** Live preview, color swatches, and `UpDown` spinners are implemented.
   Remaining check: the fixed `.rc` dialog at non-100% DPI and with very long localization strings.
2. **Remaining raw window calls.** Timers, `TrackPopupMenu`, opacity, topmost, and layered styles are
   still raw `windows` calls. Move them to winsafe wrappers only where that clearly reduces code.
3. **About polish.** The winsafe labels use the default UI font; the baseline used per-element fonts
   and sizes. Compare against the C# dialog and restore differences if needed.
4. **Platform duplication.** `desktop/`, `windows/`, `monitors`, `settings`, and the renderer are
   copied from the baseline. Do not refactor `rust-winui` to share them until this variant is
   accepted; if both survive, use a workspace with a shared core (`UI-FRAMEWORK-COMPARISON.md` §10
   Option B).
5. **Measurement.** Done: executable size, runtime prerequisite, idle CPU, working set, private bytes,
   handles/threads, GDI/USER objects, and startup are recorded above. Source-line counts are
   intentionally omitted.
6. **Repository wiring.** Done: `build.ps1 -Target winsafe`, VS Code build/run/launch entries, and
   `AGENTS.md`/`README.md`/`THIRD-PARTY-NOTICES.md` are updated.

### Settings dialog decision

The Settings dialog will use a compiled dialog template, not programmatic layout:

- Author `rust-winsafe/settings.rc` with a `DIALOGEX` template (fixed-size `DS_MODALFRAME` dialog,
  no `WS_THICKFRAME`, no resize). `build.rs` compiles it through the existing `embed-resource` step,
  together with `app.rc`.
- Load it with `gui::WindowModal::new_dlg(IDD_SETTINGS)`.
- Attach each control with the matching `new_dlg` constructor
  (`gui::Label::new_dlg`, `gui::Edit::new_dlg`, `gui::ComboBox::new_dlg`, `gui::CheckBox::new_dlg`,
  `gui::TrackBar`/`gui::UpDown` where applicable, `gui::Button::new_dlg`). The dialog manager creates
  the HWNDs; winsafe only wraps them by control ID.
- Use a native `TabControl` with per-tab control visibility for the C# Layout/Theme/Misc pages. A
  dialog template cannot host separate tab-page dialogs cheaply, and the pages are static.
- Use `IDOK`/`IDCANCEL` for Save/Cancel, with `wm_command_acc_menu(co::DLGID::CANCEL)` for Escape.
  Keep the dialog topmost like the C# dialog so the topmost pager cannot cover it.
- Color rows use `ChooseColorW` directly; winsafe has no color-picker wrapper.
- On Save, persist the settings and apply them to the running pager (layout, timers, opacity,
  pinning). On Cancel, keep the previous settings.

The C# reference has three pages: Layout (view mode, pager height, icon padding), Theme (opacity,
desktop border style/colors, preview window colors), and Misc (update delays, taskbar pinning and
the pinning note). The template is authored in dialog units at 96 DPI; the dialog manager scales it.

#### Reuse from `rust-winui`

Only the view/component layer is reactor-specific. The tested model logic comes over almost verbatim:

| `rust-winui` item | Reuse | Notes |
|---|---|---|
| `dialogs/settings/draft.rs` | Whole module | `SettingsDraft`, `Field`, `Section`, clamping, revert, reset, modified checks, and 6 tests |
| `dialogs/placement.rs` | Whole module | C# `FindPlacement` algorithm (right/left/below/above/center) and 6 tests |
| `settings.rs` additions | `Settings::save()` and `DesktopBorderStyle::as_u32()` | The winsafe `settings.rs` is otherwise identical; the diff is only these 46 lines |
| `settings.rs` limits/fields | Already in sync | Same registry format and field set |

Not reusable: `dialogs/settings/view.rs` (reactor view and `Color` picker), the `Component` lifecycle
and message plumbing in `dialogs/settings/mod.rs`, `SettingsInput`/`DialogLink`, and
`ContentDialogResult`. The winsafe version replaces those with `new_dlg` controls, `ChooseColorW`,
and a modal `WindowModal`; the draft, placement, and persistence logic stay the same.

## Open questions

- Is the native GDI pager crisp enough at 125%/150%/200% scaling, or does it need Direct2D later?
- Does the winsafe About dialog need the exact fonts and spacing of the C# dialog, or is approximate
  parity acceptable?
- Should this variant eventually replace `rust-winui`, or remain an optional no-runtime build?
