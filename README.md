# NSW Drivers Test - Find Available Test Times

A modern, efficient tool for checking driving test availability across Service NSW centers with parallel scraping and smart alerts.

![Home Page](dev/images/homepage.png)

## Overview

NSW Drivers Test is a web application that helps learner drivers find the earliest available driving test appointments across all Service NSW locations. Instead of manually checking each location, this tool aggregates availability data and sorts locations by distance from your chosen address/location.

## Technologies Used

- **Rust** - Core application logic with strong safety guarantees
- **Leptos** - Full-stack web framework with SSR + WASM hydration
- **Axum** - High-performance async web server
- **Tokio** - Asynchronous runtime for efficient concurrent operations
- **Thirtyfour** - Selenium WebDriver client for browser automation
- **Serde** - Serialization/deserialization framework
- **OpenStreetMap Nominatim API** - Geocoding for location-based searches
- **WebAssembly** - For client-side processing of location data
- **Tailwind CSS v4** - For responsive, modern UI design

## Features

- **Location Search**: Find Service NSW centers by address, suburb, or postcode
- **Distance Calculation**: View centers ordered by distance from your location
- **Availability Tracking**: See the earliest available test slot for each location
- **Parallel Scraping**: Configurable multi-worker scraping for faster data collection
- **Auto Scraping**: Automatic refresh on configurable intervals to keep data current
- **Smart Alerts**: Location and date-based notifications for available slots
- **Manual Controls**: Login, scrape, and stop buttons for on-demand control
- **Date Range Filtering**: Filter slots by specific date ranges
- **Privacy-focused**: Location searches processed locally in your browser
- **Responsive Design**: Works on desktop, tablet, and mobile devices
- **Auto-Login**: Automatic MyRTA login on server start (when credentials configured)

## Installation

### Prerequisites

- Rust and Cargo (nightly version)
- Node.js and npm
- Chrome browser
- ChromeDriver (matching your Chrome version)

### Quick Setup (Docker)

```bash
# Clone the repository
git clone https://github.com/Prompt-Surfer/nsw-drivers-test.git
cd nsw-drivers-test

# Create .env file with your MyRTA credentials
cp .envexample .env
# Edit .env with your credentials

# Run with Docker Compose
docker-compose up -d
```

### Manual Setup (Windows)

```powershell
# 1. Install Rust (nightly)
# Download from https://rustup.rs and run installer
rustup default nightly
rustup target add wasm32-unknown-unknown

# 2. Install cargo-leptos
cargo install --locked cargo-leptos

# 3. Install npm dependencies
npm install

# 4. Download ChromeDriver
# Get from https://googlechromelabs.github.io/chrome-for-testing/
# Extract to ./chromedriver-win64/

# 5. Create .env file
# Use MYRTA_USERNAME and MYRTA_PASSWORD (not USERNAME/PASSWORD to avoid Windows conflicts)
echo "MYRTA_USERNAME=your_licence_number" > .env
echo "MYRTA_PASSWORD=your_password" >> .env
```

### Manual Setup (Linux/macOS)

```bash
# 1. Install Rust (nightly)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default nightly
rustup target add wasm32-unknown-unknown

# 2. Install cargo-leptos
cargo install --locked cargo-leptos

# 3. Install npm dependencies
npm install

# 4. Install ChromeDriver
# Ubuntu/Debian: sudo apt install chromium-chromedriver
# macOS: brew install chromedriver

# 5. Create .env file
cp .envexample .env
# Edit with your MyRTA credentials
```

## Scripts (Windows PowerShell)

Convenient scripts are provided in the `scripts/` folder:

| Script | Description |
|--------|-------------|
| `.\scripts\serve.ps1` | Start the server (loads .env, starts ChromeDriver, auto-login) |
| `.\scripts\stop.ps1` | Stop all running processes |
| `.\scripts\restart.ps1` | Restart the server (stop + serve) |
| `.\scripts\dev.ps1` | Start in development mode with hot-reload |
| `.\scripts\build.ps1` | Build the application |
| `.\scripts\scrape.ps1` | Trigger scraping via API (server must be running) |
| `.\scripts\status.ps1` | Check scraping status (server must be running) |
| `.\scripts\login.ps1` | Manual login helper for testing authentication |

```powershell
# Example usage
.\scripts\serve.ps1    # Start server
.\scripts\scrape.ps1   # Trigger scrape
.\scripts\status.ps1   # Check progress
.\scripts\stop.ps1     # Stop everything
```

## Command Line Guide

