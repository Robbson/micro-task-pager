
# Building from source

## Prerequisites

- Visual Studio 2022 with the **.NET desktop development** workload, or the .NET SDK with the .NET Framework 4.8 targeting pack.
- Windows 10 or Windows 11.

## Build

```powershell
git clone https://github.com/Robbson/micro-task-pager.git
cd micro-task-pager
dotnet build csharp-winforms/MicroTaskPager.csproj
```

Alternatively, open `MicroTaskPager.sln` in Visual Studio and build the solution.

### Run from source

`run.ps1` builds and starts the implementation from the repository root, so there is no need to
change into the project directories:

```powershell
pwsh .\run.ps1
```

The application is started in its own process so the terminal stays usable. Add `-Wait` to keep the
script attached until the application exits. Run only one pager at a time: all implementations share
a single-instance mutex and the topmost/pinning state.

### Create release packages

Build one or more production releases from the repository root:

```powershell
pwsh .\build.ps1 -Target winforms
```

`csharp-winforms\Publish.cmd` remains as a thin wrapper for building only the WinForms release.
