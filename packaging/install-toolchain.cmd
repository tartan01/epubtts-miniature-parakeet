@echo off
setlocal
set ROOT=%~dp0..
set RUSTUP_HOME=%ROOT%\.rustup
set CARGO_HOME=%ROOT%\.cargo
set RUSTUP_DIST_SERVER=https://rsproxy.cn
set RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup

if exist "%CARGO_HOME%\bin\rustc.exe" goto have
if not exist "%CARGO_HOME%\bin" mkdir "%CARGO_HOME%\bin"

echo Downloading rustup-init (x86_64-pc-windows-gnu) via rsproxy mirror ...
curl.exe -L --fail --connect-timeout 20 -o "%CARGO_HOME%\bin\rustup-init.exe" https://rsproxy.cn/rustup/dist/x86_64-pc-windows-gnu/rustup-init.exe
if errorlevel 1 (
  echo Mirror failed, falling back to static.rust-lang.org ...
  curl.exe -L --fail --connect-timeout 20 -o "%CARGO_HOME%\bin\rustup-init.exe" https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-gnu/rustup-init.exe
)
if errorlevel 1 goto fail

"%CARGO_HOME%\bin\rustup-init.exe" -y --no-modify-path --default-host x86_64-pc-windows-gnu --default-toolchain stable-x86_64-pc-windows-gnu --profile minimal
if errorlevel 1 goto fail

:have
echo.
"%CARGO_HOME%\bin\rustc.exe" --version
echo Toolchain ready. Next:  powershell -ExecutionPolicy Bypass -File mini\build.ps1
exit /b 0

:fail
echo Toolchain install failed. Check network or install Rust manually.
exit /b 1
