@echo off
setlocal enabledelayedexpansion

set ROOT_DIR=%~dp0
if exist "%ROOT_DIR%target\release\download-host.exe" (
  set TARGET_EXE=%ROOT_DIR%target\release\download-host.exe
) else (
  set TARGET_EXE=%ROOT_DIR%target\debug\download-host.exe
)
set MANIFEST_PATH=%ROOT_DIR%extension\org.open_download_manager.host.json
set ESCAPED_EXE=%TARGET_EXE:\=\\%

echo Creating Chrome Native Messaging Host Manifest...
(
  echo {
  echo   "name": "org.open_download_manager.host",
  echo   "description": "Hopper Download Manager Native Host",
  echo   "path": "%ESCAPED_EXE%",
  echo   "type": "stdio",
  echo   "allowed_origins": [
  echo     "chrome-extension://gaokhlidbpdojkakefeehfpknnnlgkel/",
  echo     "chrome-extension://diofdkbmliadkdmlholappfnipkhgmin/"
  echo   ]
  echo }
) > "%MANIFEST_PATH%"

echo Registering in Windows Registry for Google Chrome, Edge, and Brave...
reg add "HKCU\Software\Google\Chrome\NativeMessagingHosts\org.open_download_manager.host" /ve /t REG_SZ /d "%MANIFEST_PATH%" /f >nul 2>&1
reg add "HKCU\Software\Microsoft\Edge\NativeMessagingHosts\org.open_download_manager.host" /ve /t REG_SZ /d "%MANIFEST_PATH%" /f >nul 2>&1
reg add "HKCU\Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\org.open_download_manager.host" /ve /t REG_SZ /d "%MANIFEST_PATH%" /f >nul 2>&1

echo.
echo ========================================================
echo  Hopper Native Host successfully registered!
echo  Manifest: %MANIFEST_PATH%
echo  Binary:   %TARGET_EXE%
echo  Allowed:  chrome-extension://gaokhlidbpdojkakefeehfpknnnlgkel/
echo ========================================================
echo.
