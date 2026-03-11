# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

NSW Drivers Test is a full-stack Rust/Leptos web application that scrapes Service NSW for the earliest available driving test appointments across test centres, sorted by distance from a user-specified address.

## Build & Run Commands

```powershell
# Development (hot reload)
./scripts/dev.ps1       # or: cargo leptos watch

# Production server (starts ChromeDriver automatically)
./scripts/serve.ps1     # or: cargo leptos serve

# Release build
./scripts/build.ps1     # or: cargo leptos build --release

# Helper scripts
./scripts/scrape.ps1    # Trigger scraping via API
./scripts/status.ps1    # Check scraping status
./scripts/login.ps1     # Manual login helper
./scripts/restart.ps1   # Stop and restart
./scripts/stop.ps1      # Stop all processes
```

Requires nightly Rust and `wasm32-unknown-unknown` target. Credentials in `.env` (see `.envexample`).

## Architecture

**Tech stack:** Rust + [Leptos](https://leptos.dev/) (SSR + WASM hydration) + Axum + Thirtyfour (Selenium) + Tailwind CSS v4

### Module Structure

| Path | Purpose |
|------|---------|
| `src/main.rs` | Axum server, routes, app initialization |
| `src/app.rs` | Leptos router, HTML shell |
| `src/settings.rs` | YAML config loading with env var substitution |
| `src/notifications.rs` | Slot alert generation and state |
| `src/data/booking.rs` | `BookingManager` — in-memory state, file I/O, scraping orchestration |
| `src/data/rta.rs` | Selenium web scraping against MyRTA portal |
| `src/data/location.rs` | Location DB with Haversine distance calculations |
| `src/data/shared_booking.rs` | Shared data structures (`TimeSlot`, `LocationBookings`) |
| `src/pages/home.rs` | Main UI page + all Leptos server functions |
| `src/utils/geocoding.rs` | OpenStreetMap Nominatim geocoding with caching |

### Data Flow

1. `BookingManager` orchestrates scraping via `data/rta.rs` (Selenium/Thirtyfour)
2. Results stored in memory and persisted to `data/bookings.json`
3. Leptos server functions (in `pages/home.rs`) expose data to the frontend via RPC
4. Client-side WASM handles geocoding, distance sorting, and interactivity

### State Management

Global state uses `OnceLock<Arc<RwLock<T>>>` pattern:
- `BOOKING_DATA`, `SCRAPING_STATUS`, `SCHEDULER_STATE`, `LOCATION_TIMES`, `NOTIFICATION_STATE`

### Parallel Scraping

Pull-based work queue (`Arc<Mutex<Vec<_>>>`): workers dynamically pull the next location from the shared queue. Configured via `settings.yaml` (`parallel_workers`, default 1).

### Configuration

`settings.yaml` — key fields:
- `headless` — Chrome headless mode
- `have_booking` — whether user already has a test booking (changes scraping strategy)
- `parallel_workers` — number of concurrent scrapers
- `auto_scrape.interval_minutes` — refresh interval
- `alerts` — location/month-based slot notifications
- Credentials via `${MYRTA_USERNAME}` / `${MYRTA_PASSWORD}` env vars

## Development Rules (from `docs/development-rules.md`)

**Async task cancellation:** Use generation counters (`AtomicU64`) to invalidate stale background tasks — never fire-and-forget long-running loops.

**Configuration validation:** Validate at load time AND use site with safe defaults (`.max(1)`, `.clamp(1, 10)`). Zero is almost never valid for intervals/counts.

**Read config inside loops:** Long-lived tasks should re-read config each iteration, not capture values at spawn time.

**Work distribution:** Prefer pull-based shared queues over pre-assigned chunks for dynamic load balancing.

**Logging:** Always include source/context in log messages (worker ID, generation counter, trigger source).

**Idempotency:** Functions like `start_scheduler()` must be safe to call multiple times — use generation counters to invalidate old instances rather than guarding with state checks alone.

**Safety bounds:** Always enforce: `interval.max(1)`, `retries.min(10)`, `workers.clamp(1, 10)`.
