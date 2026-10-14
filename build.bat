@echo off
setlocal EnableExtensions

rem LoopKaraoke Studio - build and package for Windows.
rem Usage: build.bat [vulkan|cuda]
rem   (no argument)  Vulkan if the Vulkan SDK is installed, otherwise Whisper runs on the CPU
rem   vulkan         Whisper on AMD and NVIDIA GPUs (needs the Vulkan SDK)
rem   cuda           Whisper on NVIDIA GPUs (needs the CUDA Toolkit)

cd /d "%~dp0"

set "FEATURES="
rem Without an argument, use Vulkan (AMD and NVIDIA) when the Vulkan SDK is installed.
if "%~1"=="" if defined VULKAN_SDK set "FEATURES=--features vulkan"
if /i "%~1"=="vulkan" set "FEATURES=--features vulkan"
if /i "%~1"=="cuda" set "FEATURES=--features cuda"
if not "%~1"=="" if "%FEATURES%"=="" (
    echo [ERROR] Unknown option "%~1". Use: build.bat [vulkan^|cuda]
    exit /b 1
)

where cargo >nul 2>nul
if errorlevel 1 (
    echo [ERROR] Rust/Cargo not found. Install it from https://rustup.rs
    exit /b 1
)

where cmake >nul 2>nul
if errorlevel 1 (
    echo [ERROR] CMake not found. Install it from https://cmake.org/download
    exit /b 1
)

if not defined LIBCLANG_PATH (
    if exist "%ProgramFiles%\LLVM\bin\libclang.dll" set "LIBCLANG_PATH=%ProgramFiles%\LLVM\bin"
)
if not defined LIBCLANG_PATH (
    echo [ERROR] libclang.dll not found. Install LLVM ^(https://github.com/llvm/llvm-project/releases^)
    echo         or set LIBCLANG_PATH to the folder that contains libclang.dll.
    exit /b 1
)

echo.
echo [1/3] Building release executable %FEATURES%
cargo build --release %FEATURES%
if errorlevel 1 (
    echo [ERROR] Build failed.
    exit /b 1
)

echo.
echo [2/3] Preparing dist\LoopKaraokeStudio
set "DIST=%~dp0dist\LoopKaraokeStudio"
if exist "%~dp0dist" rmdir /s /q "%~dp0dist"
mkdir "%DIST%\assets" || exit /b 1

copy /y "target\release\loopkaraoke-studio.exe" "%DIST%\LoopKaraokeStudio.exe" >nul || exit /b 1
xcopy /e /i /q /y "assets\effects" "%DIST%\assets\effects" >nul || exit /b 1

set "FFMPEG_FOUND="
for /f "delims=" %%F in ('where ffmpeg 2^>nul') do if not defined FFMPEG_FOUND set "FFMPEG_FOUND=%%F"
if not defined FFMPEG_FOUND goto :no_ffmpeg
for %%D in ("%FFMPEG_FOUND%") do set "FFMPEG_DIR=%%~dpD"
copy /y "%FFMPEG_DIR%ffmpeg.exe" "%DIST%\" >nul
if exist "%FFMPEG_DIR%ffprobe.exe" copy /y "%FFMPEG_DIR%ffprobe.exe" "%DIST%\" >nul
echo FFmpeg copied from %FFMPEG_DIR%
goto :zip

:no_ffmpeg
echo [INFO] ffmpeg not found on PATH, so it is not bundled. The app downloads it automatically on first run.

:zip
echo.
echo [3/3] Creating zip
powershell -NoProfile -Command "Compress-Archive -Path '%DIST%' -DestinationPath '%~dp0dist\LoopKaraokeStudio-Windows-x64.zip' -Force"
if errorlevel 1 (
    echo [ERROR] Could not create the zip.
    exit /b 1
)

echo.
echo Done.
echo   Folder: %DIST%
echo   Zip:    %~dp0dist\LoopKaraokeStudio-Windows-x64.zip
endlocal
