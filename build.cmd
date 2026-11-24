@echo off
setlocal
rem ---------------------------------------------------------------
rem bookctl one-click build + regression (AGENTS.md section 3).
rem dotnet is NOT on PATH on this machine, so use the absolute path.
rem Keep this file pure ASCII (see start.cmd for the reason).
rem ---------------------------------------------------------------
set "ROOT=%~dp0"
set "DN=C:\Program Files\dotnet\dotnet.exe"
if not exist "%DN%" (
    echo [bookctl] .NET SDK not found: %DN%
    exit /b 1
)

echo [1/3] Build solution ...
"%DN%" build "%ROOT%bookctl.sln" -c Debug -v q --nologo
if errorlevel 1 goto fail

echo [2/3] Rebuild App (Core.dll cache trap, see AGENTS.md 3) ...
"%DN%" build "%ROOT%src\BookCtl.App\BookCtl.App.csproj" -c Debug -v q --nologo
if errorlevel 1 goto fail

echo [3/3] Regression with mock engine ...
set "EPUB="
for %%F in ("%ROOT%testdata\*.epub") do if not defined EPUB set "EPUB=%%F"
if not defined EPUB (
    echo [bookctl] No test .epub under testdata, regression skipped.
    goto done
)
set "BOOKCTL_WORKSPACE=%ROOT%testdata\ws-regression"
"%ROOT%src\BookCtl.Cli\bin\Debug\net10.0\bookctl.exe" run --input "%EPUB%" --engine mock
if errorlevel 1 goto fail

:done
echo [bookctl] OK. Launch the GUI with start.cmd
exit /b 0
:fail
echo [bookctl] FAILED.
exit /b 1
