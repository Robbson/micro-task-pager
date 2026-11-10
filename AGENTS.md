# AGENTS.md

Instructions for AI agents working in this repository.

## Project

Micro Task Pager is a Windows-only virtual desktop pager. The repository contains:

- `csharp-winforms/` - the C#/.NET Framework 4.8 Windows Forms application. It has the latest
  features, but it is less tested in production than `rust-winsafe`; some features may be
  experimental and could change or be removed.
- `rust-winsafe/` - the stable implementation: native Win32 through `winsafe`, with no Windows App
  Runtime or reactor dependency. It has fewer features than `csharp-winforms`, and new features are
  added only after a delay. Read `rust-winsafe/MIGRATION.md`.
- `rust-winui/` - the experimental Rust implementation. It currently matches the `rust-winsafe`
  feature set but is not planned to be updated in the near future because `windows-reactor` is not
  stable yet; it also uses considerably more resources and a large runtime dependency. It uses
  `windows-reactor` for all UI. Read `rust-winui/ARCHITECTURE.md` and `rust-winui/MIGRATION.md`
  before making structural changes.
- `crates/micro-task-pager-core/` - the shared Rust platform crate. It contains everything that does
  not depend on the UI toolkit: COM bootstrap, virtual desktops, window enumeration, settings,
  logging, single instance, monitor/foreground tracking, and the pure dialog helpers. Both Rust front
  ends consume it. Read `crates/micro-task-pager-core/README.md` before changing it or adding a new
  Rust implementation.

The two Rust front ends and `micro-task-pager-core` form one Cargo workspace rooted at `Cargo.toml`.
Shared platform behavior belongs in the core crate, not duplicated per front end.

All three use undocumented Windows virtual-desktop COM interfaces, so changes that touch virtual
desktop interop must be handled carefully and tested on real Windows builds.

For `rust-winui`, new UI work belongs in reactor components. Do not reintroduce a native Win32 window,
GDI renderer, or native dialog runtime there. The separate `rust-winsafe` crate deliberately uses
winsafe controls, a GDI renderer, and native dialog templates; keep that work inside `rust-winsafe`
and do not mix the two UI stacks.

## Tooling available to agents

Use PowerShell 7.4 (`pwsh`) syntax for shell commands. `powershell.exe` is Windows PowerShell 5.1, so
call `pwsh` explicitly when 7.4 behavior is required. Do not assume a POSIX shell: commands like
`head`, `wc`, `rm -rf`, or `sed` may not exist. Prefer the agent's file tools (`read`, `write`, `edit`,
`glob`, `grep`) over shell equivalents for file operations.

`winget` is available for installing additional tools, but do not install software or change
persistent `PATH` settings without explicit user confirmation.

Installed command-line tools:

| Tool       | Version       | Command          | Notes                                                                                                                                  |
| ---------- | ------------- | ---------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| PowerShell | 7.4.22        | `pwsh`           | Use PowerShell syntax and cmdlets (`Get-ChildItem`, `Select-Object`, `Get-Content`).                                                   |
| ripgrep    | 15.2.0        | `rg`             | Preferred content search. Supports PCRE2 (`rg --pcre2`); the default engine has no lookaround. Use instead of `Select-String`.         |
| fd         | 8.3.2         | `fd`             | Fast file-name search. Use instead of recursive `Get-ChildItem -Filter`. The winget 10.5.0 copy is currently shadowed; see PATH notes. |
| jq         | 1.8.2         | `jq`             | JSON processing.                                                                                                                       |
| 7-Zip      | 26.05         | `7z`             | Archive inspection and creation.                                                                                                       |
| WinDbg     | 1.2610.1001.0 | `WinDbgX`        | Microsoft Store build, app execution alias. See Debugging below.                                                                       |
| Rust       | 1.99.1        | `rustc`, `cargo` | MSVC toolchain.                                                                                                                        |
| rustfmt    | 1.10.0        | `cargo fmt`      | Formatting.                                                                                                                            |
| Clippy     | 0.1.99        | `cargo clippy`   | Linting.                                                                                                                               |
| .NET SDK   | 8.0.427       | `dotnet`         | Builds the `net48` C# project.                                                                                                         |

