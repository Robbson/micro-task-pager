# AGENTS.md

Instructions for AI agents working in this repository.

## Project

Micro Task Pager is a Windows-only .NET 8 application (`net8.0-windows`) that combines
Windows Forms and WPF. It uses undocumented Windows virtual-desktop COM interfaces, so changes in
`src/Core/API/VirtualDesktopAPI-*.cs` must be handled carefully.

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
- The Debug executable is written to `src/bin/Debug/net8.0-windows/MicroTaskPager.exe`.
- Do not commit unless the user explicitly asks.

## Repository layout

- `src/Core/API/` - virtual-desktop COM interop. There are four platform-specific variants:
  `Win10`, `Win10LTSC`, `Win11`, and `Win11_` (Windows 11 24H2+). Keep shared changes in sync across
  all four files.
- `src/Core/` - window enumeration, types, DPI helpers.
- `src/GUI/` - pager, desktops, settings dialog, about dialog.
- `docs/NET8-migration.md` - details of the .NET Framework 4.8 to .NET 8 runtime fixes.

## Notes

- Virtual-desktop COM interfaces are undocumented and version-sensitive. Test interop changes on the
  supported Windows versions rather than assuming one layout.
- Windows belonging to the pager process are intentionally excluded from the pager.
- `IApplicationView` is declared as `InterfaceIsIUnknown` with explicit `IInspectable` methods
  because .NET 8 cannot marshal `InterfaceIsIInspectable`.
