@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0run-phase9-tests.ps1" %*
exit /b %ERRORLEVEL%
