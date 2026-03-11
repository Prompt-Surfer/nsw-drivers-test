# Trigger auto-login to the RTA portal
# Can be run standalone after server is up, or sourced from serve.ps1
# Usage: .\scripts\login.ps1 [-ServerUrl "http://127.0.0.1:3001"] [-MaxWaitSeconds 60]

param(
    [string]$ServerUrl = "http://127.0.0.1:3001",
    [int]$MaxWaitSeconds = 60
)

$ErrorActionPreference = "SilentlyContinue"

function Wait-ForServer {
    param([string]$Url, [int]$MaxSeconds)
    Write-Host "Waiting for server at $Url..." -ForegroundColor Cyan
    $deadline = (Get-Date).AddSeconds($MaxSeconds)
    while ((Get-Date) -lt $deadline) {
        try {
            $response = Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 2 -ErrorAction Stop
            if ($response.StatusCode -eq 200) {
                Write-Host "Server is ready." -ForegroundColor Green
                return $true
            }
        } catch { }
        Start-Sleep -Seconds 2
    }
    Write-Host "Timed out waiting for server." -ForegroundColor Red
    return $false
}

function Invoke-Login {
    param([string]$Url)
    Write-Host "Triggering login to RTA portal..." -ForegroundColor Cyan
    try {
        $response = Invoke-RestMethod `
            -Uri "$Url/api/login_to_portal" `
            -Method Post `
            -ContentType "application/x-www-form-urlencoded" `
            -Body "" `
            -UseBasicParsing `
            -TimeoutSec 120 `
            -ErrorAction Stop
        Write-Host "Login result: $response" -ForegroundColor Green
        return $true
    } catch {
        $msg = $_.Exception.Message
        Write-Host "Login request failed: $msg" -ForegroundColor Red
        return $false
    }
}

# If called standalone, wait for server first
if ($MyInvocation.InvocationName -ne ".") {
    $ready = Wait-ForServer -Url $ServerUrl -MaxSeconds $MaxWaitSeconds
    if (-not $ready) { exit 1 }
}

Invoke-Login -Url $ServerUrl
