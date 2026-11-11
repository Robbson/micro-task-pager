# C# WinForms vs Rust WinUI 3

Deployment, footprint, and code comparison for the C# WinForms and Rust WinUI 3 implementations.
The stable build is `rust-winsafe`; it is summarized at the end of this document.

Measured on 2026-11-09. The C#/.NET implementation has the latest features but is less tested in
production; some features may be experimental. The rust-winui/WinUI 3 port is experimental: it
currently matches the winsafe feature set but is not planned to receive updates in the near future
because its `windows-reactor` foundation is not stable yet.

For alternatives to the current WinUI 3 dependency and their trade-offs, see
[`UI-FRAMEWORK-COMPARISON.md`](UI-FRAMEWORK-COMPARISON.md).

## Executive summary

| Aspect                     | C# WinForms                                  | Rust WinUI 3                                     | Winner     |
| -------------------------- | -------------------------------------------- | ------------------------------------------------ | ---------- |
| Distribution size          | 1.49 MiB in 13 files                         | 3.37 MiB in 1 file                               | C#         |
| Clean-machine prerequisite | .NET Framework 4.8 (inbox on modern Windows) | Windows App Runtime 2.4+ (119 MiB x64 framework) | C#         |
| Idle working set           | 59-62 MiB                                    | 99 MiB                                           | C#         |
| Idle private bytes         | 33-37 MiB                                    | 63-64 MiB                                        | C#         |
| Idle CPU, default polling  | 0.30 % of total system (7.2 % of one core)   | 0.25 % of total system (5.9 % of one core)       | Rust       |
| GDI objects while idle     | 24                                           | 9                                                | Rust       |
| Handles / threads          | ~397 / 18                                    | ~1 736 / 65-66                                   | C#         |
| Application source         | 4 286 code lines                             | 5 391 code lines                                 | C#         |
| Automated tests            | none                                         | 54 unit tests                                    | Rust       |
| Branch density             | 73.7 per 1 000 code lines                    | 58.4 per 1 000 code lines                        | Comparable |

The Rust build trades a larger binary, a large shared runtime prerequisite, and roughly 1.6-1.8x the
memory for fewer GDI objects, a marginal idle-CPU edge, a much smaller COM layer, and a test suite.
Both implementations now batch the virtual-desktop lookup per snapshot, which cut their idle CPU by
2.5x (Rust) to 3.8x (C#); the CPU difference between them is now 0.05 percentage points of the
machine's total. Startup time is effectively identical.

## Measurement setup

| Item                | Value                                                                                 |
| ------------------- | ------------------------------------------------------------------------------------- |
| Machine             | AMD Ryzen AI 9 HX 370, 12 cores / 24 threads, 59.6 GiB RAM, Radeon 890M               |
| OS                  | Windows 11 25H2, build 26200.9711                                                     |
| .NET runtime        | .NET Framework 4.8.1 (release 533509)                                                 |
| Windows App Runtime | 2.6.5                                                                                 |
| Rust                | 1.99.1, MSVC, release profile: LTO, `codegen-units = 1`, stripped symbols, static CRT |
| C# build            | `dotnet build csharp-winforms/MicroTaskPager.csproj -c Release` (`net48`, x64)                    |
| Rust build          | `cargo build --release` in `rust-winui/`                                                    |

Method:

- Both applications ran with the same stored settings: primary update delay 200 ms, secondary update
  delay 500 ms, the session's normal monitor layout and window set (17 windows on 2 virtual
  desktops), no dialog open.
- Each application was launched four times per measurement and closed with `WM_CLOSE`.
- Startup time is process creation to the first main window, polled every 25 ms, so it has a 25 ms
  resolution and includes PowerShell process-creation overhead.
- Memory numbers are the steady-state sample at 20 s. Values are ranges over the runs.
- CPU is kernel-accounted process CPU time from two independent run pairs, each sampled in six 10 s
  intervals after a 10 s settle. The table reports the mean of the two 60 s averages. "% of one core"
  is CPU seconds divided by wall seconds; "% of total system" divides that by the 24 logical CPUs.
  Process Explorer's process CPU column is relative to total system CPU, which is why its numbers
  look about 24x smaller than a per-core figure.
