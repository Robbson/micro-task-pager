#Requires -Version 7.0

<#
.SYNOPSIS
    Builds the production releases for both Micro Task Pager implementations.

.DESCRIPTION
    Produces a self-contained release folder and a versioned ZIP archive for each
    selected implementation under prod\:

    - csharp-winforms: .NET Framework 4.8 WinForms build (prod\winforms)
    - rust-winui:      Rust WinUI 3 build (prod\winui)

    The WinForms build increments the build number in version.txt on every run.
    The WinUI build uses the version from Cargo.toml.

.PARAMETER Target
    Which implementation to build: all (default), winforms, or winui.

.PARAMETER NoZip
    Build and stage the release but skip ZIP creation.

.EXAMPLE
    pwsh .\build.ps1
    Builds both production releases.

.EXAMPLE
    pwsh .\build.ps1 -Target rust-winui
    Builds only the Rust WinUI 3 release.

.EXAMPLE
    pwsh .\build.ps1 -Target winforms -NoZip
    Builds the WinForms release without creating the ZIP archive.
#>
[CmdletBinding()]
param(
    [ValidateSet('all', 'winforms', 'winui', 'rust-winui')]
    [string]$Target = 'all',

    [switch]$NoZip
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

$root = $PSScriptRoot
$prodRoot = Join-Path $root 'prod'
$sharedDocs = @(
    (Join-Path $root 'README.md')
    (Join-Path $root 'LICENSE')
    (Join-Path $root 'CHANGELOG.md')
    (Join-Path $root 'THIRD-PARTY-NOTICES.md')
) | Where-Object { Test-Path -LiteralPath $_ }

function Reset-Stage {
    param([Parameter(Mandatory)][string]$Path)

    if (Test-Path -LiteralPath $Path) {
        # Keep previously created archives; remove only staged build output.
        Get-ChildItem -LiteralPath $Path -File |
            Where-Object { $_.Extension -ne '.zip' } |
            Remove-Item -Force
        Get-ChildItem -LiteralPath $Path -Directory |
            Remove-Item -Recurse -Force
    }
    else {
        New-Item -ItemType Directory -Path $Path -Force | Out-Null
    }
}

function New-ReleaseArchive {
    param(
        [Parameter(Mandatory)][string]$Stage,
        [Parameter(Mandatory)][string]$ArchiveName,
        [Parameter(Mandatory)][string[]]$Files
    )

    $existing = $Files | Where-Object { Test-Path -LiteralPath $_ }
    if (-not $existing) {
        throw "No release files found in '$Stage'."
    }

    $archive = Join-Path $Stage $ArchiveName
    Compress-Archive -Path $existing -DestinationPath $archive -Force
    Write-Host "Created $archive"
}

function Build-WinForms {
    $projectDir = Join-Path $root 'csharp-winforms'
    $project = Join-Path $projectDir 'MicroTaskPager.csproj'
    $stage = Join-Path $prodRoot 'winforms'

    Write-Host '==> Building C# WinForms release'
    Reset-Stage -Path $stage

    dotnet publish $project -c Release -o $stage
    Get-ChildItem -LiteralPath $stage -Filter '*.pdb' -File -ErrorAction SilentlyContinue |
        Remove-Item -Force

    if ($NoZip) {
        Write-Host "Skipped ZIP creation for $stage"
        return
    }

    $version = (Get-Content -LiteralPath (Join-Path $projectDir 'version.txt') -Raw).Trim()
    $files = @(
        (Join-Path $stage 'MicroTaskPager.exe')
        (Join-Path $stage 'MicroTaskPager.exe.config')
    ) + @(
        Get-ChildItem -LiteralPath $stage -Filter '*.dll' -File -ErrorAction SilentlyContinue |
            ForEach-Object FullName
    ) + $sharedDocs

    New-ReleaseArchive -Stage $stage -ArchiveName "MicroTaskPager-winforms-v$version.zip" -Files $files
}

function Build-WinUi {
    $projectDir = Join-Path $root 'rust-winui'
    $stage = Join-Path $prodRoot 'winui'

    Write-Host '==> Building Rust WinUI 3 release'
    Reset-Stage -Path $stage

    Push-Location $projectDir
    try {
        cargo build --release

        $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
        $package = $metadata.packages | Where-Object { $_.name -eq 'micro-task-pager' }
        if (-not $package) {
            throw 'Unable to read the micro-task-pager version from Cargo metadata.'
        }
        $version = $package.version

        $builtExe = Join-Path $projectDir 'target/release/micro-task-pager.exe'
        if (-not (Test-Path -LiteralPath $builtExe)) {
            throw "Expected release executable not found: $builtExe"
        }
        Copy-Item -LiteralPath $builtExe -Destination (Join-Path $stage 'MicroTaskPager.exe') -Force
    }
    finally {
        Pop-Location
    }

    if ($NoZip) {
        Write-Host "Skipped ZIP creation for $stage"
        return
    }

    $files = @((Join-Path $stage 'MicroTaskPager.exe')) + $sharedDocs
    New-ReleaseArchive -Stage $stage -ArchiveName "MicroTaskPager-winui-v$version.zip" -Files $files
}

switch ($Target) {
    'winforms' {
        Build-WinForms
    }
    'winui' {
        Build-WinUi
    }
    'rust-winui' {
        Build-WinUi
    }
    'all' {
        Build-WinForms
        Build-WinUi
    }
    default {
        throw "Unknown target '$Target'."
    }
}

Write-Host ''
Write-Host 'Release output:'
Get-ChildItem -LiteralPath $prodRoot -Directory -ErrorAction SilentlyContinue |
    ForEach-Object { Write-Host "  $($_.FullName)" }
