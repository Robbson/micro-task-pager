#Requires -Version 7.0

<#
.SYNOPSIS
    Builds the production releases for the Micro Task Pager implementations.

.DESCRIPTION
    Produces a self-contained release folder and a versioned ZIP archive for each
    selected implementation under prod\:

    - csharp-winforms: .NET Framework 4.8 WinForms build (prod\winforms)
    - rust-winui:      Rust WinUI 3 build (prod\winui, prod\winui-arm64)
    - rust-winsafe:    native Win32 winsafe build (prod\winsafe, prod\winsafe-arm64)

    The Rust builds produce both x64 and ARM64 artifacts by default. Use
    -Architecture to restrict a build to one architecture.

    The WinForms build increments the build number in version.txt on every run.
    The Rust builds use the version from their Cargo.toml.

.PARAMETER Target
    Which implementation to build: all (default), winforms, winui, or winsafe.

.PARAMETER Architecture
    Which Rust architecture to build: all (default), x64, or arm64. The WinForms
    target always builds for its configured .NET Framework platform.

.PARAMETER NoZip
    Build and stage the release but skip ZIP creation.

.EXAMPLE
    pwsh .\build.ps1
    Builds all production releases.

.EXAMPLE
    pwsh .\build.ps1 -Target rust-winui
    Builds only the Rust WinUI 3 release.

.EXAMPLE
    pwsh .\build.ps1 -Target winsafe
    Builds only the native winsafe release.

.EXAMPLE
    pwsh .\build.ps1 -Target winforms -NoZip
    Builds the WinForms release without creating the ZIP archive.

.EXAMPLE
    pwsh .\build.ps1 -Target winui -Architecture arm64
    Builds only the ARM64 WinUI 3 release.
