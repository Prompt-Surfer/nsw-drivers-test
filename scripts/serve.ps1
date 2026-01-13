# Start the NSW Drivers Test server
# Usage: .\scripts\serve.ps1

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $ProjectRoot

Write-Host "Starting NSW Drivers Test Server..." -ForegroundColor Cyan

# Load .env
if (Test-Path .env) {
    Get-Content .env | ForEach-Object {
        if ($_ -match "^([^#][^=]+)=(.*)$") {
            [System.Environment]::SetEnvironmentVariable($matches[1].Trim(), $matches[2].Trim(), "Process")
        }
    }
    Write-Host "Loaded .env file" -ForegroundColor Green
}

# Refresh PATH
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

# Start ChromeDriver
$chromedriverPath = ".\chromedriver-win64\chromedriver.exe"
if (Test-Path $chromedriverPath) {
    Start-Process -FilePath $chromedriverPath -ArgumentList "--port=57909", "--allowed-origins=*" -WindowStyle Hidden
    Write-Host "ChromeDriver started on port 57909" -ForegroundColor Green
    Start-Sleep -Seconds 2
} else {
    Write-Host "Warning: ChromeDriver not found at $chromedriverPath" -ForegroundColor Yellow
}

Write-Host "Starting Leptos server (release build)..." -ForegroundColor Cyan
Write-Host "Server will be available at http://127.0.0.1:3001" -ForegroundColor Green
Write-Host ""

# Run the pre-built release binary directly (no compilation)
$env:LEPTOS_SITE_ROOT = "target/site"
$env:LEPTOS_SITE_PKG_DIR = "pkg"
$env:LEPTOS_SITE_ADDR = "127.0.0.1:3001"
$env:LEPTOS_RELOAD_PORT = "3002"
.\target\release\nsw-closest-display.exe
