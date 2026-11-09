#Requires -Version 7.0

<#
.SYNOPSIS
    Builds and starts one Micro Task Pager implementation from the repository root.

.DESCRIPTION
    Selects an implementation, builds it when needed, and starts the resulting
    executable without requiring a manual cd into the implementation directory.

    - winforms: csharp-winforms Debug/Release build
    - winui:    rust-winui debug/release build
    - winsafe:  rust-winsafe debug/release build

    The application is started in its own process so the terminal stays usable.
    Use -Wait when the script should block until the application exits.

.PARAMETER Target
    Which implementation to run: winforms (default), winui, rust-winui, winsafe,
    or rust-winsafe.

.PARAMETER Configuration
    Build and run the debug (default) or release configuration.

.PARAMETER NoBuild
    Start the existing executable without building it first.

.PARAMETER Wait
    Wait for the application to exit instead of returning immediately.

.EXAMPLE
    pwsh .\run.ps1
    Builds and starts the C# WinForms pager.

.EXAMPLE
    pwsh .\run.ps1 winui
    Builds and starts the Rust WinUI 3 pager.

.EXAMPLE
    pwsh .\run.ps1 winsafe -Configuration release
    Builds and starts the release build of the native winsafe pager.

.EXAMPLE
    pwsh .\run.ps1 winforms -NoBuild
    Starts the existing WinForms executable without rebuilding it.
#>
[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet('winforms', 'winui', 'rust-winui', 'winsafe', 'rust-winsafe')]
    [string]$Target = 'winforms',

    [ValidateSet('debug', 'release')]
    [string]$Configuration = 'debug',

    [switch]$NoBuild,

    [switch]$Wait
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

$root = $PSScriptRoot
$buildConfiguration = if ($Configuration -eq 'release') { 'Release' } else { 'Debug' }

switch ($Target) {
    'winforms' {
        $project = Join-Path $root 'csharp-winforms/MicroTaskPager.csproj'
        if (-not $NoBuild) {
            Write-Host "==> Building C# WinForms ($buildConfiguration)"
            dotnet build $project -c $buildConfiguration
        }
        $executable = Join-Path $root "csharp-winforms/bin/$buildConfiguration/net48/MicroTaskPager.exe"
    }
    { $_ -in 'winui', 'rust-winui' } {
        $projectDir = Join-Path $root 'rust-winui'
        if (-not $NoBuild) {
            Write-Host "==> Building Rust WinUI 3 ($Configuration)"
            Push-Location $projectDir
            try {
                if ($Configuration -eq 'release') {
                    cargo build --release
                }
                else {
                    cargo build
                }
            }
            finally {
                Pop-Location
            }
        }
        $executable = Join-Path $projectDir "target/$Configuration/micro-task-pager.exe"
    }
    { $_ -in 'winsafe', 'rust-winsafe' } {
        $projectDir = Join-Path $root 'rust-winsafe'
        if (-not $NoBuild) {
            Write-Host "==> Building Rust winsafe ($Configuration)"
            Push-Location $projectDir
            try {
                if ($Configuration -eq 'release') {
                    cargo build --release
                }
                else {
                    cargo build
                }
            }
            finally {
                Pop-Location
            }
        }
        $executable = Join-Path $projectDir "target/$Configuration/micro-task-pager.exe"
    }
    default {
        throw "Unknown target '$Target'."
    }
}

if (-not (Test-Path -LiteralPath $executable)) {
    throw "Executable not found: $executable. Build it first or omit -NoBuild."
}

Write-Host "==> Starting $executable"
$startParameters = @{
    FilePath         = $executable
    WorkingDirectory = $root
}

if ($Wait) {
    Start-Process @startParameters -Wait
}
else {
    Start-Process @startParameters | Out-Null
}
