@echo off
cd /d "%~dp0"

set "VS_DEV_CMD=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"
if not exist "%VS_DEV_CMD%" (
  echo ERROR: Visual Studio C++ Build Tools not found.
  pause
  exit /b 1
)

call "%VS_DEV_CMD%" -arch=x64 -host_arch=x64 >nul
if errorlevel 1 (
  echo ERROR: Failed to load the MSVC build environment.
  pause
  exit /b 1
)
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

if not exist "node_modules" (
  echo Installing frontend dependencies for the first run...
  call npm install
  if errorlevel 1 (
    echo ERROR: npm install failed.
    pause
    exit /b 1
  )
)

echo.
echo ========================================
echo AI Personal Workbench - Desktop Dev Mode
echo ========================================
echo Vue, TypeScript and CSS: hot reload
echo Rust: incremental rebuild and restart
echo Full exit: use Quit from the tray menu
echo.

call npm run dev:desktop
set "DEV_EXIT_CODE=%ERRORLEVEL%"
if not "%DEV_EXIT_CODE%"=="0" (
  echo.
  echo Dev mode exited with code %DEV_EXIT_CODE%.
  pause
)
exit /b %DEV_EXIT_CODE%
