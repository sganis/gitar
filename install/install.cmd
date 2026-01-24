@echo off
setlocal EnableExtensions EnableDelayedExpansion

REM -----------------------------------------------------------------------------
REM gitar installer (Windows CMD)
REM Usage:
REM   install.cmd
REM   install.cmd latest
REM   install.cmd stable
REM   install.cmd v1.2.3
REM   install.cmd 1.2.3
REM
REM Env overrides:
REM   set GITAR_REPO=sganis/gitar
REM   set GITAR_INSTALL_DIR=%USERPROFILE%\.gitar
REM   set GITAR_MODIFY_PATH=1   (set to 0 to avoid touching user PATH)
REM -----------------------------------------------------------------------------

REM -------------------------
REM Parse + validate target
REM -------------------------
set "TARGET=%~1"
if "%TARGET%"=="" set "TARGET=latest"

call :validate_target "%TARGET%"
if errorlevel 1 exit /b 1

REM -------------------------
REM Detect 64-bit + arch
REM -------------------------
call :detect_arch
if errorlevel 1 exit /b 1

REM -------------------------
REM Config
REM -------------------------
if "%GITAR_REPO%"=="" set "GITAR_REPO=sganis/gitar"
if "%GITAR_INSTALL_DIR%"=="" set "GITAR_INSTALL_DIR=%USERPROFILE%\.gitar"
set "GITAR_BIN_DIR=%GITAR_INSTALL_DIR%\bin"
set "GITAR_BIN_PATH=%GITAR_BIN_DIR%\gitar.exe"

if "%GITAR_MODIFY_PATH%"=="" set "GITAR_MODIFY_PATH=1"

REM -------------------------
REM Require curl
REM -------------------------
curl --version >nul 2>&1
if errorlevel 1 (
  call :err "curl is required but not available. Install curl or use the PowerShell installer."
  exit /b 1
)

REM -------------------------
REM Temp dir (needed by resolve_version for API calls)
REM -------------------------
set "TMPROOT=%TEMP%\gitar-install-%RANDOM%%RANDOM%%RANDOM%"
mkdir "%TMPROOT%" >nul 2>&1 || (call :err "failed to create temp dir: %TMPROOT%" & exit /b 1)

REM -------------------------
REM Resolve version
REM -------------------------
call :resolve_version "%TARGET%"
if errorlevel 1 exit /b 1

REM -------------------------
REM Build URLs
REM -------------------------
REM Strip 'v' prefix from VERSION for artifact name (tags are v1.2.3, artifacts are gitar-1.2.3-...)
set "VER=%VERSION%"
if "%VER:~0,1%"=="v" set "VER=%VER:~1%"
if "%VER:~0,1%"=="V" set "VER=%VER:~1%"

set "OS=windows"
set "ASSET=gitar-%VER%-%OS%-%ARCH%.zip"
set "URL=https://github.com/%GITAR_REPO%/releases/download/%VERSION%/%ASSET%"
set "SHA_URL=%URL%.sha256"

REM -------------------------
REM Temp paths
REM -------------------------
set "ZIP=%TMPROOT%\gitar.zip"
set "SHAFILE=%TMPROOT%\gitar.zip.sha256"
set "EXTRACT=%TMPROOT%\extract"

mkdir "%EXTRACT%" >nul 2>&1 || (call :err "failed to create temp dir: %EXTRACT%" & exit /b 1)

call :say "Installing gitar (%VERSION%) for %OS%/%ARCH%"
call :say "Repo: %GITAR_REPO%"
call :say "Install dir: %GITAR_INSTALL_DIR%"
call :say ""

REM -------------------------
REM Download zip
REM -------------------------
call :say "Downloading: %URL%"
call :download_file "%URL%" "%ZIP%"
if errorlevel 1 (
  call :cleanup
  call :err "Download failed (asset not found for %OS%/%ARCH%?). URL: %URL%"
  exit /b 1
)

REM -------------------------
REM Optional checksum verify
REM -------------------------
set "EXPECTED="
call :download_file_quiet "%SHA_URL%" "%SHAFILE%"
if not errorlevel 1 (
  for /f "usebackq tokens=1" %%a in ("%SHAFILE%") do set "EXPECTED=%%a"
  call :is_sha256 "%EXPECTED%"
  if not errorlevel 1 (
    call :sha256_file "%ZIP%" ACTUAL
    if errorlevel 1 (
      call :cleanup
      call :err "Failed to compute SHA256 (certutil missing?)"
      exit /b 1
    )
    if /i not "%ACTUAL%"=="%EXPECTED%" (
      call :cleanup
      call :err "Checksum verification failed"
      exit /b 1
    )
    call :say "Checksum OK"
  ) else (
    call :say "Warning: checksum file found but format not recognized; skipping verification"
  )
) else (
  call :say "Note: no checksum file found; skipping verification."
  call :say "      (Recommended: upload *.zip.sha256 with SHA-256 of the zip.)"
)

