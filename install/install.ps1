param(
  [Parameter(Position=0)]
  [ValidatePattern('^(stable|latest|v?\d+\.\d+\.\d+(-[^\s]+)?)$')]
  [string]$Target = "latest"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

function Say($msg) { Write-Output $msg }
function Err($msg) { Write-Error "error: $msg"; exit 1 }

# ---- Config (env overrides like install.sh) ----
$GITAR_REPO = if ($env:GITAR_REPO) { $env:GITAR_REPO } else { "sganis/gitar" }
$GITAR_INSTALL_DIR = if ($env:GITAR_INSTALL_DIR) { $env:GITAR_INSTALL_DIR } else { Join-Path $env:USERPROFILE ".gitar" }
$GITAR_BIN_DIR = Join-Path $GITAR_INSTALL_DIR "bin"
$GITAR_BIN_PATH = Join-Path $GITAR_BIN_DIR "gitar.exe"

# Set to 0 to avoid modifying user PATH
$GITAR_MODIFY_PATH = if ($env:GITAR_MODIFY_PATH) { $env:GITAR_MODIFY_PATH } else { "1" }

# ---- Preflight ----
if (-not [Environment]::Is64BitProcess) {
  Err "gitar does not support 32-bit Windows. Please use 64-bit PowerShell on 64-bit Windows."
}

function Get-Arch {
  # Prefer OSArchitecture (PowerShell 7+), fall back to PROCESSOR_ARCHITECTURE
  try {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    switch ($arch) {
      "X64"   { return "x86_64" }
      "Arm64" { return "aarch64" }
      default { Err "Unsupported architecture: $arch" }
    }
  } catch {
    $pa = $env:PROCESSOR_ARCHITECTURE
    if ($pa -eq "AMD64") { return "x86_64" }
    if ($pa -eq "ARM64") { return "aarch64" }
    Err "Unsupported architecture: $pa"
  }
}

function Normalize-Target([string]$t) {
  if ([string]::IsNullOrWhiteSpace($t) -or $t -eq "latest" -or $t -eq "stable") {
    return $t
  }
  if ($t -match '^v') { return $t }
  return "v$t"
}

function Resolve-Version([string]$t) {
  $t = Normalize-Target $t
  if ($t -eq "latest" -or $t -eq "stable") {
    $api = "https://api.github.com/repos/$GITAR_REPO/releases/latest"
    try {
      $resp = Invoke-RestMethod -Uri $api -Headers @{ "User-Agent" = "gitar-installer" }
      if (-not $resp.tag_name) { Err "Could not parse latest release tag from GitHub API" }
      return [string]$resp.tag_name
    } catch {
      Err "Failed to query GitHub API for latest release: $_"
    }
  }
  return $t
}

# Strip 'v' prefix from version for artifact name (tags are v1.2.3, artifacts are gitar-1.2.3-...)
function Strip-VPrefix([string]$ver) {
  if ($ver.StartsWith("v") -or $ver.StartsWith("V")) {
    return $ver.Substring(1)
  }
  return $ver
}

function Artifact-Url([string]$tag, [string]$arch) {
  # Windows uses zip for easy extraction
  $ver = Strip-VPrefix $tag
  return "https://github.com/$GITAR_REPO/releases/download/$tag/gitar-$ver-windows-$arch.zip"
}

function Artifact-ShaUrl([string]$tag, [string]$arch) {
  $ver = Strip-VPrefix $tag
  return "https://github.com/$GITAR_REPO/releases/download/$tag/gitar-$ver-windows-$arch.zip.sha256"
}

function Try-Download([string]$url, [string]$outFile) {
  try {
    Invoke-WebRequest -Uri $url -OutFile $outFile -UseBasicParsing | Out-Null
    return $true
  } catch {
    return $false
  }
}

function Get-Sha256([string]$filePath) {
  return (Get-FileHash -Path $filePath -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Maybe-Add-Path([string]$binDir) {
  # If already in current session PATH, nothing to do
  if ($env:Path.Split(';') -contains $binDir) { return }

  if ($GITAR_MODIFY_PATH -ne "1") {
    Say "Note: not modifying user PATH (set GITAR_MODIFY_PATH=1 to enable)."
    Say "Add this to your PATH:"
    Say "  $binDir"
    return
  }

  $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
  $userParts = @()
  if (-not [string]::IsNullOrEmpty($userPath)) {
    $userParts = $userPath.Split(';') | Where-Object { $_ -and $_.Trim() -ne "" }
  }

  if ($userParts -contains $binDir) {
    # Still add to current session so it works immediately
    $env:Path = "$binDir;$env:Path"
    return
  }

  $newUserPath = if ($userPath) { "$userPath;$binDir" } else { $binDir }
  [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
  $env:Path = "$binDir;$env:Path"

  Say "Added to user PATH: $binDir"
  Say "Note: new terminals will pick this up automatically."
}

# ---- Main ----
$arch = Get-Arch
$version = Resolve-Version $Target

Say "Installing gitar ($version) for windows/$arch"
Say "Repo: $GITAR_REPO"
Say "Install dir: $GITAR_INSTALL_DIR"
Say ""

$tmpRoot = Join-Path ([IO.Path]::GetTempPath()) ("gitar-install-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tmpRoot | Out-Null

$zipPath = Join-Path $tmpRoot "gitar.zip"
$shaPath = Join-Path $tmpRoot "gitar.zip.sha256"
$extractDir = Join-Path $tmpRoot "extract"

try {
  $url = Artifact-Url $version $arch
  $shaUrl = Artifact-ShaUrl $version $arch

  Say "Downloading: $url"
  if (-not (Try-Download $url $zipPath)) {
    Err "Download failed (asset not found for windows/$arch?). URL: $url"
  }

  $expected = $null
  if (Try-Download $shaUrl $shaPath) {
    $expected = (Get-Content -Path $shaPath -Raw).Trim() -split '\s+' | Select-Object -First 1
    if ($expected -match '^[a-fA-F0-9]{64}$') {
      $actual = Get-Sha256 $zipPath
      if ($actual -ne $expected.ToLowerInvariant()) {
        Err "Checksum verification failed"
      }
      Say "Checksum OK"
    } else {
      Say "Warning: checksum file found but format not recognized; skipping verification"
    }
  } else {
    Say "Note: no checksum file found; skipping verification."
    Say "      (Recommended: upload *.zip.sha256 with SHA-256 of the zip.)"
  }

  New-Item -ItemType Directory -Force -Path $extractDir | Out-Null
  Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force

  # Expect gitar.exe at root; otherwise search anywhere in archive
  $candidate = Join-Path $extractDir "gitar.exe"
  if (-not (Test-Path $candidate)) {
    $found = Get-ChildItem -Path $extractDir -Recurse -File -Filter "gitar.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $found) {
      Err "Archive did not contain a 'gitar.exe' binary"
    }
    $candidate = $found.FullName
  }

  New-Item -ItemType Directory -Force -Path $GITAR_BIN_DIR | Out-Null
  Copy-Item -Force -Path $candidate -Destination $GITAR_BIN_PATH

  Say "Installed: $GITAR_BIN_PATH"

  Maybe-Add-Path $GITAR_BIN_DIR

  Say ""
  try {
    $vout = & $GITAR_BIN_PATH --version 2>$null
    if ($vout) {
      Say "✅ gitar runs: $vout"
    } else {
      Say "✅ gitar installed"
    }
  } catch {
    Say "✅ gitar installed"
  }

  if (Get-Command gitar -ErrorAction SilentlyContinue) {
    $where = (Get-Command gitar).Source
    Say "✅ gitar is on PATH: $where"
  } else {
    Say "ℹ️  gitar is not yet on PATH in this shell."
    Say "   Run: `$env:Path = `"$GITAR_BIN_DIR;`$env:Path`""
  }

  Say ""
  Say "✅ Installation complete!"
  Say ""
}
finally {
  try {
    Remove-Item -Recurse -Force -Path $tmpRoot -ErrorAction SilentlyContinue | Out-Null
  } catch {
    # ignore cleanup errors
  }
}
