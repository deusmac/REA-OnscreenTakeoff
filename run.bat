@echo off
rem Builds (first run takes a few minutes) and starts REA Takeoff.
setlocal
cd /d "%~dp0app"
rem Keep build output inside this project, whatever the machine-wide setting is.
set "CARGO_TARGET_DIR=%~dp0app\target"
where cargo >nul 2>nul || (echo Rust is not installed. Install it from https://rustup.rs then run this again. & pause & exit /b 1)
cargo build --release -p rea-desktop || (echo Build failed. See the messages above. & pause & exit /b 1)
start "" "%CARGO_TARGET_DIR%\release\rea-desktop.exe"
