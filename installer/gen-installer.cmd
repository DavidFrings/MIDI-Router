@echo off
setlocal
rem Builds the router and compiles installer\midi-router-installer.exe.
rem Version and project path go to ISCC as /D defines, so installer.iss is never modified.
cd /d "%~dp0"
for %%a in ("%~dp0..") do set "PROJECT_DIR=%%~fa"

cargo build --release || exit /b 1

set "VERSION="
for /f "tokens=2 delims== " %%A in ('findstr /R "^version *= *" "%PROJECT_DIR%\Cargo.toml"') do (
    set "VERSION=%%~A"
    goto :found
)
:found
if "%VERSION%"=="" (
    echo Error: No version found in Cargo.toml
    exit /b 1
)

set "ISCC=%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"
if not exist "%ISCC%" (
    echo Error: Inno Setup 6 not found at "%ISCC%"
    exit /b 1
)

echo Version: %VERSION%
echo Project: %PROJECT_DIR%
"%ISCC%" "/DMyAppVersion=%VERSION%" "/DProjectDir=%PROJECT_DIR%" installer.iss || exit /b 1
echo Installer: %~dp0midi-router-installer.exe
