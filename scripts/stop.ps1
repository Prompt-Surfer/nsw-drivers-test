# Stop all NSW Drivers Test processes
# Usage: .\scripts\stop.ps1

Write-Host "Stopping NSW Drivers Test processes..." -ForegroundColor Cyan

$processes = @("nsw-closest-display", "chromedriver", "cargo", "rustc")

foreach ($proc in $processes) {
    $found = Get-Process | Where-Object { $_.ProcessName -like "*$proc*" }
    if ($found) {
        $found | Stop-Process -Force -ErrorAction SilentlyContinue
        Write-Host "Stopped: $proc" -ForegroundColor Yellow
    }
}

Write-Host "All processes stopped." -ForegroundColor Green
