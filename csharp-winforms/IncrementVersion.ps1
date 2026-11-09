$version = (Get-Content version.txt).Trim()

$parts = $version.Split('.')

if ($parts.Length -ne 4)
{
    throw "Version has to be in format Major.Minor.Patch.Build"
}

$parts[3] = ([int]$parts[3] + 1).ToString()

$newVersion = $parts -join '.'

Set-Content version.txt $newVersion

@"
using System.Reflection;

[assembly: AssemblyVersion("$newVersion")]
[assembly: AssemblyFileVersion("$newVersion")]
[assembly: AssemblyInformationalVersion("$newVersion")]
"@ | Set-Content GeneratedVersion.cs
