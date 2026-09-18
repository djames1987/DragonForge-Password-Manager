@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0run-phase8-tests.ps1" %*
exit /b %ERRORLEVEL%
