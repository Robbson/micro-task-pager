# C# WinForms vs Rust WinUI 3

Deployment, footprint, and code comparison for the two Micro Task Pager implementations.

Measured on 2026-11-09. The C#/.NET implementation is the current release; the Rust/WinUI 3 port
is feature-complete but unreleased.

For alternatives to the current WinUI 3 dependency and their trade-offs, see
[`UI-FRAMEWORK-COMPARISON.md`](UI-FRAMEWORK-COMPARISON.md).

## Executive summary

| Aspect                     | C# WinForms                                  | Rust WinUI 3                                     | Winner     |
| -------------------------- | -------------------------------------------- | ------------------------------------------------ | ---------- |
| Distribution size          | 1.49 MiB in 13 files                         | 3.36 MiB in 1 file                               | C#         |
| Clean-machine prerequisite | .NET Framework 4.8 (inbox on modern Windows) | Windows App Runtime 2.4+ (119 MiB x64 framework) | C#         |
| Idle working set           | 55-57 MiB                                    | 99 MiB                                           | C#         |
| Idle private bytes         | 33 MiB                                       | 64 MiB                                           | C#         |
| Idle CPU, default polling  | 1.13 % of total system (27 % of one core)    | 0.63 % of total system (15 % of one core)        | Rust       |
| GDI objects while idle     | 24                                           | 9                                                | Rust       |
| Handles / threads          | ~430 / 15-18                                 | ~1 734 / 60-66                                   | C#         |
| Application source         | 4 036 code lines                             | 5 342 code lines                                 | C#         |
| Automated tests            | none                                         | 52 unit tests                                    | Rust       |
| Branch density             | 68.6 per 1 000 code lines                    | 61.0 per 1 000 code lines                        | Comparable |

The Rust build trades a larger binary, a large shared runtime prerequisite, and roughly double the
memory for fewer GDI objects, lower idle CPU, a much smaller COM layer, and a test suite. Startup
time is effectively identical.

## Measurement setup

| Item                | Value                                                                                 |
| ------------------- | ------------------------------------------------------------------------------------- |
| Machine             | AMD Ryzen AI 9 HX 370, 12 cores / 24 threads, 59.6 GiB RAM, Radeon 890M               |
| OS                  | Windows 11 25H2, build 26200.9711                                                     |
| .NET runtime        | .NET Framework 4.8.1 (release 533509)                                                 |
| Windows App Runtime | 2.6.5                                                                                 |
| Rust                | 1.99.1, MSVC, release profile: LTO, `codegen-units = 1`, stripped symbols, static CRT |
| C# build            | `dotnet build src/MicroTaskPager.csproj -c Release` (`net48`, x64)                    |
| Rust build          | `cargo build --release` in `rust/`                                                    |

Method:

- Both applications ran with the same stored settings: primary update delay 200 ms, secondary update
  delay 500 ms, the session's normal monitor layout and window set, no dialog open.
- Each application was launched twice per measurement and closed with `WM_CLOSE`.
- Startup time is process creation to the first main window, polled every 25 ms, so it has a 25 ms
  resolution and includes PowerShell process-creation overhead.
- Memory numbers are the steady-state sample at 20 s. Border values are ranges over the two runs.
- CPU is kernel-accounted process CPU time from a second run pair, sampled in six 10 s intervals
  after a 10 s settle. The table reports the average over the 60 s window. "% of one core" is CPU
  seconds divided by wall seconds; "% of total system" divides that by the 24 logical CPUs. Process
  Explorer's process CPU column is relative to total system CPU, which is why its numbers look about
  24x smaller than a per-core figure.
- The measurements ran on a normal, non-isolated desktop. Process CPU time is not inflated by other
  processes competing for the CPU, but short sampling windows are noisy; the 60 s average was stable
  within about ±1 percentage point of one core across intervals and runs.
- Both applications saw the same window set. Their CPU scales with the number of windows and the
  pager size because both enumerate and redraw on the same timers, so treat these as warm desktop
  idle figures for this session rather than a general benchmark.
- Code metrics come from a small script that strips comments and string literals, then counts code,
  comment, and blank lines, `fn`/method declarations, and branch points (`if`, `for`, `foreach`,
  `while`, `loop`, `case`, `catch`, `when`, `guard`, `&&`, `||`, and ternaries). This is a
  readability-oriented approximation, not McCabe complexity, and it does not count `switch`
  expression arms or LINQ predicate branches.
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
| Main executable     | 744 KiB (`MicroTaskPager.exe`) | 3.36 MiB (`micro-task-pager.exe`)      |
| Additional DLLs     | 11 (0.78 MiB)                  | none                                   |
| Config file         | 1 (870 bytes)                  | none                                   |
| Total shipped files | 13                             | 1                                      |
| Total deployed size | 1.49 MiB                       | 3.36 MiB                               |
| Debug symbols       | `.pdb` separate, 31 KiB        | stripped; no PDB in the measured build |
| Runtime bundled     | no                             | no                                     |