### PATH notes

- `7z`, `jq`, `rg`, and `fd` are on `PATH`.
- `rg` resolves to the winget ripgrep 15.2.0.
- `fd` still resolves to Neovim's 8.3.2 because `E:\Dev\Tools\Neovim\bin` comes before the winget
  package directory. The winget fd 10.5.0 is later on `PATH`. Use `Get-Command fd` to confirm which
  binary is selected. To use 10.5.0, remove the Neovim directory from `PATH` or remove its `fd.exe`
  copy; do not change `PATH` without asking the user.
- Do not hardcode user-specific paths in repository files. If a tool is needed repeatedly, ask before
  changing persistent `PATH` settings.

### Debugging

- WinDbg (Store) is available as `WinDbgX` through the app execution alias in
  `%LOCALAPPDATA%\Microsoft\WindowsApps`. It is GUI-first, so do not launch it from automated commands.
- There is no `windbg.exe`, `cdb.exe`, or `kd.exe` on `PATH`. For command-line debugging, install the
  Windows SDK "Debugging Tools for Windows" separately, with explicit user confirmation.
- Rust MSVC builds write PDBs next to the executable; use `RUST_BACKTRACE=1` when diagnosing panics.

## Build and verification

### Production releases

Build one or more production releases from the repository root with PowerShell 7:

```powershell
pwsh ./build.ps1                    # all three implementations
pwsh ./build.ps1 -Target winforms   # C# WinForms only
pwsh ./build.ps1 -Target winui      # Rust WinUI 3 only
pwsh ./build.ps1 -Target winsafe    # native winsafe only
pwsh ./build.ps1 -Target all -NoZip # stage all without ZIP archives
```

Outputs are written to `prod/winforms/`, `prod/winui/`, and `prod/winsafe/`, each with a versioned
`MicroTaskPager-<target>-v<version>.zip`. The C# build increments `csharp-winforms/version.txt`; the
Rust archives use the version from their `Cargo.toml` (`rust-winui` or `rust-winsafe`).

### Rust workspace

The Rust crates are one Cargo workspace:

| Directory | Package | Library crate | Binary |
| --- | --- | --- | --- |
| `crates/micro-task-pager-core/` | `micro-task-pager-core` | `micro_task_pager_core` | none |
| `rust-winsafe/` | `micro-task-pager-winsafe` | `micro_task_pager_winsafe` | `micro-task-pager-winsafe` |
| `rust-winui/` | `micro-task-pager-winui` | `micro_task_pager_winui` | `micro-task-pager-winui` |

Shared build configuration lives at the repository root:

- `Cargo.toml` - workspace membership and the release profile. Profiles are only valid here.
- `.cargo/config.toml` - static MSVC runtime for x64 and ARM64.
- `rust-toolchain.toml` - the stable toolchain and its components.
- `target/` - one shared output directory for every member.
- `Cargo.lock` - one shared lockfile.

Run workspace-wide commands from the repository root:

