# micro-task-pager-core

Shared Windows platform layer for every Rust Micro Task Pager front end.

The crate is deliberately **UI-toolkit independent**. It knows about COM, virtual
desktops, Win32 window enumeration, the registry, monitors, and logging, but it
does not know about `winsafe`, `windows-reactor`, GDI drawing, or WinUI. Rendering,
input handling, widgets, and dialog windows belong to a front end.

```text
crates/micro-task-pager-core   shared platform code (this crate)
rust-winsafe                   native Win32 front end (winsafe + GDI)
rust-winui                     WinUI 3 front end (windows-reactor)
```

The shared behavior contract, COM/threading rules, settings format, and the parity checklist are in
[`ARCHITECTURE.md`](ARCHITECTURE.md). This README is the quick module map.

## Workspace

The three crates form one Cargo workspace rooted at the repository `Cargo.toml`.
Shared build configuration lives there too:

| File | Purpose |
| --- | --- |
| `Cargo.toml` | Workspace membership and the release profile. |
| `.cargo/config.toml` | Static MSVC runtime (no Visual C++ redistributable). |
| `rust-toolchain.toml` | Stable toolchain plus `rustfmt` and `clippy`. |
| `target/` | One output directory for every member. |
| `Cargo.lock` | One lockfile for every member. |

Run workspace-wide commands from the repository root:

```powershell
cargo check --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

Package-scoped commands use `-p`:

```powershell
cargo test -p micro-task-pager-core
cargo run -p micro-task-pager-core --example probe
```

Running `cargo` from a member directory selects that package, and output still
lands in the root `target/`.

## Module map

| Module | What it owns | Notable entry points |
| --- | --- | --- |
| `com` | RAII single-threaded apartment on the UI thread. | `Apartment::init` |
| `desktop` | Safe wrapper over the undocumented virtual-desktop COM interfaces, the Win10/Win11 layouts, and the restart supervisor. | `desktop::open`, `VirtualDesktopApi`, `SupervisedDesktop`, `ApiError` |
| `desktop::com` (private) | Raw vtables, GUIDs, and layout selection. | not public |
| `windows` | Top-level window enumeration, filtering, stable ordering, change hashing, and the per-window icon-handle cache. | `WindowSnapshotter::capture`, `WindowSnapshot`, `WindowInfo`, `WindowIcon` |
| `settings` | C#-compatible registry values and defaults. | `Settings::load`, `Settings::save`, `save_pager_location`, `save_view_modes` |
| `monitors` | Physical monitor enumeration and ordering. | `enumerate`, `primary`, `MonitorInfo` |
| `foreground` | `EVENT_SYSTEM_FOREGROUND` hook and per-desktop last-active tracking. | `ForegroundTracker` |
| `single_instance` | Named-mutex guard shared by all implementations. | `acquire`, `show_already_running`, `Acquisition` |
| `logging` | Small daily file logger in `%LOCALAPPDATA%\MicroTaskPager\logs`. | `init`, `info`, `warn`, `error` |
| `dialogs::placement` | Pure right/left/below/above dialog placement in physical pixels. | `find_placement`, `Rect` |
| `dialogs::settings_draft` | Pure Settings working copy: revert, reset, clamps. | `SettingsDraft`, `Field`, `Section` |

Everything except the COM modules is safe Rust. The `unsafe` blocks are confined
to `desktop/com`, `windows`, `monitors`, `foreground`, `single_instance`, and
`logging`, and each one carries a `SAFETY` comment.

## Threading contract

The virtual-desktop objects are **apartment-threaded**:

1. Create `com::Apartment` on the thread that will use the desktop API.
2. Keep that guard alive for as long as any desktop object exists.
3. Keep every `desktop` call, and every `windows::WindowSnapshotter::capture`
   call, on the same thread.

The types are not `Send`. In `rust-winsafe` the owning thread is the winsafe
message-loop thread; in `rust-winui` it is the `windows-reactor` UI thread.
`foreground::ForegroundTracker` only stores a handle from its WinEvent callback
and must be drained on the owning thread.

## Settings compatibility

`settings` reads and writes `HKEY_CURRENT_USER\SOFTWARE\MicroTaskPager` in the
format the C# implementation uses. Keep that format stable while multiple
implementations coexist:

- `RendererViewModes` is one byte per `(desktop, screen)` in
  `desktop * screen_count + screen` order.
- `BackgroundOpacity` is a decimal string; the reader also accepts a DWORD.
- Missing or malformed values fall back to defaults instead of failing.

Front ends may map the flat view-mode list to their own layout, but they must
write it back as a flat byte array.

## Icons

`WindowInfo::icon` is a borrowed `WindowIcon` wrapper around a shell `HICON`. The
window, its class, or the system owns that handle.

- `WindowIcon::handle()` returns the raw handle for the front end's renderer.
- Do **not** call `DestroyIcon` on it.
- If a front end needs a scaled or converted copy (a GDI bitmap for winsafe, a
  PNG for WinUI), it must make its own copy first.
- `WindowSnapshotter` caches only the handle. Encoding or bitmap conversion
  belongs in the front end, because it is toolkit-specific.

## Adding a new Rust front end

1. Create the crate directory and add it to `members` in the root `Cargo.toml`.
2. Depend on the core crate by path and pick a unique package, library, and
   binary name so the workspace stays collision-free:

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

3. Start the process in the same order the existing front ends use:
   `logging::init()` → `com::Apartment::init()` → `single_instance::acquire()` →
   your UI loop.
4. Re-export the core modules from your `lib.rs` if you want the existing
   `crate::desktop` / `crate::settings` paths:

   ```rust
   pub use micro_task_pager_core::{
       com, desktop, foreground, logging, monitors, settings, single_instance, windows,
   };

   pub mod ui;
   ```

5. Add the pure dialog helpers explicitly if you use them:

   ```rust
   use micro_task_pager_core::dialogs::{placement, settings_draft};
   ```

6. Keep the UI in your crate. Do not add rendering, widgets, or dialog windows
   to the core crate.

### Where does a new module go?

- **Core** if it only needs Win32, COM, the registry, or plain Rust and would be
  useful to a different UI toolkit.
- **Front end** if it needs the toolkit's types, renderer, layout units (DIPs vs
  device pixels), or window handles.

The pager layout is the clearest example: the winsafe layout works in device
pixels with GDI rectangles, while the WinUI layout works in DIPs with
pixel-snapped floating-point geometry. They serve the same behavior contract but
must stay in their own crates. The settings draft and dialog placement are pure
and shared.

## Verification

The core crate has unit tests for the supervisor, layout-selection thresholds,
settings decoding, snapshot hashing, dialog placement, and the settings draft.
Run them with `cargo test -p micro-task-pager-core`.

The standalone COM probe lives in `examples/probe` and is shared by every front
end. It initializes COM itself and exercises the same code path as the pager:

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

Run the read-only commands first. `move`, `pin`, `unpin`, and `switch` change
real window or shell state. Use `window` to create a throwaway probe-owned window
instead of moving a real application.

Virtual-desktop interop is undocumented and build-sensitive. Verify changes on
real Windows 10 and Windows 11 builds rather than assuming one COM layout.