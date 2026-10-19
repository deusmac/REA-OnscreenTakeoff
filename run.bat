@echo off
rem Builds (first run takes a few minutes) and starts OpenTakeoff.
setlocal
cd /d "%~dp0app"
rem Own build folder, whatever the machine-wide setting is. E: when present (C: is nearly full).
set "CARGO_TARGET_DIR=%~dp0app\target"
if exist E:\cargo-targets\ set "CARGO_TARGET_DIR=E:\cargo-targets\opentakeoff"
where cargo >nul 2>nul || (echo Rust is not installed. Install it from https://rustup.rs then run this again. & pause & exit /b 1)
cargo build --release -p rea-desktop || (echo Build failed. See the messages above. & pause & exit /b 1)
rem The PDF engine has to sit next to the program.
copy /y "%~dp0app\vendor\pdfium\pdfium.dll" "%CARGO_TARGET_DIR%\release\" >nul || (echo Could not copy pdfium.dll. & pause & exit /b 1)
start "" "%CARGO_TARGET_DIR%\release\rea-desktop.exe" %*
