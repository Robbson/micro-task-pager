@echo off
pwsh -NoProfile -ExecutionPolicy Bypass -File "%~dp0..\build.ps1" -Target winforms
if errorlevel 1 exit /b %errorlevel%
