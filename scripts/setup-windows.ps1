# Kiflo Windows Dev Setup
# Run from an elevated (Administrator) PowerShell terminal:
#   Set-ExecutionPolicy Bypass -Scope Process -Force
#   .\scripts\setup-windows.ps1

$ErrorActionPreference = "Stop"

function Write-Step($msg) {
    Write-Host ""
    Write-Host "==> $msg" -ForegroundColor Cyan
}

function Write-Ok($msg) {
    Write-Host "    OK: $msg" -ForegroundColor Green
}

function Write-Skip($msg) {
    Write-Host "    SKIP: $msg (already installed)" -ForegroundColor Yellow
}

# --- Require Administrator ---
if (-not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host "ERROR: Please run this script as Administrator." -ForegroundColor Red
    Write-Host "  Right-click PowerShell -> 'Run as administrator', then re-run the script."
    exit 1
}

# --- winget availability check ---
Write-Step "Checking winget"
if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    Write-Host "ERROR: winget not found. Install the App Installer from the Microsoft Store and try again." -ForegroundColor Red
    exit 1
}
Write-Ok "winget is available"

# --- Git ---
Write-Step "Git"
if (Get-Command git -ErrorAction SilentlyContinue) {
    Write-Skip "git $(git --version)"
} else {
    winget install --id Git.Git -e --silent --accept-package-agreements --accept-source-agreements
    Write-Ok "Git installed"
}

# --- Node.js LTS ---
Write-Step "Node.js LTS"
if (Get-Command node -ErrorAction SilentlyContinue) {
    Write-Skip "node $(node --version)"
} else {
    winget install --id OpenJS.NodeJS.LTS -e --silent --accept-package-agreements --accept-source-agreements
    Write-Ok "Node.js installed"
}

# --- CMake ---
Write-Step "CMake"
if (Get-Command cmake -ErrorAction SilentlyContinue) {
    Write-Skip "cmake $(cmake --version | Select-Object -First 1)"
} else {
    winget install --id Kitware.CMake -e --silent --accept-package-agreements --accept-source-agreements --override "/silent /add_to_path=1"
    Write-Ok "CMake installed"
}

# --- Visual Studio Build Tools 2022 (C++ Desktop workload) ---
Write-Step "Visual Studio Build Tools 2022 (MSVC + Windows SDK)"
$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$hasMsvc = $false
if (Test-Path $vsWhere) {
    $installs = & $vsWhere -products * -requires Microsoft.VisualCpp.Tools.HostX64.TargetX64 -format value -property installationPath 2>$null
    if ($installs) { $hasMsvc = $true }
}

if ($hasMsvc) {
    Write-Skip "MSVC toolchain already present"
} else {
    winget install --id Microsoft.VisualStudio.2022.BuildTools -e --silent --accept-package-agreements --accept-source-agreements --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.Windows11SDK.22621 --includeRecommended"
    Write-Ok "Visual Studio Build Tools installed"
}

# --- Rust (rustup) ---
Write-Step "Rust via rustup"
if (Get-Command rustup -ErrorAction SilentlyContinue) {
    Write-Skip "rustup $(rustup --version 2>&1 | Select-Object -First 1)"
} else {
    $rustupInstaller = "$env:TEMP\rustup-init.exe"
    Write-Host "    Downloading rustup-init.exe..."
    Invoke-WebRequest -Uri "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe" -OutFile $rustupInstaller
    & $rustupInstaller -y --default-toolchain stable-msvc --default-host x86_64-pc-windows-msvc
    Remove-Item $rustupInstaller -Force
    Write-Ok "Rust installed"
}

# --- Ensure MSVC toolchain is default ---
Write-Step "Ensuring Rust MSVC toolchain is default"
$env:PATH = [System.Environment]::GetEnvironmentVariable("PATH", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("PATH", "User")

if (Get-Command rustup -ErrorAction SilentlyContinue) {
    rustup toolchain install stable-msvc 2>&1 | Out-Null
    rustup default stable-msvc
    rustup target add x86_64-pc-windows-msvc 2>&1 | Out-Null
    Write-Ok "Default toolchain: $(rustup default)"
} else {
    Write-Host "    NOTE: rustup not on PATH yet - open a new terminal after this script and run:" -ForegroundColor Yellow
    Write-Host "      rustup default stable-msvc" -ForegroundColor Yellow
}

# --- WebView2 ---
Write-Step "WebView2 Runtime"
$wv2Key = "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
$wv2 = Get-ItemProperty $wv2Key -ErrorAction SilentlyContinue
if ($wv2) {
    Write-Skip "WebView2 $($wv2.pv)"
} else {
    winget install --id Microsoft.EdgeWebView2Runtime -e --silent --accept-package-agreements --accept-source-agreements
    Write-Ok "WebView2 installed"
}

# --- Summary ---
Write-Host ""
Write-Host "======================================" -ForegroundColor Cyan
Write-Host "  Setup complete!" -ForegroundColor Green
Write-Host "======================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Next steps:" -ForegroundColor White
Write-Host "  1. Open a NEW PowerShell window (to pick up updated PATH)" -ForegroundColor White
Write-Host "  2. From the repo root:  npm install" -ForegroundColor White
Write-Host "  3. Then build:" -ForegroundColor White
Write-Host "       cd apps\desktop" -ForegroundColor White
Write-Host "       npm run tauri:build" -ForegroundColor White
Write-Host ""
Write-Host "First build will take 10-20 min (whisper.cpp compiles from source)." -ForegroundColor Yellow
