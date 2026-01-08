# Restart the NSW Drivers Test server (stop + serve)
# Usage: .\scripts\restart.ps1

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "Restarting NSW Drivers Test Server..." -ForegroundColor Cyan

# Stop existing processes
& "$ScriptRoot\stop.ps1"

Start-Sleep -Seconds 2

# Start server
& "$ScriptRoot\serve.ps1"
