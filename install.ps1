$ErrorActionPreference = "Stop"

$Repo = if ($env:SCLAUDE_REPO) { $env:SCLAUDE_REPO } else { "lauzhihao/sclaude" }
$SclaudeHome = if ($env:SCLAUDE_HOME) { $env:SCLAUDE_HOME } else { Join-Path $HOME ".sclaude" }
$env:SCLAUDE_HOME = $SclaudeHome
$InstallBin = if ($env:INSTALL_BIN) { $env:INSTALL_BIN } else { Join-Path $SclaudeHome "bin" }
$TmpRoot = Join-Path $SclaudeHome "tmp"
$WrapperPath = Join-Path $InstallBin "sclaude.exe"
$OpusPath = Join-Path $InstallBin "opus.exe"
$SonnetPath = Join-Path $InstallBin "sonnet.exe"
$HaikuPath = Join-Path $InstallBin "haiku.exe"
$OriginalWrapperPath = Join-Path $InstallBin "sclaude-original.cmd"
$Version = $env:SCLAUDE_VERSION

function Resolve-Version {
  if ($Version) {
    return $Version
  }
  $api = "https://api.github.com/repos/$Repo/releases/latest"
  $release = Invoke-RestMethod -Uri $api
  if (-not $release.tag_name) {
    throw "Failed to resolve latest release tag from $api"
  }
  return $release.tag_name
}

function Resolve-Target {
  switch ($env:PROCESSOR_ARCHITECTURE) {
    "AMD64" { return "x86_64-pc-windows-msvc" }
    "ARM64" { throw "Windows ARM64 release assets are not published yet. Build from source with cargo for now." }
    default { throw "Unsupported Windows architecture: $env:PROCESSOR_ARCHITECTURE" }
  }
}

function Ensure-UserPath {
  $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
  $needle = $InstallBin.TrimEnd('\')
  if (-not $userPath) {
    [Environment]::SetEnvironmentVariable("Path", $needle, "User")
    return
  }
  $parts = $userPath.Split(';') | Where-Object { $_ -ne "" }
  if ($parts -notcontains $needle) {
    [Environment]::SetEnvironmentVariable("Path", ($parts + $needle) -join ';', "User")
  }
}

function Install-OriginalWrapper {
  @"
@echo off
if not defined SCLAUDE_HOME set "SCLAUDE_HOME=%USERPROFILE%\.sclaude"
if defined CLAUDE_BIN (
  if exist "%CLAUDE_BIN%" (
    "%CLAUDE_BIN%" %*
    exit /b %errorlevel%
  )
)
if exist "%SCLAUDE_HOME%\runtime\claude-code\claude.cmd" (
  "%SCLAUDE_HOME%\runtime\claude-code\claude.cmd" %*
  exit /b %errorlevel%
)
if exist "%SCLAUDE_HOME%\runtime\claude-code\claude.exe" (
  "%SCLAUDE_HOME%\runtime\claude-code\claude.exe" %*
  exit /b %errorlevel%
)
if exist "%SCLAUDE_HOME%\runtime\claude-code\node_modules\.bin\claude.cmd" (
  "%SCLAUDE_HOME%\runtime\claude-code\node_modules\.bin\claude.cmd" %*
  exit /b %errorlevel%
)
if exist "%SCLAUDE_HOME%\runtime\claude-code\node_modules\.bin\claude.exe" (
  "%SCLAUDE_HOME%\runtime\claude-code\node_modules\.bin\claude.exe" %*
  exit /b %errorlevel%
)
where claude >nul 2>nul
if %errorlevel% neq 0 (
  echo claude not found on PATH. 1>&2
  exit /b 1
)
claude %*
"@ | Set-Content -Path $OriginalWrapperPath -Encoding ASCII
}

function Post-InstallImport {
  $authPath = Join-Path $HOME ".claude.json"
  $altAuthPath = Join-Path $HOME ".config.json"
  $profileAuthPath = Join-Path $HOME ".claude\.claude.json"
  $profileAltAuthPath = Join-Path $HOME ".claude\.config.json"
  if ((Test-Path $authPath) -or (Test-Path $altAuthPath) -or (Test-Path $profileAuthPath) -or (Test-Path $profileAltAuthPath)) {
    & $WrapperPath import-known | Out-Null
    & $WrapperPath refresh | Out-Null
  }
}

$target = Resolve-Target
$version = Resolve-Version
$asset = "sclaude-$version-$target.zip"
$url = "https://github.com/$Repo/releases/download/$version/$asset"
New-Item -ItemType Directory -Path $TmpRoot -Force | Out-Null
$tmp = Join-Path $TmpRoot ("install-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
New-Item -ItemType Directory -Path $InstallBin -Force | Out-Null

$archivePath = Join-Path $tmp $asset
Invoke-WebRequest -Uri $url -OutFile $archivePath
Expand-Archive -Path $archivePath -DestinationPath $tmp -Force

$binaryPath = Join-Path $tmp "sclaude.exe"
if (-not (Test-Path $binaryPath)) {
  throw "Release archive did not contain sclaude.exe"
}

Copy-Item $binaryPath $WrapperPath -Force
Copy-Item $binaryPath $OpusPath -Force
Copy-Item $binaryPath $SonnetPath -Force
Copy-Item $binaryPath $HaikuPath -Force
Install-OriginalWrapper
Ensure-UserPath
Post-InstallImport

Write-Host "SCLAUDE_HOME is $SclaudeHome"
Write-Host "Installed to $WrapperPath"
Write-Host "Installed model entrypoints to $OpusPath, $SonnetPath, $HaikuPath"
Write-Host "Installed passthrough helper to $OriginalWrapperPath"
Write-Host "If the current shell cannot find sclaude yet, restart PowerShell or open a new terminal."
