# rust-winsafe - Migration Log

Status: experimental third implementation. It is a native Win32 pager built with
[`winsafe`](https://crates.io/crates/winsafe) and has **no Windows App Runtime / WinUI dependency**.
The shipping implementation remains `rust-winui`; `csharp-winforms` remains the behavioral reference.

This document tracks the winsafe variant only. For the WinUI migration see
[`../rust-winui/MIGRATION.md`](../rust-winui/MIGRATION.md); for the framework trade-offs see
[`../UI-FRAMEWORK-COMPARISON.md`](../UI-FRAMEWORK-COMPARISON.md).

## Baseline

The crate was copied from commit `deed611295547cb4d7baeb3766ff3c882c5191ac` (`rust/`), the last
pure-Win32 Rust implementation before the reactor migration. That baseline already had a working
pager, GDI renderer, virtual-desktop COM layer, settings storage, About dialog, and 18 unit tests, but
built the window class and dialog by hand.

The copy is deliberate: platform modules (`desktop/`, `windows/`, `monitors`, `foreground`,
`settings`, `com`, `single_instance`, `logging`) are kept identical to the baseline for now. They are
not refactored into UI-independent modules, so `rust-winui` is untouched.

## Status

| Area | Status | Notes |
|---|---|---|
| Crate scaffold, resources, manifest | Done | Resource paths point at `../csharp-winforms/Resources/` |
| Virtual-desktop COM, snapshot, settings | Done (copied) | Unchanged from the baseline commit |
| Pager window host | **Migrated to winsafe** | `gui::WindowMain` + typed events, no `WNDCLASSW`/`WndProc`/manual message loop |
| Pager rendering | Copied | Raw GDI `Painter`; winsafe adds no drawing value here |
| Context menu | Copied | Raw `TrackPopupMenu` shim |
| About dialog | **Migrated to winsafe** | `gui::WindowModal` + `Label` + `Button`; WIC/GDI logo paint retained |
| Settings dialog | Not implemented | Was a non-goal in the baseline; planned as a later step |
| Build pipeline (`build.ps1`, CI, VS Code) | Not wired | Only Cargo commands are verified so far |

## What changed versus the baseline

- Added `winsafe = 0.0.29` (pinned; the crate is `0.0.x` and its API may change). The raw `windows`
  crate is retained for GDI, WIC, COM, shell helpers, timers, menus, and layered-window calls.
- Removed the `windows-reactor` dev-dependency and `examples/reactor_about.rs`. This crate is
  reactor-free and does not require the Windows App Runtime.
- `src/window.rs` was rewritten:
  - winsafe creates and owns the main window and message loop;
  - input is handled by typed `wm_l_button_down`, `wm_mouse_move`, `wm_mouse_wheel`,
    `wm_r_button_up`, `wm_timer`, `wm_display_change`, and `wm_capture_changed` events;
  - winsafe stores its own state in `GWLP_USERDATA`, so the `App` pointer lives in a thread-local
    slot instead of the window user data.
- `src/about.rs` was rewritten:
  - the hand-built `DLGTEMPLATE`, `about_proc`, `WM_CTLCOLORSTATIC`, and manual `CreateWindowExW`
    control creation are gone;
  - the dialog is a `gui::WindowModal` with winsafe `Label` and `Button` controls;
  - the embedded PNG logo is still decoded with WIC and drawn in `wm_paint`, because winsafe has no
    image control.
- `App::on_command` and `App::show_about` take the owning `gui::WindowMain`; `about::show` takes that
  parent instead of a raw `HWND`.
- `app.rc` and `build.rs` now reference `../csharp-winforms/Resources/` instead of the old `../src/`
  layout.

## Verification

Run from `rust-winsafe/`:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```

Current results (2026-11-09): formatting clean, clippy clean with `-D warnings`, 21 unit tests
pass, and the release executable is 976,384 bytes (0.93 MiB).

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
- **The logo loader dominated `about.rs`.** The PNG is now embedded with `include_bytes!` and decoded
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
- taskbar pinning / all-desktop pinning still works when `ShowAppInTaskbar` is enabled.

## Known gaps and next steps

1. **Settings dialog.** Implement it later with winsafe `Tab` pages and native controls. winsafe has
   no layout engine, so rows need explicit positions or a small helper, similar to the C# code.
2. **Remaining raw window calls.** Timers, `TrackPopupMenu`, opacity, topmost, and layered styles are
   still raw `windows` calls. Move them to winsafe wrappers only where that clearly reduces code.
3. **About polish.** The winsafe labels use the default UI font; the baseline used per-element fonts
   and sizes. Compare against the C# dialog and restore differences if needed.
4. **Platform duplication.** `desktop/`, `windows/`, `monitors`, `settings`, and the renderer are
   copied from the baseline. Do not refactor `rust-winui` to share them until this variant is
   accepted; if both survive, use a workspace with a shared core (`UI-FRAMEWORK-COMPARISON.md` §10
   Option B).
5. **Measurement.** Compare executable size, idle working set/private bytes, handles/threads, startup,
   and source lines against `../COMPARISON.md`.
6. **Repository wiring.** Add `rust-winsafe` to `build.ps1`, the VS Code tasks, and `AGENTS.md` only
   after the interaction checklist passes.

## Open questions

- Is the native GDI pager crisp enough at 125%/150%/200% scaling, or does it need Direct2D later?
- Does the winsafe About dialog need the exact fonts and spacing of the C# dialog, or is approximate
  parity acceptable?
- Should the settings dialog be built programmatically with winsafe controls, or from a `.res` dialog
  template produced by a resource editor?
- Should this variant eventually replace `rust-winui`, or remain an optional no-runtime build?
