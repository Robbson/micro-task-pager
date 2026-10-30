# AGENTS.md

Instructions for AI agents working in this repository.

## Project

Micro Task Pager is a Windows-only .NET 4.8 application using Windows Forms only. It uses undocumented Windows virtual-desktop COM interfaces, so changes in `src/Core/API/VirtualDesktopAPI-*.cs` must be handled carefully.

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

## Build and verification

```powershell
dotnet build src/MicroTaskPager.csproj -c Debug
```

- The project has no automated test project. Verify changes by building and, where relevant, by
  manually testing the pager UI.
- The Debug executable is written to `src/bin/Debug/net48/MicroTaskPager.exe`.
- Do not commit unless the user explicitly asks.

## Repository layout

- `src/Core/API/` - virtual-desktop COM interop. There are four platform-specific variants:
  `Win10`, `Win10LTSC`, `Win11`, and `Win11_` (Windows 11 24H2+). Keep shared changes in sync across
  all four files.
- `src/Core/` - window enumeration, types, DPI helpers.
- `src/GUI/` - pager, desktops, settings dialog, about dialog.

## Notes

- Virtual-desktop COM interfaces are undocumented and version-sensitive. Test interop changes on the
  supported Windows versions rather than assuming one layout.
- Windows belonging to the pager process are intentionally excluded from the pager.

