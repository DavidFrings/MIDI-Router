cargo build --release

@echo off
setlocal enabledelayedexpansion

set "PROJECT_DIR=%cd%"

set "VERSION="
for /f "tokens=2 delims== " %%A in ('findstr /R "^version *= *" Cargo.toml') do (
    set "line=%%~A"
    set "line=!line:"=!"
    set "VERSION=!line!"
    goto :found
)
:found

if "%VERSION%"=="" (
    echo Error: No version found in Cargo.toml
    exit /b 1
)

set "TEMP_FILE=%TEMP%\installer_tmp.iss"

(for /f "usebackq delims=" %%L in ("installer.iss") do (
    set "line=%%L"
    echo !line! | findstr /B /C:"#define MyAppVersion" >nul
    if errorlevel 1 (
        echo !line! | findstr /B /C:"#define ProjectDir" >nul
        if errorlevel 1 (
            echo !line!
        )
    )
)) > "%TEMP_FILE%_body"

(
    echo #define MyAppVersion "%VERSION%"
    echo #define ProjectDir "%PROJECT_DIR%"
    type "%TEMP_FILE%_body"
) > "%TEMP_FILE%"

move /Y "%TEMP_FILE%" installer.iss > nul
del "%TEMP_FILE%_body" > nul 2> nul

echo Version: %VERSION%
echo Projektpfad: %PROJECT_DIR%


start "C:\Program Files (x86)\Inno Setup 6\Compil32.exe" "installer.iss"