REM -------------------------
REM Extract (prefer tar.exe, fallback to PowerShell)
REM -------------------------
call :extract_zip "%ZIP%" "%EXTRACT%"
if errorlevel 1 (
  call :cleanup
  call :err "Failed to extract archive (need tar.exe or PowerShell Expand-Archive)"
  exit /b 1
)

REM -------------------------
REM Find gitar.exe in extracted content
REM -------------------------
set "FOUND="
if exist "%EXTRACT%\gitar.exe" set "FOUND=%EXTRACT%\gitar.exe"

if "%FOUND%"=="" (
  for /f "delims=" %%f in ('dir /b /s "%EXTRACT%\gitar.exe" 2^>nul') do (
    set "FOUND=%%f"
    goto :found_done
  )
)
:found_done

if "%FOUND%"=="" (
  call :cleanup
  call :err "Archive did not contain a 'gitar.exe' binary"
  exit /b 1
)

REM -------------------------
REM Install binary
REM -------------------------
if not exist "%GITAR_BIN_DIR%" mkdir "%GITAR_BIN_DIR%" >nul 2>&1
copy /y "%FOUND%" "%GITAR_BIN_PATH%" >nul 2>&1
if errorlevel 1 (
  call :cleanup
  call :err "Failed to install to: %GITAR_BIN_PATH%"
  exit /b 1
)

call :say "Installed: %GITAR_BIN_PATH%"

REM -------------------------
REM PATH (user) update
REM -------------------------
call :maybe_add_path "%GITAR_BIN_DIR%"

REM -------------------------
REM Smoke test
REM -------------------------
call :say ""
"%GITAR_BIN_PATH%" --version >nul 2>&1
if errorlevel 1 (
  call :say "✅ gitar installed"
) else (
  for /f "delims=" %%v in ('"%GITAR_BIN_PATH%" --version 2^>nul') do set "VOUT=%%v"
  if not "%VOUT%"=="" (
    call :say "✅ gitar runs: %VOUT%"
  ) else (
    call :say "✅ gitar installed"
  )
)

where gitar >nul 2>&1
if not errorlevel 1 (
  for /f "delims=" %%w in ('where gitar') do (
    call :say "✅ gitar is on PATH: %%w"
    goto :path_done
  )
) else (
  call :say "ℹ️  gitar is not yet on PATH in this shell."
  call :say "   Run: set ""PATH=%GITAR_BIN_DIR%;%PATH%"""
)
:path_done

call :say ""
call :say "✅ Installation complete!"
call :say ""

call :cleanup
exit /b 0

REM ============================================================================
REM SUBROUTINES
REM ============================================================================

:say
echo %~1
exit /b 0

:err
echo error: %~1 1>&2
exit /b 0

:validate_target
set "T=%~1"
if /i "%T%"=="stable" exit /b 0
if /i "%T%"=="latest" exit /b 0
echo %T% | findstr /r "^[vV][0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*" >nul && exit /b 0
echo %T% | findstr /r "^[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*" >nul && exit /b 0
echo Usage: %~nx0 [stable^|latest^|vX.Y.Z^|X.Y.Z] 1>&2
exit /b 1

:detect_arch
set "ARCH="
REM Handle WOW64
set "PA=%PROCESSOR_ARCHITECTURE%"
set "PW=%PROCESSOR_ARCHITEW6432%"

if /i "%PA%"=="AMD64" set "ARCH=x86_64"
if /i "%PA%"=="ARM64" set "ARCH=aarch64"
if "%ARCH%"=="" (
  if /i "%PW%"=="AMD64" set "ARCH=x86_64"
  if /i "%PW%"=="ARM64" set "ARCH=aarch64"
)

if "%ARCH%"=="" (
  echo gitar does not support 32-bit Windows. Please use a 64-bit version of Windows. 1>&2
  exit /b 1
)
exit /b 0

:resolve_version
set "T=%~1"
if /i "%T%"=="latest" goto :resolve_latest
if /i "%T%"=="stable" goto :resolve_latest

REM Normalize: allow "1.2.3" -> "v1.2.3"
echo %T% | findstr /r "^[0-9]" >nul
if not errorlevel 1 (
  set "VERSION=v%T%"
  exit /b 0
)

REM Already v-prefixed
set "VERSION=%T%"
exit /b 0

:resolve_latest
set "API=https://api.github.com/repos/%GITAR_REPO%/releases/latest"
set "JSON=%TMPROOT%\latest.json"

call :download_file "%API%" "%JSON%"
if errorlevel 1 (
  call :err "Failed to query GitHub API for latest release"
  exit /b 1
)

