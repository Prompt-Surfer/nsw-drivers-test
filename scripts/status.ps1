# Check scraping status (server must be running)
# Usage: .\scripts\status.ps1

Write-Host "Checking scraping status..." -ForegroundColor Cyan

try {
    $response = Invoke-WebRequest -Uri "http://127.0.0.1:3001/api/get_scraping_status" -Method GET -TimeoutSec 5 -ErrorAction Stop
    $status = $response.Content | ConvertFrom-Json
    
    Write-Host ""
    if ($status.is_running) {
        Write-Host "Status: RUNNING" -ForegroundColor Yellow
        Write-Host "Current location: $($status.current_location)" -ForegroundColor Cyan
        Write-Host "Progress: $($status.completed_count)/$($status.total_count) locations" -ForegroundColor Cyan
        if ($status.estimated_remaining_secs -gt 0) {
            $mins = [math]::Floor($status.estimated_remaining_secs / 60)
            $secs = $status.estimated_remaining_secs % 60
            Write-Host "ETA: ${mins}m ${secs}s remaining" -ForegroundColor Cyan
        }
    } else {
        Write-Host "Status: IDLE" -ForegroundColor Green
        if ($status.last_duration_secs -gt 0) {
            Write-Host "Last scrape: $($status.last_duration_secs)s" -ForegroundColor Gray
        }
    }
    
    if ($status.error_message) {
        Write-Host "Error: $($status.error_message)" -ForegroundColor Red
    }
} catch {
    Write-Host "Failed to get status. Is the server running?" -ForegroundColor Red
    Write-Host "Start the server first with: .\scripts\serve.ps1" -ForegroundColor Yellow
}
