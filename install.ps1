# Sloth Installer — Windows (PowerShell)
# Usage: irm https://raw.githubusercontent.com/AymericChaverot/sloth/main/install.ps1 | iex
#Requires -Version 5.1
$ErrorActionPreference = 'Stop'

$Repo = "AymericChaverot/sloth"
$BinaryName = "sloth"
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\sloth"

# ─── Helpers ─────────────────────────────────────────────
function Write-Info  { Write-Host "[INFO]  $args" -ForegroundColor Cyan }
function Write-Ok    { Write-Host "[OK]    $args" -ForegroundColor Green }
function Write-Warn  { Write-Host "[WARN]  $args" -ForegroundColor Yellow }
function Write-Err   { Write-Host "[ERROR] $args" -ForegroundColor Red; exit 1 }

# ─── Detect target ───────────────────────────────────────
function Get-Target {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    switch ($arch) {
        "X64"  { return "x86_64-pc-windows-msvc" }
        "Arm64" { Write-Err "ARM64 Windows builds are not currently available." }
        default { Write-Err "Unsupported architecture: $arch" }
    }
}

# ─── Get latest release version ──────────────────────────
function Get-LatestVersion {
    $url = "https://api.github.com/repos/$Repo/releases/latest"
    try {
        $release = Invoke-RestMethod -Uri $url -UseBasicParsing
        return $release.tag_name
    } catch {
        Write-Err "Failed to fetch latest release. Check your internet connection."
    }
}

# ─── Main install logic ─────────────────────────────────
Write-Host ""
Write-Host "  🦥 Sloth Installer for Windows" -ForegroundColor Cyan
Write-Host "  ════════════════════════════════" -ForegroundColor DarkGray
Write-Host ""

$Target = Get-Target
Write-Info "Detected platform: $Target"

Write-Info "Fetching latest release..."
$Version = Get-LatestVersion
Write-Ok "Latest version: $Version"

$AssetName = "$BinaryName-$Target.zip"
$DownloadUrl = "https://github.com/$Repo/releases/download/$Version/$AssetName"

$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) "sloth-install-$(Get-Random)"
New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

try {
    Write-Info "Downloading $AssetName..."
    $ZipPath = Join-Path $TmpDir $AssetName
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath -UseBasicParsing
    Write-Ok "Downloaded."

    Write-Info "Extracting..."
    Expand-Archive -Path $ZipPath -DestinationPath $TmpDir -Force
    Write-Ok "Extracted."

    # Create install directory
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    # Move binary
    $ExeName = "$BinaryName.exe"
    $SourceExe = Join-Path $TmpDir $ExeName
    $DestExe = Join-Path $InstallDir $ExeName

    if (-not (Test-Path $SourceExe)) {
        # Sometimes the zip contains just the exe at root
        $found = Get-ChildItem -Path $TmpDir -Recurse -Filter $ExeName | Select-Object -First 1
        if ($found) {
            $SourceExe = $found.FullName
        } else {
            Write-Err "Could not find $ExeName in the downloaded archive."
        }
    }

    Copy-Item -Path $SourceExe -Destination $DestExe -Force
    Write-Ok "Installed to $DestExe"

    # ─── PATH configuration ──────────────────────────────
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($UserPath -split ';' | Where-Object { $_ -eq $InstallDir }) {
        Write-Ok "$InstallDir is already in your PATH."
    } else {
        Write-Info "Adding $InstallDir to your user PATH..."
        $NewPath = "$UserPath;$InstallDir"
        [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
        # Also update current session
        $env:Path = "$env:Path;$InstallDir"
        Write-Ok "Added to PATH. Changes apply to new terminal sessions."
    }

    Write-Host ""
    Write-Host "  ✅ sloth $Version installed successfully!" -ForegroundColor Green
    Write-Host "     Run 'sloth --help' to get started." -ForegroundColor Cyan
    Write-Host ""
} finally {
    # Cleanup
    Remove-Item -Path $TmpDir -Recurse -Force -ErrorAction SilentlyContinue
}
