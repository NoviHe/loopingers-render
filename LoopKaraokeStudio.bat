@echo off
title LoopKaraoke Studio
cd /d "%~dp0"

echo ========================================================
echo   LoopKaraoke Studio - Memulai Aplikasi Desktop...
echo ========================================================
echo.

where python >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] Python tidak ditemukan di sistem ini!
    echo Silakan install Python 3.10+ dari https://www.python.org/
    pause
    exit /b 1
)

python desktop.py
if %errorlevel% neq 0 (
    echo.
    echo Aplikasi ditutup dengan kode error %errorlevel%.
    pause
)
