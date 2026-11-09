# Micro Task Pager

<p align="center">
  <img src="csharp-winforms/Resources/logo.svg" width="520" alt="Micro Task Pager logo">
</p>

<p align="center">
  A compact, always-available desktop pager for Windows 10 and 11.
</p>

<p align="center">
  <a href="https://github.com/Robbson/micro-task-pager/releases/latest"><img src="https://img.shields.io/github/v/release/Robbson/micro-task-pager" alt="Latest release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
  <img
    src="https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-lightgrey"
    alt="Platform: Windows 10 and 11"
  >
</p>

**Micro Task Pager** gives you a permanent overview of your Windows virtual desktops. It shows what is open on every desktop, lets you switch with a single click, and lets you move windows between desktops with drag and drop.

Unlike a simple desktop switcher, it can display each desktop in one of two ways:

- **Preview** — a miniature of the real screen, including window positions and z-order.
- **Micro Taskbar** — a compact strip of application icons, ideal for small screens and maximized windows.

Each desktop screen can use its own view, so you can mix both styles however you like.

<p align="center">
  <img
    src="csharp-winforms/Resources/pager-example.png"
    alt="Micro Task Pager showing multiple virtual desktops"
  >
  <br>
  <em>Micro Task Pager with several desktops, using both preview and Micro Taskbar views.</em>
</p>

## Why another pager?

Windows virtual desktops are useful, but the native experience has limitations: there are only a few fixed shortcuts, there is no persistent overview of all desktops, and the Task View (`Win` + `Tab`) is a full-screen interruption rather than a quick glance.

Micro Task Pager fills that gap with a small panel that stays out of the way and is always ready when you need it. It uses the native virtual desktop features of Windows, so it works alongside other window managers and desktop tools instead of replacing them.

> [!NOTE]
> Windows has no native support for oversized desktops, so the pager does not implement virtual desktop _pages_ as some classic Linux tools do.

## Features

- Switch to any virtual desktop with a single mouse click.
- See all desktops at once, with a near real-time view of their windows.
- Choose **Preview** or **Micro Taskbar** rendering independently for each desktop screen.
- Drag and drop windows between virtual desktops.
- Click a window in the current desktop to bring it to the front.
- Return to the last active window automatically when switching back to a desktop.
- Reposition the pager by dragging it, and save, restore, or reset its position from the context menu.
- Keep the pager visible on every virtual desktop and above other windows; it restores itself when covered.
- Use multiple monitors; screen changes are detected while the app is running. High-DPI displays are supported through DPI-aware rendering.
- Customize pager height, icon padding, colors, border style, opacity, and update intervals.
- Automatically follows the number of virtual desktops, including desktops added or removed at runtime.
- Prevents multiple instances from running at the same time.
- Runs without administrator rights and stores its settings per user.
- Writes diagnostic logs to `%LOCALAPPDATA%\MicroTaskPager\logs`.
- Free and open source, with no Windows patches or shell extensions required.

## Getting started

### Select the implementation you like

Three implementations are available from the same repository. The C# build is the current published
release, the WinUI 3 build is feature-complete and can be built from source, and the native winsafe
build is an experimental no-runtime variant. The table compares them by requirements, app footprint,
performance, runtime memory, and look.

| Aspect                         | C# WinForms                                        | Rust WinUI 3                                                       | Rust winsafe                                |
| ------------------------------ | -------------------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------- |
| Runtime requirement            | Windows 10/11 x64; .NET Framework 4.8 (inbox 1903+) | Windows 10 1809+ x64; Windows App Runtime 2.4+ (~111-119 MiB, install) | Windows 10/11 x64; no extra runtime         |
| App footprint                  | 1.49 MiB in 13 files                               | 3.37 MiB in 1 file                                                 | 1.06 MiB in 1 file                          |
| Idle CPU                       | 7.2% of one core / 0.30% total                     | 5.9% of one core / 0.25% total                                     | 3.7-4.1% of one core / 0.155-0.170% total   |
| Warm startup                   | 220 ms                                             | 236 ms                                                             | 144 ms                                      |
| Runtime memory                 | 59-62 MiB working set, 33-37 MiB private            | 99 MiB working set, 63-64 MiB private                              | 23.1 MiB working set, 3.1 MiB private       |
| Look                           | Native WinForms; functional                        | Modern Fluent/WinUI 3; the only genuinely polished one             | Native Win32 controls; functional, plain    |

