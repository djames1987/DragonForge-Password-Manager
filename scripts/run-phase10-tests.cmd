@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0run-phase10-tests.ps1" %*
exit /b %ERRORLEVEL%