set "VERSION="
for /f "usebackq delims=" %%l in (`findstr /c:"\"tag_name\"" "%JSON%"`) do (
  set "LINE=%%l"
  REM Split on :
  for /f "tokens=2 delims=:" %%a in ("!LINE!") do (
    set "V=%%a"
    set "V=!V: =!"
    set "V=!V:"=!"
    set "V=!V:,=!"
    if not "!V!"=="" (
      set "VERSION=!V!"
      goto :got_tag
    )
  )
)

:got_tag
del "%JSON%" >nul 2>&1
if "%VERSION%"=="" (
  call :err "Could not parse latest release tag from GitHub API"
  exit /b 1
)
exit /b 0

:download_file
set "URL=%~1"
set "OUT=%~2"
curl -fsSL "%URL%" -o "%OUT%"
exit /b %ERRORLEVEL%

:download_file_quiet
set "URL=%~1"
set "OUT=%~2"
curl -fsSL "%URL%" -o "%OUT%" >nul 2>&1
exit /b %ERRORLEVEL%

:is_sha256
set "S=%~1"
echo %S% | findstr /r "^[0-9a-fA-F][0-9a-fA-F]*$" >nul || exit /b 1
call :strlen "%S%" LEN
if "%LEN%"=="64" exit /b 0
exit /b 1

:sha256_file
REM Args: %1=file, %2=outvar
set "FILE=%~1"
set "OUTVAR=%~2"
set "HASH="
certutil -hashfile "%FILE%" SHA256 > "%TMPROOT%\hash.txt" 2>nul
if errorlevel 1 exit /b 1

for /f "usebackq skip=1 tokens=*" %%i in ("%TMPROOT%\hash.txt") do (
  set "L=%%i"
  set "L=!L: =!"
  if /i "!L!"=="CertUtil:Thecommandcompletedsuccessfully." goto :hash_done
  if not "!L!"=="" (
    set "HASH=!L!"
    goto :hash_done
  )
)
:hash_done
del "%TMPROOT%\hash.txt" >nul 2>&1
if "%HASH%"=="" exit /b 1
set "%OUTVAR%=%HASH%"
exit /b 0

:extract_zip
REM Prefer tar.exe (Windows 10+), fallback to PowerShell
set "ZIP=%~1"
set "DEST=%~2"

tar --version >nul 2>&1
if not errorlevel 1 (
  REM bsdtar can usually extract zip via -xf
  tar -xf "%ZIP%" -C "%DEST%" >nul 2>&1
  exit /b %ERRORLEVEL%
)

powershell -NoProfile -Command "Expand-Archive -LiteralPath '%ZIP%' -DestinationPath '%DEST%' -Force" >nul 2>&1
exit /b %ERRORLEVEL%

:maybe_add_path
set "BDIR=%~1"

REM Already in current PATH?
echo %PATH% | findstr /i /c:"%BDIR%" >nul
if not errorlevel 1 exit /b 0

if "%GITAR_MODIFY_PATH%"=="0" (
  call :say "Note: not modifying user PATH (set GITAR_MODIFY_PATH=1 to enable)."
  call :say "Add this to your PATH:"
  call :say "  %BDIR%"
  exit /b 0
)

REM Update user PATH (persist). Also update current session PATH.
set "CUR_USER_PATH="
for /f "tokens=2,*" %%a in ('reg query "HKCU\Environment" /v Path 2^>nul ^| findstr /i "Path"') do (
  set "CUR_USER_PATH=%%b"
)

echo !CUR_USER_PATH! | findstr /i /c:"%BDIR%" >nul
if not errorlevel 1 (
  set "PATH=%BDIR%;%PATH%"
  exit /b 0
)

if not "!CUR_USER_PATH!"=="" (
  set "NEW_USER_PATH=!CUR_USER_PATH!;%BDIR%"
) else (
  set "NEW_USER_PATH=%BDIR%"
)

REM setx truncates very long PATHs; acceptable for typical user PATH sizes.
setx Path "!NEW_USER_PATH!" >nul 2>&1
if errorlevel 1 (
  call :say "Warning: failed to persist PATH update. You can add this manually:"
  call :say "  %BDIR%"
) else (
  call :say "Added to user PATH: %BDIR%"
)
set "PATH=%BDIR%;%PATH%"
exit /b 0

:strlen
REM Args: %1=string, %2=outvar
setlocal EnableDelayedExpansion
set "S=%~1"
set /a N=0
:strlen_loop
if "!S:~%N%,1!"=="" goto :strlen_done
set /a N+=1
goto :strlen_loop
:strlen_done
endlocal & set "%~2=%N%"
exit /b 0

:cleanup
REM Best-effort cleanup
if exist "%TMPROOT%" (
  rmdir /s /q "%TMPROOT%" >nul 2>&1
)
exit /b 0
