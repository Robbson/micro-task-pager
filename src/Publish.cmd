@echo off
cd /d "%~dp0"

dotnet restore
if errorlevel 1 exit /b %errorlevel%

dotnet publish -c Release -o ..\prod
if errorlevel 1 exit /b %errorlevel%

del /q ..\prod\*.pdb
del /q ..\prod\*.config

for /f "usebackq delims=" %%v in ("%~dp0version.txt") do set "VERSION=%%v"

powershell -NoProfile -ExecutionPolicy Bypass -Command "$ErrorActionPreference = 'Stop'; Compress-Archive -Path '..\prod\MicroTaskPager.exe', '..\README.md', '..\LICENSE', '..\CHANGELOG.md' -DestinationPath ('..\prod\MicroTaskPager-v' + $env:VERSION + '.zip') -Force"
if errorlevel 1 exit /b %errorlevel%

echo Created ..\prod\MicroTaskPager-v%VERSION%.zip
