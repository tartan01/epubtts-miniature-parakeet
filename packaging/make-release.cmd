@echo off
setlocal
rem ---------------------------------------------------------------
rem One entry point for both release shapes: portable zip + setup exe.
rem   make-release.cmd          standard portable (fully self-contained)
rem   make-release.cmd -Slim    slim portable (CLI tools need .NET 10)
rem Inno Setup is optional; if missing, the portable edition still ships.
rem ---------------------------------------------------------------
for %%I in ("%~dp0..") do set "ROOT=%%~fI"
set "SLIM="
if /i "%~1"=="-Slim" set "SLIM=-Slim"

echo [bookctl] Building portable edition ...
powershell -ExecutionPolicy Bypass -File "%ROOT%\packaging\build-portable.ps1" -Version 0.3.0 %SLIM%
if errorlevel 1 goto fail

set "ISCC=C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
if not exist "%ISCC%" (
    echo [bookctl] Inno Setup not found - portable edition is ready.
    echo [bookctl] For setup.exe: winget install JRSoftware.InnoSetup
    goto done
)
echo [bookctl] Building setup exe ...
"%ISCC%" "%ROOT%\packaging\bookctl.iss" /DMyVersion=0.3.0
if errorlevel 1 goto fail

:done
echo [bookctl] Release outputs: %ROOT%\dist\
exit /b 0
:fail
echo [bookctl] FAILED.
exit /b 1
