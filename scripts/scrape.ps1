# Trigger scraping via HTTP request (server must be running)
# Usage: .\scripts\scrape.ps1

Write-Host "Triggering scrape..." -ForegroundColor Cyan

try {
    $response = Invoke-WebRequest -Uri "http://127.0.0.1:3000/api/start_single_scrape" -Method POST -TimeoutSec 5 -ErrorAction Stop
    Write-Host "Scrape triggered successfully!" -ForegroundColor Green
    Write-Host "Check http://127.0.0.1:3000 for progress." -ForegroundColor Yellow
} catch {
    Write-Host "Failed to trigger scrape. Is the server running?" -ForegroundColor Red
    Write-Host "Start the server first with: .\scripts\serve.ps1" -ForegroundColor Yellow
}
