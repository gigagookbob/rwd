# rwd install script for Windows
# Usage: irm https://raw.githubusercontent.com/gigagookbob/rwd/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "gigagookbob/rwd"
$BinaryName = "rwd.exe"

# Get latest release tag
$Release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
$Version = $Release.tag_name

if (-not $Version) {
    Write-Error "Failed to fetch latest release version."
    exit 1
}

Write-Host "Installing rwd $Version ..."

$Asset = "rwd-x86_64-pc-windows-msvc.zip"
$DownloadUrl = "https://github.com/$Repo/releases/download/$Version/$Asset"

# Download to temp directory
$TmpDir = Join-Path $env:TEMP "rwd-install"
if (Test-Path $TmpDir) { Remove-Item $TmpDir -Recurse -Force }
New-Item -ItemType Directory -Path $TmpDir | Out-Null

$ZipPath = Join-Path $TmpDir $Asset

Write-Host "Downloading: $DownloadUrl"
Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath -UseBasicParsing

# Extract
Expand-Archive -Path $ZipPath -DestinationPath $TmpDir -Force

# Install directory: ~/.rwd/bin
$InstallDir = Join-Path $env:USERPROFILE ".rwd\bin"
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir | Out-Null
}

Copy-Item (Join-Path $TmpDir $BinaryName) (Join-Path $InstallDir $BinaryName) -Force

# Create default output directory
$DefaultOutput = Join-Path $env:USERPROFILE ".rwd\output"
if (-not (Test-Path $DefaultOutput)) {
    New-Item -ItemType Directory -Path $DefaultOutput | Out-Null
}

# Add to user PATH if not already present
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    Write-Host "Added $InstallDir to user PATH (restart terminal to apply)."
}

# Cleanup
Remove-Item $TmpDir -Recurse -Force

Write-Host ""
Write-Host "rwd $Version installed!"
Write-Host "Location: $InstallDir\$BinaryName"
Write-Host "Default output: $DefaultOutput"
Write-Host ""
Write-Host "Get started:"
Write-Host "  rwd init     # Initial setup (API key, output path)"
Write-Host "  rwd today    # Analyze today's sessions"