#>
[CmdletBinding()]
param(
    [ValidateSet('all', 'winforms', 'winui', 'rust-winui', 'winsafe', 'rust-winsafe')]
    [string]$Target = 'all',

    [ValidateSet('all', 'x64', 'arm64')]
    [string]$Architecture = 'all',

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

function Get-RustTargets {
    param([Parameter(Mandatory)][string]$Architecture)

    $targets = @(
        [pscustomobject]@{
            Name          = 'x64'
            Triple        = 'x86_64-pc-windows-msvc'
            StageSuffix   = ''
            ArchiveSuffix = ''
        }
        [pscustomobject]@{
            Name          = 'arm64'
            Triple        = 'aarch64-pc-windows-msvc'
            StageSuffix   = '-arm64'
            ArchiveSuffix = '-arm64'
        }
    )

    if ($Architecture -eq 'all') {
        return $targets
    }
    return $targets | Where-Object { $_.Name -eq $Architecture }
}

function Assert-RustTarget {
    param([Parameter(Mandatory)][string]$Triple)

    $installed = & rustup target list --installed
    if ($installed -notcontains $Triple) {
        throw "Rust target '$Triple' is not installed. Run: rustup target add $Triple"
    }
}

function Build-Rust {
    param(
        [Parameter(Mandatory)][string]$DisplayName,
        [Parameter(Mandatory)][string]$ProjectDir,
        [Parameter(Mandatory)][string]$PackageName,
        [Parameter(Mandatory)][string]$BinaryName,
        [Parameter(Mandatory)][string]$StageName,
        [Parameter(Mandatory)][string]$ArchivePrefix,
        [Parameter(Mandatory)][string]$Architecture
    )

    $targets = @(Get-RustTargets -Architecture $Architecture)
    foreach ($target in $targets) {
        Assert-RustTarget -Triple $target.Triple
    }

    Push-Location $root
    try {
        $manifest = Join-Path $ProjectDir 'Cargo.toml'
        $metadata = cargo metadata --manifest-path $manifest --no-deps --format-version 1 | ConvertFrom-Json
        $package = $metadata.packages | Where-Object { $_.name -eq $PackageName }
        if (-not $package) {
            throw "Unable to read the $PackageName version from Cargo metadata."
        }
        $version = $package.version
        $targetRoot = $metadata.target_directory

        foreach ($target in $targets) {
            $stage = Join-Path $prodRoot ($StageName + $target.StageSuffix)

            Write-Host "==> Building Rust $DisplayName release ($($target.Name))"
            Reset-Stage -Path $stage

            cargo build --release --target $target.Triple --package $PackageName --manifest-path $manifest

            $builtExe = Join-Path $targetRoot "$($target.Triple)/release/$BinaryName.exe"
            if (-not (Test-Path -LiteralPath $builtExe)) {
                throw "Expected release executable not found: $builtExe"
            }
            Copy-Item -LiteralPath $builtExe -Destination (Join-Path $stage 'MicroTaskPager.exe') -Force

            if ($NoZip) {
                Write-Host "Skipped ZIP creation for $stage"
                continue
            }

            $files = @((Join-Path $stage 'MicroTaskPager.exe')) + $sharedDocs
            $archiveName = "$ArchivePrefix$($target.ArchiveSuffix)-v$version.zip"
            New-ReleaseArchive -Stage $stage -ArchiveName $archiveName -Files $files
        }
    }
    finally {
        Pop-Location
    }
}

switch ($Target) {
    'winforms' {
        Build-WinForms
    }
    'winui' {
        Build-Rust -DisplayName 'WinUI 3' `
            -ProjectDir (Join-Path $root 'rust-winui') `
            -PackageName 'micro-task-pager-winui' `
            -BinaryName 'micro-task-pager-winui' `
            -StageName 'winui' `
            -ArchivePrefix 'MicroTaskPager-winui' `
            -Architecture $Architecture
    }
    'rust-winui' {
        Build-Rust -DisplayName 'WinUI 3' `
            -ProjectDir (Join-Path $root 'rust-winui') `
            -PackageName 'micro-task-pager-winui' `
            -BinaryName 'micro-task-pager-winui' `
            -StageName 'winui' `
            -ArchivePrefix 'MicroTaskPager-winui' `
            -Architecture $Architecture
    }
    'winsafe' {
        Build-Rust -DisplayName 'winsafe native' `
            -ProjectDir (Join-Path $root 'rust-winsafe') `
            -PackageName 'micro-task-pager-winsafe' `
            -BinaryName 'micro-task-pager-winsafe' `
            -StageName 'winsafe' `
            -ArchivePrefix 'MicroTaskPager-winsafe' `
            -Architecture $Architecture
    }
    'rust-winsafe' {
        Build-Rust -DisplayName 'winsafe native' `
            -ProjectDir (Join-Path $root 'rust-winsafe') `
            -PackageName 'micro-task-pager-winsafe' `
            -BinaryName 'micro-task-pager-winsafe' `
            -StageName 'winsafe' `
            -ArchivePrefix 'MicroTaskPager-winsafe' `
            -Architecture $Architecture
    }
    'all' {
        Build-WinForms
        Build-Rust -DisplayName 'WinUI 3' `
            -ProjectDir (Join-Path $root 'rust-winui') `
            -PackageName 'micro-task-pager-winui' `
            -BinaryName 'micro-task-pager-winui' `
            -StageName 'winui' `
            -ArchivePrefix 'MicroTaskPager-winui' `
            -Architecture $Architecture
        Build-Rust -DisplayName 'winsafe native' `
            -ProjectDir (Join-Path $root 'rust-winsafe') `
            -PackageName 'micro-task-pager-winsafe' `
            -BinaryName 'micro-task-pager-winsafe' `
            -StageName 'winsafe' `
            -ArchivePrefix 'MicroTaskPager-winsafe' `
            -Architecture $Architecture
    }
    default {
        throw "Unknown target '$Target'."
    }
}

Write-Host ''
Write-Host 'Release output:'
Get-ChildItem -LiteralPath $prodRoot -Directory -ErrorAction SilentlyContinue |
    ForEach-Object { Write-Host "  $($_.FullName)" }
