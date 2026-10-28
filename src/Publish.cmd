@echo off
cd /d "%~dp0"

rem Remove previous build output but keep existing release archives.
if not exist ..\prod mkdir ..\prod
del /q ..\prod\MicroTaskPager.* 2>nul
del /q ..\prod\*.dll 2>nul
del /q ..\prod\*.pdb 2>nul

dotnet restore
if errorlevel 1 exit /b %errorlevel%

rem Framework-dependent publish: the .NET 8 Desktop Runtime is a prerequisite.
dotnet publish -c Release -o ..\prod --self-contained false
if errorlevel 1 exit /b %errorlevel%

del /q ..\prod\*.pdb

for /f "usebackq delims=" %%v in ("%~dp0version.txt") do set "VERSION=%%v"

powershell -NoProfile -ExecutionPolicy Bypass -Command "$ErrorActionPreference = 'Stop'; Compress-Archive -Path '..\prod\MicroTaskPager.exe', '..\prod\MicroTaskPager.deps.json', '..\prod\MicroTaskPager.runtimeconfig.json', '..\prod\*.dll', '..\README.md', '..\LICENSE', '..\CHANGELOG.md', '..\THIRD-PARTY-NOTICES.md' -DestinationPath ('..\prod\MicroTaskPager-v' + $env:VERSION + '.zip') -Force"
if errorlevel 1 exit /b %errorlevel%

echo Created ..\prod\MicroTaskPager-v%VERSION%.zip