- The measurements ran on a normal, non-isolated desktop. Process CPU time is not inflated by other
  processes competing for the CPU, but short sampling windows are noisy; the 60 s averages were
  within about ±1.5 percentage points of one core across intervals and runs.
- Both applications saw the same window set. Their CPU scales with the number of windows and the
  pager size because both enumerate and redraw on the same timers, so treat these as warm desktop
  idle figures for this session rather than a general benchmark.
- Code metrics come from a small script that strips comments and string literals, then counts code,
  comment-only, and blank lines, `fn`/method declarations that own a body, and branch points (`if`,
  `for`, `foreach`, `while`, `loop`, `case`, `catch`, `when`, `guard`, `&&`, `||`, and C#
  ternaries). This is a readability-oriented approximation, not McCabe complexity, and it does not
  count `switch`/`match` arms, Rust `?` error propagation, or LINQ predicate branches. The metrics
  were regenerated with the same script after the polling change, so they are internally consistent
  but not directly comparable with earlier revisions of this document.
- Not measured: installer download size, resource use while dragging windows, resource use with
  hundreds of windows, battery impact, and CPU under an isolated benchmark workload.

## 1. Dependencies on a clean Windows 10/11 machine

| Requirement                | C# WinForms                                               | Rust WinUI 3                                         |
| -------------------------- | --------------------------------------------------------- | ---------------------------------------------------- |
| Minimum OS                 | Windows 10, 64-bit                                        | Windows 10 version 1809 (build 17763), 64-bit        |
| UI framework               | WinForms + WPF, part of .NET Framework                    | WinUI 3, provided by the Windows App Runtime         |
| Primary runtime            | .NET Framework 4.8                                        | Windows App Runtime 2.4 or later                     |
| Runtime availability       | Inbox on Windows 10 1903+ and Windows 11; install on 1809 | Never inbox; separate installer, framework package   |
| Visual C++ redistributable | Not required                                              | Not required (static MSVC runtime)                   |
| Bundled runtime files      | 11 .NET libraries from NuGet                              | none                                                 |
| Application install        | Copy a folder; no installer                               | Copy one executable; no installer                    |
| Runtime servicing          | Windows Update                                            | Windows App Runtime updates, MSIX framework packages |
| Architecture               | x64                                                       | x64                                                  |

### Framework footprint

| Package                                                        | Measured disk use |
| -------------------------------------------------------------- | ----------------- |
| Windows App Runtime 2.6.5, x64 framework                       | 119.4 MiB         |
| Windows App Runtime 2.6.5, x86 framework                       | 59.1 MiB          |
| All Windows App Runtime 2.x framework versions on this machine | 903.1 MiB         |

The Windows App Runtime uses side-by-side framework packages. Updating it keeps the old version
around for apps that still target it, so the disk cost is not one-time on a machine that receives
several runtime updates. The 903 MiB figure is the accumulation of nine 2.x x64/x86 packages on the
measurement machine, not the cost of a fresh install.

.NET Framework 4.8 is an OS component rather than an app-local dependency. It is inbox on Windows 10
1903 and later, so a current Windows 10/11 machine needs no extra download; Windows 10 1809 needs
the .NET Framework 4.8 installer first. The framework is shared and serviced by Windows Update, so it
does not add an app-specific side-by-side folder.

Both applications remain x64. The measured builds have no native ARM64 counterpart; on ARM64
Windows 11 they run through x64 emulation.

## 2. Deployment artifacts

| Artifact            | C# WinForms                    | Rust WinUI 3                           |
| ------------------- | ------------------------------ | -------------------------------------- |
| Main executable     | 729 KiB (`MicroTaskPager.exe`) | 3.37 MiB (`micro-task-pager.exe`)      |
| Additional DLLs     | 11 (0.78 MiB)                  | none                                   |
| Config file         | 1 (870 bytes)                  | none                                   |
| Total shipped files | 13                             | 1                                      |
| Total deployed size | 1.49 MiB                       | 3.37 MiB                               |
| Debug symbols       | `.pdb` separate, 33 KiB        | stripped; no PDB in the measured build |
| Runtime bundled     | no                             | no                                     |

The C# distribution carries Serilog and its transitive .NET libraries; the Rust executable statically
links the standard library, the `windows` bindings, and the reactor glue. The Rust binary is 2.3x
larger, but still small in absolute terms. Including the shared runtime, the comparison flips: the
Rust app needs a 119 MiB framework package for a fresh install, while the C# app relies on a framework
that current Windows versions already include.