Details: [`COMPARISON.md`](COMPARISON.md),
[`rust-winui/ARCHITECTURE.md`](rust-winui/ARCHITECTURE.md),
[`rust-winui/MIGRATION.md`](rust-winui/MIGRATION.md),
[`rust-winsafe/MIGRATION.md`](rust-winsafe/MIGRATION.md), and
[`UI-FRAMEWORK-COMPARISON.md`](UI-FRAMEWORK-COMPARISON.md).

### Requirements

- Windows 10 or Windows 11, 64-bit. Current Windows 11 releases, including 26H2, are tested.
- For the C# build: .NET Framework 4.8 or newer. It is included in Windows 10 version 1903 and later, as well as Windows 11.
- On ARM64 devices running Windows 11, the x64 build runs through Windows emulation; there is no native ARM64 build yet.

The three builds have different runtime prerequisites; the comparison table above lists them. The
WinUI build is framework-dependent and does not bundle the Windows App Runtime.

### Installation

1. Download the release for the implementation you want: `MicroTaskPager-winforms-v*.zip` (C#),
   `MicroTaskPager-winui-v*.zip` (Rust WinUI 3), or `MicroTaskPager-winsafe-v*.zip` (experimental
   native Rust, when published) from the
   [releases page](https://github.com/Robbson/micro-task-pager/releases/latest), or build it from
   source as described below.
2. Extract the archive.
3. Run `MicroTaskPager.exe`.

No installer is required. Keep the extracted files together: the WinForms executable needs the bundled DLLs and `MicroTaskPager.exe.config`, while the Rust WinUI build ships as a single executable and requires the Windows App Runtime 2.4 or later. The winsafe build is also a single executable and has no runtime prerequisite. Each archive also contains this README, the changelog, the license, and the third-party notices.

> [!IMPORTANT]
> The application is not code-signed yet. Windows SmartScreen may therefore show a warning the first time you start it.

On first launch, the pager appears at the bottom center of the primary screen's working area, so it does not overlap the taskbar.

## Using the pager

| Action                             | How                                                                                        |
| ---------------------------------- | ------------------------------------------------------------------------------------------ |
| Switch to a desktop                | Left-click anywhere on that desktop's area.                                                |
| Bring a window to the front        | Left-click its miniature or icon while its desktop is active.                              |
| Move a window to another desktop   | Drag its miniature or icon onto the target desktop.                                        |
| Change a desktop's view            | Right-click with the pointer over that desktop and choose **Toggle View Mode**.            |
| Move the pager                     | Drag it with the middle mouse button, or hold `Shift` and drag with the left mouse button. |
| Open the menu                      | Right-click the pager.                                                                     |
| Save or restore the pager position | Right-click, open **Position**, then choose **Save**, **Restore**, or **Default**.         |
| Open settings                      | Right-click the pager and choose **Settings**.                                             |
| Exit                               | Right-click the pager and choose **Exit**.                                                 |

### Preview view

The **Preview** view draws a scaled representation of each desktop, including the position, size, and stacking order of its windows. This is the closest equivalent to a traditional Linux-style pager and is useful when you want to understand the layout of a desktop at a glance.

### Micro Taskbar view

The **Micro Taskbar** view replaces the miniature layout with a compact strip of application icons, adding a second row when space is tight. It is especially useful on laptops and small displays where windows are usually maximized and stacked on top of each other. Icons are easier to recognize than overlapping preview rectangles, and the view remains readable even when the pager is only as tall as the Windows taskbar.

Each desktop's screen remembers its own view mode. Use **Toggle View Mode** on the desktop under the cursor to switch it.

## Configuration

Open **Settings** from the right-click menu. Changes are previewed live where possible and can be reverted per option or reset per tab.

- **Layout** — default view mode for new desktops, pager height, and icon padding.
- **Theme** — background opacity, desktop border style and colors, preview window colors, and active window highlighting.
- **Misc** — refresh intervals for window tracking and background tasks, plus whether the app appears in the Windows taskbar.

Settings and the saved pager position are stored under `HKEY_CURRENT_USER\SOFTWARE\MicroTaskPager`.

> [!NOTE]
> Automatic pinning of the pager to all virtual desktops requires **Show App In Taskbar** to be enabled. If you disable it, Windows limits what the app can do automatically, and you may need to pin it manually through Task View (`Win` + `Tab`).

## Known issues

- Not every visual parameter is configurable yet.
- Newly opened windows are occasionally not detected immediately.
- In **Micro Taskbar** view, the order of icons follows the window order and cannot be rearranged manually.
- The application is not code-signed, so SmartScreen warnings are expected.
- The project still targets .NET Framework 4.8, so there is no optimized native ARM64 build.

## Building from source

### Prerequisites

- Visual Studio 2022 with the **.NET desktop development** workload, or the .NET SDK with the .NET Framework 4.8 targeting pack.
- Windows 10 or Windows 11.

### Build

Build the C# implementation:

```powershell
git clone https://github.com/Robbson/micro-task-pager.git
cd micro-task-pager
dotnet build csharp-winforms/MicroTaskPager.csproj
```

Alternatively, open `MicroTaskPager.sln` in Visual Studio and build the solution.

Build the Rust implementations (from their own directories):

```powershell
cd rust-winui
cargo build --release

cd ../rust-winsafe
cargo build --release
```

See [`rust-winui/ARCHITECTURE.md`](rust-winui/ARCHITECTURE.md) for the WinUI build prerequisites, which
include the MSVC build tools, the Windows SDK, and the Windows App Runtime 2.4 or later framework
package. The winsafe build needs only the MSVC build tools and Windows SDK.

### Run from source

`run.ps1` builds and starts one implementation from the repository root, so there is no need to
change into the project directories:

```powershell
pwsh .\run.ps1                    # C# WinForms (default)
pwsh .\run.ps1 winui              # Rust WinUI 3
pwsh .\run.ps1 winsafe            # native winsafe
pwsh .\run.ps1 winui -Configuration release
pwsh .\run.ps1 winforms -NoBuild  # start an existing build
```

The application is started in its own process so the terminal stays usable. Add `-Wait` to keep the
script attached until the application exits. Run only one pager at a time: all implementations share
a single-instance mutex and the topmost/pinning state.

### Create release packages

Build one or more production releases from the repository root:

```powershell
pwsh .\build.ps1                    # all three implementations
pwsh .\build.ps1 -Target winforms   # C# WinForms only
pwsh .\build.ps1 -Target winui      # Rust WinUI 3 only
pwsh .\build.ps1 -Target winsafe    # native winsafe only
```

The script writes each release to its own folder under `prod\` (`prod\winforms`, `prod\winui`, and
`prod\winsafe`) and creates a versioned ZIP archive there. The C# build automatically increments the
version in `csharp-winforms\version.txt`; the Rust archives use the version from their `Cargo.toml`.

`csharp-winforms\Publish.cmd` remains as a thin wrapper for building only the WinForms release.

## Feedback

Bug reports and feature requests are welcome in the [issue tracker](https://github.com/Robbson/micro-task-pager/issues).

## Acknowledgements

- [Switchie](https://github.com/darkguy2008/Switchie) — the original project this app is based on.
- [VirtualDesktop](https://github.com/MScholtes/VirtualDesktop) — virtual desktop management for recent Windows 10 and 11 versions.

## License

Released under the [MIT License](LICENSE).

Made with ♥ in Munich, Germany.