The C# distribution carries Serilog and its transitive .NET libraries; the Rust executable statically
links the standard library, the `windows` bindings, and the reactor glue. The Rust binary is 2.3x
larger, but still small in absolute terms. Including the shared runtime, the comparison flips: the
Rust app needs a 119 MiB framework package for a fresh install, while the C# app relies on a framework
that current Windows versions already include.

## 3. Runtime resource use

Steady state with closed dialogs and default polling. Memory is sampled at 20 s; CPU is the 60 s
average:

| Metric              |               C# WinForms |              Rust WinUI 3 | Rust / C# |
| ------------------- | ------------------------: | ------------------------: | --------: |
| First (cold) start  |                    555 ms |                    601 ms |     1.08x |
| Second (warm) start |                    248 ms |                    286 ms |     1.15x |
| Working set         |             54.9-56.8 MiB |             99.0-99.1 MiB |     1.76x |
| Private bytes       |                  32.8 MiB |             62.9-63.9 MiB |     1.94x |
| Commit              |                  32.8 MiB |             62.9-63.9 MiB |     1.94x |
| Handles             |                   389-482 |               1 731-1 736 |     4.03x |
| Threads             |                     15-18 |                     60-66 |     3.73x |
| GDI objects         |                        24 |                         9 |     0.38x |
| USER objects        |                        26 |                        36 |     1.38x |
| Idle CPU, one core  |                    27.0 % |                    15.1 % |     0.56x |
| Idle CPU, total     | 1.13 % of 24 logical CPUs | 0.63 % of 24 logical CPUs |     0.56x |

Observations:

- Startup is the same within one polling interval. WinUI and the CLR both need roughly a quarter of a
  second on a warm cache.
- Rust uses about twice the private memory and working set. Most of the difference is the WinUI
  composition stack, DirectX, and the Windows App Runtime host loaded into the process. The CLR plus
  GDI+ is lighter for a small custom-drawn window.
- Rust creates about 4x the handles and 3.7x the threads, again from the WinUI runtime. Handles
  fluctuate as WinUI creates and retires short-lived objects, but none of the measured values grew
  monotonically over 70 s, so this is steady-state overhead rather than a leak.
- Rust uses fewer GDI objects because WinUI composition does not draw through GDI. The C# pager draws
  every preview and icon with GDI+.
- Both implementations poll: the snapshot tick enumerates windows every 200 ms and the maintenance
  tick runs every 500 ms. That explains the constant CPU load in both. Rust uses about 44 % less
  process CPU (15.1 vs 27.0 % of one core), but the absolute gap is only about half a percentage
  point of the machine's total CPU (0.63 vs 1.13 %). Neither is event-driven; users can raise the
  delays in Settings at the cost of slower updates.
- A Process Explorer spot check on the same machine showed the same ordering: about 0.8 % for C# and
  0.6 % for Rust of total system CPU. Rust matches the 60 s average above; the C# spot reading is
  somewhat lower, which is expected from a different sampling window. The absolute numbers are close
  enough that the difference matters less than the fact that both settle around 1 % of the machine's
  total CPU.

## 4. Code complexity

### Source size

All application sources, excluding generated files and build output. Rust inline tests are shown
separately because they ship in the same files but are not part of the application logic.

| Metric                             | C# WinForms | Rust, application |                  Rust, including tests |
| ---------------------------------- | ----------: | ----------------: | -------------------------------------: |
| Source files                       |          24 |                29 |                                     29 |
| Total lines                        |       4 828 |             7 292 |                                  8 240 |
| Code lines                         |       4 036 |             5 342 |                                  6 156 |
| Comment lines                      |         184 |             1 197 |                                  1 202 |
| Comment share of non-blank lines   |       4.4 % |            18.3 % |                                 16.3 % |
| Functions (heuristic)              |         209 |               312 |                                    387 |
| Branch points                      |         277 |               326 |                                    332 |
| Branch points per 1 000 code lines |        68.6 |              61.0 |                                   53.9 |
| Code lines per function            |        19.3 |              17.1 |                                   15.9 |
| Automated tests                    |           0 |                 - | 52 `#[test]` functions, 830 code lines |
| `unsafe` blocks                    |         n/a |               121 |                                    121 |

