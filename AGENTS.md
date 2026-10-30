# AGENTS.md

Instructions for AI agents working in this repository.

## Project

Micro Task Pager is a Windows-only virtual desktop pager. The repository currently contains two
implementations:

- `src/` - the existing C#/.NET Framework 4.8 Windows Forms application. It is the behavioral
  reference and remains the shipping implementation during the migration.
- `rust/` - the native Rust port. Read `rust/ARCHITECTURE.md` and `rust/MIGRATION.md` before making
  structural changes.

Both use undocumented Windows virtual-desktop COM interfaces, so changes that touch virtual desktop
interop must be handled carefully and tested on real Windows builds.

## Tooling available to agents

Use PowerShell 7.4 (`pwsh`) syntax for shell commands. `powershell.exe` is Windows PowerShell 5.1, so
call `pwsh` explicitly when 7.4 behavior is required. Do not assume a POSIX shell: commands like
`head`, `wc`, `rm -rf`, or `sed` may not exist. Prefer the agent's file tools (`read`, `write`, `edit`,
`glob`, `grep`) over shell equivalents for file operations.

`winget` is available for installing additional tools, but do not install software or change
persistent `PATH` settings without explicit user confirmation.

Installed command-line tools:

| Tool | Version | Command | Notes |
|---|---|---|---|
| PowerShell | 7.4.22 | `pwsh` | Use PowerShell syntax and cmdlets (`Get-ChildItem`, `Select-Object`, `Get-Content`). |
| ripgrep | 15.2.0 | `rg` | Preferred content search. Supports PCRE2 (`rg --pcre2`); the default engine has no lookaround. Use instead of `Select-String`. |
| fd | 8.3.2 | `fd` | Fast file-name search. Use instead of recursive `Get-ChildItem -Filter`. The winget 10.5.0 copy is currently shadowed; see PATH notes. |
| jq | 1.8.2 | `jq` | JSON processing. |
| 7-Zip | 26.05 | `7z` | Archive inspection and creation. |
| WinDbg | 1.2610.1001.0 | `WinDbgX` | Microsoft Store build, app execution alias. See Debugging below. |
| Rust | 1.99.1 | `rustc`, `cargo` | MSVC toolchain. |
| rustfmt | 1.10.0 | `cargo fmt` | Formatting. |
| Clippy | 0.1.99 | `cargo clippy` | Linting. |
| .NET SDK | 8.0.427 | `dotnet` | Builds the `net48` C# project. |

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

### Rust (target implementation)

Run Cargo commands from the `rust/` directory; the repository root is not a Cargo workspace.

```powershell
cargo build
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

- The first build requires the MSVC build tools (`link.exe`) and a Windows SDK.
- Add unit tests next to the modules they cover. There is no separate test crate.
- `rust-toolchain.toml` currently selects `stable`, which is 1.99.1. The exact `1.99.1` rustup
  toolchain is installed without `cargo.exe`; see `rust/MIGRATION.md` open questions.
- The current binary is a scaffold: it loads settings and exits. The Win32 window and message loop
  arrive in Phase 4.
- COM verification is done with the standalone probe once it exists:

  ```powershell
  cargo run --example probe
  ```

- Do not declare work complete while `cargo fmt --check` or `cargo clippy -D warnings` fails.
- Do not run the GUI while the C# application is running: both implementations pin themselves to all
  desktops and fight over topmost. The release mutex prevents this; a development build may use a
  different mutex for side-by-side comparison.

### C# (reference implementation)

```powershell
dotnet build src/MicroTaskPager.csproj -c Debug
```

- The project has no automated test project. Verify changes by building and, where relevant, by
  manually testing the pager UI.
- The Debug executable is written to `src/bin/Debug/net48/MicroTaskPager.exe`.
- Do not commit unless the user explicitly asks.

## Generated files: ignore them

The following files are generated automatically by the build:

- `src/version.txt`
- `src/GeneratedVersion.cs`

`src/MicroTaskPager.csproj` runs `src/IncrementVersion.ps1` before every build. The script increments
the fourth version component (`Major.Minor.Patch.Build`) and rewrites both files.

Guidance:

- Do not edit these files manually.
- Do not revert or "fix" changes to them just because a build changed the version number.
- An increasing build number is expected and is not a problem.
- Ignore them when reviewing diffs or looking for unintended changes.
- Do not stage them for a commit unless the user explicitly asks for that.
- If a working tree only differs in these files, no action is required.

## Repository layout

- `src/Core/API/` - virtual-desktop COM interop. There are four platform-specific variants:
  `Win10`, `Win10LTSC`, `Win11`, and `Win11_`. The current selector uses `Win11_` for build 22621 and
  later; `rust/ARCHITECTURE.md` section 6.3 flags that the real boundary still needs verification.
  Keep shared changes in sync across all four files.
- `src/Core/` - window enumeration, types, DPI helpers.
- `src/GUI/` - pager, desktops, settings dialog, about dialog.
- `src/Config/` - settings model, registry storage, saved pager position.
- `rust/ARCHITECTURE.md` - migration architecture and behavioral contract.
- `rust/MIGRATION.md` - phase status, verification log, and open questions.
- `rust/src/` - Rust implementation (scaffold: settings, COM apartment, single instance).
- `rust/examples/probe/` - standalone virtual-desktop COM probe (planned).

## Notes

- Virtual-desktop COM interfaces are undocumented and version-sensitive. Test interop changes on the
  supported Windows versions rather than assuming one layout.
- Windows belonging to the pager process are intentionally excluded from the pager.
- The Rust implementation must preserve the behavior described in `rust/ARCHITECTURE.md` section 9.
  Intentional deviations from the C# implementation are listed in section 10 there.
- Do not add a settings window, DWM thumbnails, Direct2D, per-monitor DPI v2, or ARM64 support unless
  the user explicitly asks for them.
- Do not introduce a GUI framework (`winsafe`, `windows-reactor`, `egui`, etc.) for the v1 pager.
- Keep the C# and Rust settings registry values compatible while both implementations exist.
