@echo off
rem Builds FLP_next in release and assembles dist\FLP_next + dist\FLP_next.zip.
rem Double-click to run; the window stays open to show the result.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\build-dist.ps1" %*
set code=%ERRORLEVEL%
echo.
pause
exit /b %code%
