@echo off
setlocal
rem ---------------------------------------------------------------
rem bookctl one-click GUI launcher. The ONLY entry point you need.
rem Priority: portable release > dev Debug build > dev Release build.
rem Extra args are forwarded, e.g.:
rem   start.cmd --detailed
rem   start.cmd "C:\books\my.epub" --auto-start
rem Keep this file pure ASCII: cmd.exe parses batch files with the
rem system ANSI code page, and this workspace contains non-ASCII paths.
rem ---------------------------------------------------------------
set "ROOT=%~dp0"
set "EXE="

for /d %%D in ("%ROOT%dist\*-win-x64*") do (
    if exist "%%D\BookCtl.App.exe" set "EXE=%%D\BookCtl.App.exe"
)

if not defined EXE if exist "%ROOT%src\BookCtl.App\bin\x64\Debug\net10.0-windows10.0.19041.0\win-x64\BookCtl.App.exe" set "EXE=%ROOT%src\BookCtl.App\bin\x64\Debug\net10.0-windows10.0.19041.0\win-x64\BookCtl.App.exe"
if not defined EXE if exist "%ROOT%src\BookCtl.App\bin\Debug\net10.0-windows10.0.19041.0\win-x64\BookCtl.App.exe" set "EXE=%ROOT%src\BookCtl.App\bin\Debug\net10.0-windows10.0.19041.0\win-x64\BookCtl.App.exe"
if not defined EXE if exist "%ROOT%src\BookCtl.App\bin\Release\net10.0-windows10.0.19041.0\win-x64\BookCtl.App.exe" set "EXE=%ROOT%src\BookCtl.App\bin\Release\net10.0-windows10.0.19041.0\win-x64\BookCtl.App.exe"

if not defined EXE (
    echo [bookctl] No built exe found. Run build.cmd first.
    echo [bookctl] Expected: dist\*-win-x64\BookCtl.App.exe
    pause
    exit /b 1
)
echo [bookctl] Launching: %EXE%
start "" "%EXE%" %*
endlocal
