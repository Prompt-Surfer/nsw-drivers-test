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

# Set Tailwind version to avoid version mismatch warning
$env:LEPTOS_TAILWIND_VERSION = "v4.1.18"

# Start ChromeDriver
$chromedriverPath = ".\chromedriver-win64\chromedriver.exe"
if (Test-Path $chromedriverPath) {
    Start-Process -FilePath $chromedriverPath -ArgumentList "--port=57909", "--allowed-origins=*"
    Write-Host "ChromeDriver started on port 57909" -ForegroundColor Green
    Start-Sleep -Seconds 2
} else {
    Write-Host "Warning: ChromeDriver not found at $chromedriverPath" -ForegroundColor Yellow
}

Write-Host "Starting Leptos server..." -ForegroundColor Cyan
Write-Host "Server will be available at http://127.0.0.1:3000" -ForegroundColor Green
Write-Host ""

cargo leptos serve