```powershell
cargo check --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

Package-scoped commands use `-p`, for example `cargo build -p micro-task-pager-winsafe`. Running
`cargo` inside a member directory also works and selects that package; output still lands in the root
`target/`. The binaries are staged and renamed to `MicroTaskPager.exe` by `build.ps1`.

When adding a platform module, put it in `micro-task-pager-core` if it is UI-toolkit independent. If
it needs the front end's renderer or widget types, keep it in that front end. A new Rust front end
adds one path dependency on `micro-task-pager-core` and supplies its own UI.

### Rust WinUI 3 (experimental)

Run the package-scoped commands below from the repository root, or `cd rust-winui` and omit `-p`.

```powershell
cargo build -p micro-task-pager-winui
cargo build -p micro-task-pager-winui --release
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p micro-task-pager-winui
```

- The first build requires the MSVC build tools (`link.exe`) and a Windows SDK, including `rc.exe` for
  the icon/version resources and the manifest.
- `.cargo/config.toml` at the repository root enables the static MSVC runtime. Cargo discovers it from
  any directory in the workspace, so the release executable has no Visual C++ redistributable
  dependency.
- The release executable is `target/release/micro-task-pager-winui.exe`. A deployment folder may rename
  it to `MicroTaskPager.exe`; the embedded icon/version/manifest stay intact.
- Add unit tests next to the modules they cover. There is no separate test crate.
- `rust-toolchain.toml` at the repository root selects `stable`, which is 1.99.1. The exact `1.99.1`
  rustup toolchain is installed without `cargo.exe`; see `rust-winui/MIGRATION.md` open questions.
- `cargo run -p micro-task-pager-winui` builds and starts the WinUI 3 pager GUI. `build.rs` compiles
  `app.rc` (icon and version) and embeds `app.manifest` through the MSVC linker flags.
- `windows-reactor` 0.100 is a normal dependency. The pager, About, and Settings are reactor
  components; `rust-winui/src/main.rs` starts the reactor pager.
- WinUI 3 requires the Windows App Runtime 2.x framework package (2.4 or later). Release packaging is
  framework-dependent; the executable does not bundle the runtime. It is installed on this machine
  (2.6.5); the reactor examples are the quickest smoke test:

  ```powershell
  cargo run -p micro-task-pager-winui --example reactor_about
  cargo run -p micro-task-pager-winui --example reactor_settings
  cargo run -p micro-task-pager-winui --example reactor_pager
  ```

  `reactor_pager` starts the same component as the main binary. Stop any running pager first: both
  use the release mutex and would otherwise fight over pinning and topmost.

- The app requires Windows 10 1809 (build 17763) or later because of the Windows App Runtime. The
  virtual-desktop backend still supports build 10240 and later, but the WinUI app cannot run below
  17763.
- The app manifest declares `PerMonitorV2` DPI awareness, which WinUI 3 requires. The pager rebuilds its
  pixel grid and icon cache when the window DPI changes; monitor rectangles stay in physical pixels.
- The WinUI pager renders both view modes and supports click-to-switch, click-to-focus, pager
  dragging, window dragging between desktops, Shift+wheel mode toggle, and the context menu with
  Settings and About. It also pins itself to all desktops when `ShowAppInTaskbar` is enabled,
  re-shows when covered, and restores the last active window per desktop after a switch.
- The virtual-desktop backend supports build 10240 and later. Builds below 22000 use the Win10 layout
  (covering Win10 and Win10LTSC); build 22000 and later use the Win11 layout. Older builds return
  `ApiError::NotSupported`.
- Verification on Windows 10, Win10LTSC, and Windows 11 21H2/22H2 is postponed; do not block work on
  obtaining those images. See the Postponed section in `rust-winui/MIGRATION.md`.
- `rust-winui/MIGRATION.md` also contains a code map and glossary for the module structure and terms such as
  `SupervisedDesktop`, layout, and probe.
- Logs are written to `%LOCALAPPDATA%\MicroTaskPager\logs` with 14-day retention.
- COM verification uses the standalone probe from the core crate:

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

  Run the read-only commands first. `move`, `pin`, and `unpin` change real window or shell state. Use
  `window` to create a probe-owned throwaway window instead of moving a real application.

- Do not declare work complete while `cargo fmt --check` or `cargo clippy -D warnings` fails.
- Do not run the GUI while the C# application is running: the implementations pin themselves to all
  desktops and fight over topmost. The release mutex prevents this; a development build may use a
  different mutex for side-by-side comparison.

### Rust winsafe (native Win32, stable)

Run the package-scoped commands below from the repository root, or `cd rust-winsafe` and omit `-p`.

```powershell
cargo build -p micro-task-pager-winsafe
cargo build -p micro-task-pager-winsafe --release
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p micro-task-pager-winsafe
```

- The crate is reactor-free and does not require the Windows App Runtime. It links only OS DLLs plus
  the static MSVC runtime.
- `winsafe` is pinned at `0.0.29`. The raw `windows` crate stays in use for GDI, COM, timers, menus,
  layered windows, and the color picker.
- The pager window is a winsafe `gui::WindowMain`; About is a `gui::WindowModal`; Settings is a fixed
  `.rc` dialog loaded with `new_dlg`. `app.rc` includes `settings.rc`, and `settings_ids.h` must stay in
  sync with `src/dialogs/settings/ids.rs`.
- `dialogs::placement` and `dialogs::settings_draft` come from `micro-task-pager-core`. Keep the
  behavior and tests compatible with `rust-winui` when changing them.
- The release executable is `target/release/micro-task-pager-winsafe.exe`.
- Do not declare work complete while fmt, clippy with `-D warnings`, or tests fail.
- Do not run the winsafe build while another pager is running; all implementations share the
  single-instance mutex and the topmost/pinning behavior.

### C# (latest features)

```powershell
dotnet build csharp-winforms/MicroTaskPager.csproj -c Debug
```

- The project has no automated test project. Verify changes by building and, where relevant, by
  manually testing the pager UI.
- The Debug executable is written to `csharp-winforms/bin/Debug/net48/MicroTaskPager.exe`.
- Do not commit unless the user explicitly asks.

## Generated files: ignore them

The following files are generated automatically by the build:

- `csharp-winforms/version.txt`
- `csharp-winforms/src/GeneratedVersion.cs`

`csharp-winforms/MicroTaskPager.csproj` runs `csharp-winforms/IncrementVersion.ps1` before every build. The script increments
the fourth version component (`Major.Minor.Patch.Build`) and rewrites both files.

Guidance:

- Do not edit these files manually.
- Do not revert or "fix" changes to them just because a build changed the version number.
- An increasing build number is expected and is not a problem.
- Ignore them when reviewing diffs or looking for unintended changes.
- Do not stage them for a commit unless the user explicitly asks for that.
- If a working tree only differs in these files, no action is required.

## Repository layout

- `csharp-winforms/src/` - C# source root (`Program.cs`, `Config`, `Core`, `GUI`).
- `csharp-winforms/src/Core/API/` - virtual-desktop COM interop. There are four platform-specific variants:
  `Win10`, `Win10LTSC`, `Win11`, and `Win11_`. The current selector uses `Win11_` for build 22621 and
  later; `rust-winui/ARCHITECTURE.md` §7.3 flags that the real boundary still needs verification.
  Keep shared changes in sync across all four files.
- `csharp-winforms/src/Core/` - window enumeration, types, DPI helpers.
- `csharp-winforms/src/GUI/` - pager, desktops, settings dialog, about dialog.
- `csharp-winforms/src/Config/` - settings model, registry storage, saved pager position.
- `csharp-winforms/Resources/` - icon and logo assets shared with the Rust resource files.
- `Cargo.toml`, `.cargo/config.toml`, `rust-toolchain.toml` - workspace membership, static-runtime
  flags, and the toolchain.
- `crates/micro-task-pager-core/` - shared Rust platform crate. See its `README.md`.
- `crates/micro-task-pager-core/src/desktop/` - safe `VirtualDesktopApi`, the Win10/Win11 COM layout
  backends, and the restart supervisor.
- `crates/micro-task-pager-core/src/windows/` - window enumeration, filtering, snapshot model, and the
  icon-handle cache.
- `crates/micro-task-pager-core/src/dialogs/` - pure `placement` and `settings_draft` helpers.
- `crates/micro-task-pager-core/examples/probe/` - the shared virtual-desktop COM probe.
- `rust-winui/ARCHITECTURE.md` - migration architecture, WinUI 3 direction, and behavioral contract.
- `rust-winui/MIGRATION.md` - phase status, verification log, and open questions.
- `rust-winui/src/` - WinUI 3 implementation (pager component and dialogs). Platform modules are
  re-exported from `micro-task-pager-core`.
- `rust-winui/src/pager/` - reactor pager component, view, layout, input, and WinUI icon conversion.
- `rust-winui/src/dialogs/` - reactor About and Settings windows.
- `rust-winui/src/native_window.rs` - HWND/pinning/topmost/opacity shim for the reactor pager.
- `rust-winui/app.rc`, `rust-winui/app.manifest`, `rust-winui/build.rs` - icon, version resource, and manifest.
- `rust-winui/examples/reactor_about.rs` - reactor About reference and regression sample.
- `rust-winui/examples/reactor_settings.rs` - standalone Settings window runner.
- `rust-winui/examples/reactor_pager.rs` - the same reactor pager runner as the main binary.
- `rust-winsafe/MIGRATION.md` - winsafe variant status, source commit, verification log, and gaps.
- `rust-winsafe/src/` - native implementation (app coordinator, GDI renderer, winsafe window host).
- `rust-winsafe/src/render/` - `layout.rs` geometry and hit testing, `paint.rs` drawing instructions,
  and `gdi.rs` backend.
- `rust-winsafe/src/dialogs/` - winsafe About and `.rc`-backed Settings dialog.
- `rust-winsafe/app.rc`, `rust-winsafe/settings.rc`, `rust-winsafe/settings_ids.h` - icon/version,
  fixed Settings dialog templates, and the shared control IDs.

## Notes

- Virtual-desktop COM interfaces are undocumented and version-sensitive. Test interop changes on the
  supported Windows versions rather than assuming one layout.
- The desktop API is apartment-threaded. Initialize `micro_task_pager_core::com::Apartment` on the
  thread that will use it and keep all `desktop` calls on that thread. In `rust-winui` that thread is
  the reactor UI thread; in `rust-winsafe` it is the winsafe UI thread.
- Windows belonging to the pager process are intentionally excluded from the pager.
- The Rust implementation must preserve the behavior described in `rust-winui/ARCHITECTURE.md` §11.
  Intentional deviations from the C# implementation are listed in §12 there.
- Shared platform behavior goes in `micro-task-pager-core`, never copied into a front end. A new
  front end (for example another UI toolkit) depends on the core crate and re-exports its modules if
  it wants the existing `crate::desktop` / `crate::settings` style paths.
- **For `rust-winui`, use WinUI 3 through `windows-reactor` for all UI work.** Do not build a custom UI
  framework, widget toolkit, dialog runtime, or drawing abstraction on Win32 there. `winsafe`, `egui`,
  and other GUI frameworks are not used in `rust-winui`.
- `rust-winsafe` is a separate native implementation built on `winsafe`. Keep its winsafe controls,
  GDI renderer, and `.rc` dialogs inside that crate; do not mix the two UI stacks.
- For `rust-winui`, do not add DWM thumbnails, a custom Direct2D/GDI renderer, or ARM64 support unless
  the user explicitly asks. WinUI composition replaces the custom renderer. The winsafe crate already
  uses a GDI renderer by design.
- Keep `unsafe` confined to platform modules and document each block. In the core crate that includes
  `desktop/com`, `windows`, `monitors`, `foreground`, `single_instance`, `logging`, and the COM
  bootstrap; the supervisor, settings, and dialog helpers stay safe Rust. In `rust-winui` that includes
  `native_window`, the WinUI icon conversion, and the COM bootstrap in `main.rs`; components, layout,
  hit testing, and dialog state stay safe Rust. In `rust-winsafe`, keep drawing, Win32, and dialog
  interop in the platform modules and dialogs.
- The context menu is a native `TrackPopupMenu` shim (`TrackPopupMenu` is modal): if an automated
  test sends `WM_RBUTTONUP`, the pager stops its timers until the menu is dismissed. Use a normal
  mouse click to choose an item, or click outside/close the menu afterward. `windows-reactor` 0.100
  has no right-click context menu API.
- Keep the C# and Rust settings registry values compatible while multiple implementations exist.
- The Windows App Runtime 2.x framework package (2.4 or later) is a prerequisite for the WinUI target.
  Packaging is framework-dependent; the executable does not bundle the runtime.
