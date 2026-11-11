$projectDir = $PSScriptRoot
$versionFile = Join-Path $projectDir 'version.txt'
$generatedFile = Join-Path $projectDir 'GeneratedVersion.cs'

$version = (Get-Content -LiteralPath $versionFile).Trim()

$parts = $version.Split('.')

if ($parts.Length -lt 3 -or $parts.Length -gt 4)
{
    throw "Version has to be in format Major.Minor.Patch or Major.Minor.Patch.Build"
}

if ($parts.Length -eq 3)
{
    $parts += '0'
}
else
{
    $parts[3] = ([int]$parts[3] + 1).ToString()
}

$newVersion = $parts -join '.'

Set-Content -LiteralPath $versionFile $newVersion

@"
using System.Reflection;

[assembly: AssemblyVersion("$newVersion")]
[assembly: AssemblyFileVersion("$newVersion")]
[assembly: AssemblyInformationalVersion("$newVersion")]
"@ | Set-Content -LiteralPath $generatedFile
