@echo off
setlocal
cd /d "%~dp0"
echo.
echo  FloatNote - portable build
echo  ==========================

where node >nul 2>nul || (echo [!] Node.js is missing: https://nodejs.org & goto :fail)
where cargo >nul 2>nul || (echo [!] Rust is missing: https://rustup.rs & goto :fail)

echo [1/2] Installing JS dependencies...
call npm ci || goto :fail

echo [2/2] Building FloatNote.exe (first build takes a few minutes)...
call npm run app:build || goto :fail

if not exist dist-portable mkdir dist-portable
copy /Y "src-tauri\target\release\floatnote.exe" "dist-portable\FloatNote.exe" >nul || goto :fail

echo.
echo  Done:  dist-portable\FloatNote.exe
echo  No installer. Copy it anywhere. Settings are stored in a "floatnote-data" folder next to it.
echo.
pause
exit /b 0

:fail
echo.
echo  Build failed. See the messages above.
pause
exit /b 1
