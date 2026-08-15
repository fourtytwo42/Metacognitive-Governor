@echo off
setlocal
cd /d "%~dp0"

rem Start the local control plane if it is not already running. A second daemon
rem exits cleanly when the loopback port is owned by an existing installation.
start "MCG daemon" /min "%~dp0mcg-daemon.exe"
timeout /t 2 /nobreak >nul
start "MCG" "%~dp0mcg-desktop.exe"

endlocal
