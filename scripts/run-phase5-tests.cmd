@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0run-phase5-tests.ps1" %*
exit /b %ERRORLEVEL%