### Starting the Application (Manual)

```bash
# Start ChromeDriver (required for scraping)
# Windows:
.\chromedriver-win64\chromedriver.exe --port=57909 --allowed-origins=*

# Linux/macOS:
chromedriver --port=57909 --allowed-origins=*

# In a new terminal, start the web server
cargo leptos serve
# Server runs at http://127.0.0.1:3000
# Note: Auto-login happens automatically if credentials are configured
```

### Development Mode (with hot-reload)

```bash
cargo leptos watch
```

### Building for Production

```bash
cargo leptos build --release
```

### Running the Built Binary

```bash
./target/release/nsw-closest-display
```

### Stopping All Processes (Windows PowerShell)

```powershell
Get-Process | Where-Object { $_.ProcessName -like "*nsw-closest*" } | Stop-Process -Force
Get-Process | Where-Object { $_.ProcessName -like "*chromedriver*" } | Stop-Process -Force
```

### Stopping All Processes (Linux/macOS)

```bash
pkill -f nsw-closest
pkill -f chromedriver
```

## Configuration

### settings.yaml Options

```yaml
headless: true                    # Run Chrome in headless mode (false to debug)
username: "${MYRTA_USERNAME}"     # MyRTA username from .env
password: "${MYRTA_PASSWORD}"     # MyRTA password from .env
have_booking: false               # Set true if you already have a booking
selenium_driver_url: "http://localhost:57909"
selenium_element_timout: 20000    # Element wait timeout (ms)
selenium_element_polling: 100     # Polling interval (ms)
retries: 4                        # Number of retry attempts
parallel_workers: 3               # Number of concurrent scraping workers (1-10)

# Auto-scraping configuration
auto_scrape_enabled: true         # Enable automatic scraping
auto_scrape_interval_hours: 2     # Auto-refresh interval (0 = disabled)

# Filter to specific locations (empty = all locations)
locations:
  - "96"    # Ryde
  - "621"   # North Rocks
  # Add more location IDs from data/centres.json

# Date range filter (optional)
date_filter_start: "2026-02-01"
date_filter_end: "2026-09-30"

# Alert notifications for specific locations/dates
alerts_enabled: true              # Enable alert system
alerts:
  - location_id: "621"
    location_name: "North Rocks"
    enabled: true
    months: [3, 4, 5, 6]          # Alert for slots in March-June
```

### Finding Location IDs

Location IDs can be found in `data/centres.json`. Common Sydney locations:

| Location | ID |
|----------|-----|
| Auburn | 112 |
| Auburn PCYC | 601 |
| Bankstown | 20 |
| Blacktown | 26 |
| Castle Hill | 35 |
| Chatswood | 37 |
| Hornsby | 63 |
| Liverpool | 74 |
| North Rocks | 621 |
| Parramatta (Silverwater) | 241 |
| Ryde | 96 |

## Usage

1. Visit the application in your browser (default: `http://localhost:3000`)
2. **First Run**: Click "Login" to authenticate with MyRTA (auto-login on server start if configured)
3. **Start Scraping**: Click "Start Scraping" to begin collecting availability data
   - Progress is shown with live updates
   - Parallel workers speed up the process
4. **Search Location**: Enter your address, suburb, or postcode in the search box
5. **View Results**: See driving test centers sorted by distance from your location
6. **Check Availability**: View the earliest available time slot for each center
7. **Alerts**: Configure alerts in `settings.yaml` to get notified of specific availability
8. **Auto-Refresh**: Enable auto-scraping for automatic data updates at configured intervals

## Troubleshooting

### "cargo not recognized" (Windows)
Restart your terminal or refresh PATH:
```powershell
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
```

### Wrong credentials being used (Windows)
Windows has a system `USERNAME` variable. Use `MYRTA_USERNAME` instead in your `.env` file.

### ChromeDriver version mismatch
Ensure ChromeDriver version matches your Chrome browser version. This fork uses ChromeDriver v145. Check Chrome version at `chrome://version/` and download matching ChromeDriver from [Chrome for Testing](https://googlechromelabs.github.io/chrome-for-testing/)

### Scraping fails with "Book test not found"
- Verify your MyRTA credentials are correct
- Set `headless: false` in settings.yaml to watch the browser
- Check if MyRTA requires captcha or 2FA

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Disclaimer

- Not affiliated with Service NSW or the New South Wales Government

## License

This project is licensed under the GPL3 License - see the LICENSE file for details.

## References
[sbmkvp](https://github.com/sbmkvp/rta_booking_information)