The Rust application has about 32 % more code lines and 49 % more functions. Part of that is
explicit: error handling, the COM vtable definitions, reactor declarations, and documentation. The
Rust test code adds another 830 lines.

Branch density is slightly lower in Rust when tests are excluded, so the shipping code is not
branchier per line. The metric says nothing about how hard each branch is to reason about; the C#
version concentrates complexity in a few very large Form files, while the Rust version splits state
machines into testable modules.

### Where the code lives

C# by area (includes all source):

| Area                      | Files | Code lines | Branch points | Functions |
| ------------------------- | ----: | ---------: | ------------: | --------: |
| WinForms UI (`GUI/`)      |     6 |      1 847 |           142 |        79 |
| COM interop (`Core/API/`) |     8 |      1 435 |            58 |        88 |
| Core model (`Core/`)      |     6 |        452 |            54 |        27 |
| Config (`Config/`)        |     3 |        217 |            20 |        12 |
| Program and mutex         |     1 |         85 |             3 |         3 |

Rust by area (includes inline tests):

| Area                                                            | Files | Code lines | Branch points | Functions |
| --------------------------------------------------------------- | ----: | ---------: | ------------: | --------: |
| Pager UI (`pager/`)                                             |     6 |      2 284 |           139 |       129 |
| Dialog UI (`dialogs/`)                                          |     6 |      1 347 |            47 |        74 |
| Platform (`windows`, `monitors`, `foreground`, `native_window`) |     5 |      1 365 |            83 |        77 |
| COM interop (`desktop/`)                                        |     5 |        710 |            48 |        73 |
| App (`settings`, `single_instance`, `logging`, root files)      |     7 |        450 |            15 |        34 |

The largest structural difference is the COM layer. The C# project has four near-identical
`VirtualDesktopAPI-*.cs` variants totalling 1 216 code lines for the Windows 10, Windows 10 LTSC, and
Windows 11 layouts. The Rust `desktop/` module defines the interfaces once and selects a layout
backend, in 710 code lines including its tests.

### Largest files

| C#                                         | Code lines | Branch points |
| ------------------------------------------ | ---------: | ------------: |
| `src/GUI/AppSettingsWindow.cs`             |        828 |            45 |
| `src/GUI/MainForm.cs`                      |        435 |            34 |
| `src/Core/API/VirtualDesktopAPI-Win11_.cs` |        325 |            14 |
| `src/Core/API/VirtualDesktopAPI-Win11.cs`  |        308 |            11 |
| `src/GUI/VirtualDesktop/Desktop.cs`        |        228 |            40 |

| Rust                                | Code lines | Branch points |
| ----------------------------------- | ---------: | ------------: |
| `rust/src/pager/mod.rs`             |        836 |            83 |
| `rust/src/pager/layout.rs`          |        633 |            26 |
| `rust/src/native_window.rs`         |        590 |            31 |
| `rust/src/dialogs/settings/view.rs` |        482 |             2 |
| `rust/src/desktop/com/core.rs`      |        374 |            29 |

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
- **CPU favors Rust, but the absolute gap is small.** Both poll the same way, but the Rust process
  spends about 44 % less CPU per poll: 0.63 % versus 1.13 % of total system CPU on this 24-thread
  machine. That matters for battery life and heavily loaded systems more than for a desktop at idle.
- **Maintainability is mixed.** Rust has more code, split into more, smaller functions, with 52 tests
  and a much smaller COM layer. C# has 32 % less code but no automated tests, a 828-line Settings
  file, and four duplicated COM layout implementations.
- **Onboarding favors documented Rust.** The Rust sources carry 16-18 % comments and module-level
  state documentation; the C# sources carry 4.4 %. Whether that outweighs the unfamiliar WinUI/reactor
  model depends on the reader's background.

## 6. Reproduction

```powershell
# C# release
dotnet build src/MicroTaskPager.csproj -c Release
Get-ChildItem src/bin/Release/net48 -File

# Rust release
Push-Location rust
cargo build --release
Pop-Location
Get-Item rust/target/release/micro-task-pager.exe

# Runtime versions on this machine
Get-AppxPackage -Name "Microsoft.WindowsAppRuntime.2" |
    Select-Object Version, Architecture, InstallLocation
Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\NET Framework Setup\NDP\v4\Full'
```

Resource measurements used `Get-Process` (`WorkingSet64`, `PrivateMemorySize64`,
`PagedMemorySize64`, `Handles`, `Threads`, `TotalProcessorTime`) plus `GetGuiResources` from
`user32.dll`. The code metrics were produced with a one-off analyzer; the counting rules are listed
in the measurement setup above so the numbers can be reproduced or challenged.
