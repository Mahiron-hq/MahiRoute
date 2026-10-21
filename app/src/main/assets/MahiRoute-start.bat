@echo off
rem SPDX-License-Identifier: AGPL-3.0-or-later
rem Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)
setlocal EnableExtensions EnableDelayedExpansion
chcp 65001 >nul

rem Starts the MahiRoute helper on a USB-connected phone.
rem Pass /auto to skip the confirmation. Set MAHIROUTE_LANG to en or ru to force the language.
set "PKG=io.github.mahiron_hq.mahiroute"
set "SCRIPT=/storage/emulated/0/Android/data/%PKG%/files/start-helper.sh"

set "LOCALE=en"
for /f "tokens=3" %%L in ('reg query "HKCU\Control Panel\International" /v LocaleName 2^>nul') do set "LOCALE=%%L"
if defined MAHIROUTE_LANG set "LOCALE=%MAHIROUTE_LANG%"
if /i "%LOCALE:~0,2%"=="ru" (call :messages_ru) else (call :messages_en)
title %M_TITLE%

echo.
echo  %M_HEAD%
echo  ----------------------------------------
echo.

set "ADB="
where adb >nul 2>&1 && set "ADB=adb"
if not defined ADB if exist "%~dp0platform-tools\adb.exe" set "ADB=%~dp0platform-tools\adb.exe"
if not defined ADB if exist "%LOCALAPPDATA%\Android\Sdk\platform-tools\adb.exe" set "ADB=%LOCALAPPDATA%\Android\Sdk\platform-tools\adb.exe"
if not defined ADB if defined ANDROID_HOME if exist "%ANDROID_HOME%\platform-tools\adb.exe" set "ADB=%ANDROID_HOME%\platform-tools\adb.exe"
if not defined ADB goto noadb

"%ADB%" start-server >nul 2>&1
echo  %M_CONNECT%
echo  %M_WAIT%

:wait
set "COUNT=0"
set "SERIAL="
set "UNAUTH="
for /f "skip=1 tokens=1,2" %%A in ('"%ADB%" devices') do (
    if "%%B"=="device" (
        set /a COUNT+=1
        set "SERIAL=%%A"
    )
    if "%%B"=="unauthorized" set "UNAUTH=1"
)
if "%COUNT%"=="1" goto ready
if not "%COUNT%"=="0" goto several
if defined UNAUTH if not defined TOLD (
    set "TOLD=1"
    echo  %M_AUTH%
)
ping -n 3 127.0.0.1 >nul
goto wait

:several
set /a TRIES+=1
if %TRIES% gtr 3 goto end
echo.
echo  %M_SEVERAL%
"%ADB%" devices
set "SERIAL="
set /p "SERIAL=%M_SERIAL%"
if not defined SERIAL goto several

:ready
echo.
echo  %M_PHONE% !SERIAL!
"%ADB%" -s !SERIAL! shell test -f %SCRIPT%
if errorlevel 1 goto noscript

echo.
echo  %M_COMMAND%
echo    adb -s !SERIAL! shell sh %SCRIPT%
echo.
if /i "%~1"=="/auto" goto run
set /p "DUMMY=%M_ENTER%"

:run
echo.
"%ADB%" -s !SERIAL! shell sh %SCRIPT%
if errorlevel 1 goto failed
echo.
echo  %M_DONE%
goto end

:noscript
echo.
echo  %M_NOSCRIPT%
goto end

:failed
echo.
echo  %M_FAILED%
goto end

:noadb
echo.
echo  %M_NOADB1%
echo  %M_NOADB2%

:end
echo.
pause
endlocal
exit /b

:messages_en
set "M_TITLE=MahiRoute: start the helper"
set "M_HEAD=MahiRoute: starting the helper from a computer"
set "M_CONNECT=Connect the phone with a cable. USB debugging must be turned on."
set "M_WAIT=Waiting for the phone..."
set "M_AUTH=Allow debugging on the phone screen and tick Always allow from this computer."
set "M_SEVERAL=Several devices are connected:"
set "M_SERIAL= Enter the serial number of the phone: "
set "M_PHONE=Phone:"
set "M_COMMAND=Command to run:"
set "M_ENTER= Press Enter to start the helper on the phone..."
set "M_DONE=Done. The helper is running and MahiRoute will connect to it by itself."
set "M_NOSCRIPT=The start file is missing on the phone. Open the MahiRoute app once and run this file again."
set "M_FAILED=Could not start the helper. Check that USB debugging is allowed on the phone."
set "M_NOADB1=adb was not found. Download Android SDK Platform-Tools from developer.android.com/tools/releases/platform-tools,"
set "M_NOADB2=unzip the platform-tools folder next to this file and run it again."
exit /b

:messages_ru
set "M_TITLE=MahiRoute: запуск помощника"
set "M_HEAD=MahiRoute: запуск помощника с компьютера"
set "M_CONNECT=Подключите телефон кабелем. На телефоне должна быть включена «Отладка по USB»."
set "M_WAIT=Жду телефон..."
set "M_AUTH=Разрешите отладку на экране телефона и отметьте «Всегда разрешать с этого компьютера»."
set "M_SEVERAL=Подключено несколько устройств:"
set "M_SERIAL= Введите серийный номер нужного телефона: "
set "M_PHONE=Телефон:"
set "M_COMMAND=Команда для запуска:"
set "M_ENTER= Нажмите Enter, чтобы запустить помощника на телефоне..."
set "M_DONE=Готово. Помощник запущен, MahiRoute на телефоне подключится к нему сам."
set "M_NOSCRIPT=На телефоне нет файла запуска. Откройте приложение MahiRoute один раз и запустите этот файл снова."
set "M_FAILED=Не удалось запустить помощника. Проверьте, что отладка по USB разрешена на телефоне."
set "M_NOADB1=Не найден adb. Скачайте Android SDK Platform-Tools с developer.android.com/tools/releases/platform-tools,"
set "M_NOADB2=распакуйте папку platform-tools рядом с этим файлом и запустите его снова."
exit /b