## 3. Runtime resource use

Steady state with closed dialogs and default polling. Memory is sampled at 20 s; CPU is the mean of
two 60 s averages:

| Metric              |               C# WinForms |              Rust WinUI 3 | Rust / C# |
| ------------------- | ------------------------: | ------------------------: | --------: |
| First (cold) start  |                    592 ms |                    599 ms |     1.01x |
| Second (warm) start |                    220 ms |                    236 ms |     1.07x |
| Working set         |             58.8-62.1 MiB |             98.8-99.3 MiB |     1.64x |
| Private bytes       |             33.3-36.8 MiB |             63.4-63.8 MiB |     1.82x |
| Commit              |             33.3-36.8 MiB |             63.4-63.8 MiB |     1.82x |
| Handles             |                   396-399 |               1 733-1 738 |     4.37x |
| Threads             |                        18 |                     65-66 |     3.65x |
| GDI objects         |                        24 |                         9 |     0.38x |
| USER objects        |                        26 |                     35-36 |     1.37x |
| Idle CPU, one core  |                     7.2 % |                     5.9 % |     0.82x |
| Idle CPU, total     | 0.30 % of 24 logical CPUs | 0.25 % of 24 logical CPUs |     0.83x |

Observations:

- Startup remains within one 25 ms polling interval in both pairs. A repeat pass on a warm file cache
  measured 295/304 ms for the first launch and 234/246 ms for the second, so the difference is
  measurement noise rather than a real distinction.
- Rust still uses about 1.6x the working set and 1.8x the private memory. Most of the difference is
  the WinUI composition stack, DirectX, and the Windows App Runtime host loaded into the process. The
  CLR plus GDI+ is lighter for a small custom-drawn window.
- Rust creates about 4.4x the handles and 3.7x the threads, again from the WinUI runtime. Handles
  fluctuate as WinUI creates and retires short-lived objects, but none of the measured values grew
  monotonically over 70 s, so this is steady-state overhead rather than a leak.
- Rust uses fewer GDI objects because WinUI composition does not draw through GDI. The C# pager draws
  every preview and icon with GDI+.
- Both implementations poll: the snapshot tick enumerates windows every 200 ms and the maintenance
  tick runs every 500 ms. That explains the constant CPU load in both. Both now fetch the virtual
  desktop GUID list once per snapshot and resolve each window against it; previously every window
  lookup re-enumerated the list through COM. That change cut idle CPU by 3.8x in C# and 2.5x in Rust
  (C# 1.13 -> 0.30 %, Rust 0.63 -> 0.25 % of total system).
- After that fix the CPU difference is small: Rust uses about 18 % less process CPU (5.9 vs 7.2 % of
  one core), but the absolute gap is only 0.05 percentage points of the machine's total CPU. Neither
  is event-driven; users can raise the delays in Settings at the cost of slower updates. The
  remaining cost is dominated by window enumeration, icon queries, and the desktop-list fetch rather
  than the change-detection hash.

## 4. Code complexity

### Source size

All application sources, excluding generated files and build output. Rust inline tests are shown
separately because they ship in the same files but are not part of the application logic.

| Metric                             | C# WinForms | Rust, application |                  Rust, including tests |
| ---------------------------------- | ----------: | ----------------: | -------------------------------------: |
| Source files                       |          24 |                29 |                                     29 |
| Total lines                        |       5 111 |             7 369 |                                  8 352 |
| Code lines                         |       4 286 |             5 391 |                                  6 236 |
| Comment lines                      |         172 |             1 220 |                                  1 225 |
| Comment share of non-blank lines   |       3.9 % |            18.5 % |                                 16.4 % |
| Functions (heuristic)              |         196 |               320 |                                    399 |
| Branch points                      |         316 |               315 |                                    320 |
| Branch points per 1 000 code lines |        73.7 |              58.4 |                                   51.3 |
| Code lines per function            |        21.9 |              16.8 |                                   15.6 |
| Automated tests                    |           0 |                 - | 54 `#[test]` functions, 845 code lines |
| `unsafe` blocks                    |         n/a |               121 |                                    121 |

The Rust application has about 26 % more code lines and 63 % more functions. Part of that is
explicit: error handling, the COM vtable definitions, reactor declarations, and documentation. The
Rust test code adds another 845 lines.

Branch density is lower in Rust when tests are excluded (58.4 vs 73.7 per 1,000 code lines), so the
application code is not branchier per line. The metric says nothing about how hard each branch is to
reason about; the C# version concentrates complexity in a few very large Form files, while the Rust
version splits state machines into testable modules.

### Where the code lives

C# by area (includes all source):

| Area                      | Files | Code lines | Branch points | Functions |
| ------------------------- | ----: | ---------: | ------------: | --------: |
| WinForms UI (`GUI/`)      |     6 |      1 847 |           152 |        82 |
| COM interop (`Core/API/`) |     8 |      1 663 |            82 |        70 |
| Core model (`Core/`)      |     6 |        474 |            58 |        29 |
| Config (`Config/`)        |     3 |        217 |            21 |        12 |
| Program and mutex         |     1 |         85 |             3 |         3 |

Rust by area (includes inline tests):

| Area                                                            | Files | Code lines | Branch points | Functions |
| --------------------------------------------------------------- | ----: | ---------: | ------------: | --------: |
| Pager UI (`pager/`)                                             |     6 |      2 283 |           139 |       129 |
| Dialog UI (`dialogs/`)                                          |     6 |      1 347 |            47 |        74 |
| Platform (`windows`, `monitors`, `foreground`, `native_window`) |     5 |      1 416 |            83 |        81 |
| COM interop (`desktop/`)                                        |     5 |        740 |            37 |        81 |
| App (`settings`, `single_instance`, `logging`, root files)      |     7 |        450 |            14 |        34 |

The largest structural difference is the COM layer. The C# project has four near-identical
`VirtualDesktopAPI-*.cs` variants totalling 1 440 code lines for the Windows 10, Windows 10 LTSC, and
Windows 11 layouts. The Rust `desktop/` module defines the interfaces once and selects a layout
backend, in 740 code lines including its tests.

### Largest files

| C#                                            | Code lines | Branch points |
| --------------------------------------------- | ---------: | ------------: |
| `csharp-winforms/GUI/AppSettingsWindow.cs`                |        828 |            53 |
| `csharp-winforms/GUI/MainForm.cs`                         |        435 |            35 |
| `csharp-winforms/Core/API/VirtualDesktopAPI-Win11_.cs`    |        381 |            20 |
| `csharp-winforms/Core/API/VirtualDesktopAPI-Win11.cs`     |        364 |            17 |
| `csharp-winforms/Core/API/VirtualDesktopAPI-Win10LTSC.cs` |        348 |            17 |
| `csharp-winforms/Core/API/VirtualDesktopAPI-Win10.cs`     |        347 |            17 |

| Rust                                | Code lines | Branch points |
| ----------------------------------- | ---------: | ------------: |
| `rust-winui/src/pager/mod.rs`             |        835 |            83 |
| `rust-winui/src/pager/layout.rs`          |        633 |            26 |
| `rust-winui/src/native_window.rs`         |        590 |            31 |
| `rust-winui/src/dialogs/settings/view.rs` |        482 |             2 |
| `rust-winui/src/desktop/com/core.rs`      |        387 |            20 |

`pager/mod.rs` is the Rust hot spot: one component owns snapshot, layout, input, window, and dialog
coordination. It is the file to watch for further splitting. The C# hot spot is the 828-line Settings
window, which combines the form, its controls, validation, and persistence.

### Dependencies

| Metric                      |                            C# WinForms |                                                 Rust WinUI 3 |
| --------------------------- | -------------------------------------: | -----------------------------------------------------------: |
| Direct package dependencies |             5 (1 analyzer, build-only) |                                             6 (1 build-only) |
| Transitive packages         |                    7 runtime libraries |                                    55 crates in `Cargo.lock` |
| Runtime libraries shipped   |                                11 DLLs |                                                            0 |
| Unsafe code                 | none visible; P/Invoke and COM interop | 121 documented `unsafe` blocks, confined to platform modules |

The Rust crate graph is larger at build time, but none of it ships separately: the code is linked
into the executable. The C# package graph is smaller, but its runtime pieces are separate DLLs.

## 5. Reading the numbers

- **Disk and prerequisites favor C#.** On a modern Windows 10/11 machine the C# app is a 1.5 MiB
  folder with no runtime installation. The Rust app is a 3.4 MiB executable plus a 119 MiB framework
  package that is not part of Windows. If the machine already has the Windows App Runtime, the
  marginal difference is one file.
- **Memory favors C#.** The Rust process needs roughly 30 MiB more private memory while idle. That is
  the price of WinUI composition and the runtime host.
- **CPU is now close to a tie.** Both implementations spend about a quarter of a percent of the
  machine's total CPU at idle. Rust uses about 18 % less process CPU (5.9 vs 7.2 % of one core), but
  that is 0.05 percentage points of total system CPU. The earlier 1.8x gap came from re-enumerating
  the desktop list per window; after batching it per snapshot, both improved by 2.5-3.8x.
- **Maintainability is mixed.** Rust has more code (26 % more code lines, 63 % more functions),
  split into more, smaller functions, with 54 tests and a much smaller COM layer. C# has less code but
  no automated tests, an 828-line Settings file, and four duplicated COM layout implementations
  totalling 1 440 code lines.
- **Onboarding favors documented Rust.** The Rust sources carry about 18 % comments and module-level
  state documentation; the C# sources carry 3.9 %. Whether that outweighs the unfamiliar WinUI/reactor
  model depends on the reader's background.

## 6. Reproduction

```powershell
# C# release
dotnet build csharp-winforms/MicroTaskPager.csproj -c Release
Get-ChildItem csharp-winforms/bin/Release/net48 -File

# Rust release (both Rust crates and the shared core are one workspace)
cargo build --release -p micro-task-pager-winui
cargo build --release -p micro-task-pager-winsafe
Get-Item target/release/micro-task-pager-winui.exe
Get-Item target/release/micro-task-pager-winsafe.exe

# Runtime versions on this machine
Get-AppxPackage -Name "Microsoft.WindowsAppRuntime.2" |
    Select-Object Version, Architecture, InstallLocation
Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\NET Framework Setup\NDP\v4\Full'
```

Resource measurements used `Get-Process` (`WorkingSet64`, `PrivateMemorySize64`,
`PagedMemorySize64`, `Handles`, `Threads`, `TotalProcessorTime`) plus `GetGuiResources` from
`user32.dll`. The code metrics were produced with a one-off analyzer; the counting rules are listed
in the measurement setup above so the numbers can be reproduced or challenged.

## Stable rust-winsafe build

The stable implementation in `rust-winsafe/` uses native Win32 through `winsafe` and has no separate
UI runtime. It is the stable build, with fewer features than the C# implementation; new features are
added only after a delay. Measured on 2026-11-09 on the same machine;
source-line metrics are intentionally omitted for this comparison. Full status is in
[`rust-winsafe/MIGRATION.md`](../rust-winsafe/MIGRATION.md).

| Metric | rust-winsafe | C# WinForms | Rust WinUI 3 |
|---|---:|---:|---:|
| Deployed size | 1.06 MiB in 1 file | 1.49 MiB in 13 files | 3.37 MiB in 1 file |
| Runtime prerequisite | None beyond Windows | .NET Framework 4.8 (inbox) | Windows App Runtime 2.4+ (~111-119 MiB) |
| Idle CPU, one core | 3.7-4.1 % | 7.2 % | 5.9 % |
| Idle CPU, total system | 0.155-0.170 % | 0.30 % | 0.25 % |
| Working set | 23.1 MiB | 58.8-62.1 MiB | 98.8-99.3 MiB |
| Private bytes | 3.1 MiB | 33.3-36.8 MiB | 63.4-63.8 MiB |
| Handles / threads | 198 / 8 | 396-399 / 18 | 1 733-1 738 / 65-66 |
| GDI / USER objects | 10 / 16 | 24 / 26 | 9 / 35-36 |
| Startup (warm) | 144 ms | 220 ms | 236 ms |

The winsafe figures use the same 60 s CPU method and 20 s steady-state memory sample as the rest of
this document. Startup is a single 25 ms-resolution run with a warm file cache. The implementation
trades WinUI's declarative views for native winsafe controls and a GDI renderer; the pager, About,
and Settings are all fixed-size native windows.
