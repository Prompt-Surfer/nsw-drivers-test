# NSW Drivers Test - Find Available Test Times

A modern, efficient tool for checking driving test availability across Service NSW centers.


## Overview

NSW Drivers Test is a web application that helps learner drivers find the earliest available driving test appointments across all Service NSW locations. Instead of manually checking each location, this tool aggregates availability data and sorts locations by distance from your chosen address/location.

Originally based on the scraping script from [sbmkvp](https://github.com/sbmkvp/rta_booking_information), made because that repo seems to be dead since 7/3/25 and their release goals for new features haven't been met

## Technologies Used

- **Rust** - Core application logic with strong safety guarantees
- **Tokio** - Asynchronous runtime for efficient concurrent operations
- **Leptos** - Fast, reactive web framework that compiles to WebAssembly
- **Serde** - Serialization/deserialization framework
- **OpenStreetMap Nominatim API** - Geocoding for location-based searches
- **WebAssembly** - For client-side processing of location data
- **Tailwind CSS** - For responsive, modern UI design

## Features

- 🔍 **Location Search**: Find Service NSW centers by address, suburb, or postcode
- 📍 **Distance Calculation**: View centers ordered by distance from your location
- 📅 **Availability Tracking**: See the earliest available test slot for each location
- 🔄 **Auto Refresh**: Data automatically refreshes to keep information current
- 🔒 **Privacy-focused**: Location searches processed locally in your browser
- 📱 **Responsive Design**: Works on desktop, tablet, and mobile devices
- 🚫 **No Login Required**: No Service NSW credentials needed to view availability

## Installation

### Prerequisites

- Rust and Cargo (latest nightly version)
- Docker-Compose 

### Setup

```bash
# Clone the repository
git clone https://github.com/teehee567/nsw-drivers-test.git
cd nsw-drivers-test

# Run Docker Compose
docker-compose up -d

```

## Usage

1. Visit the application in your browser (default: `http://localhost:8082`)
2. Enter your address, suburb, or postcode in the search box
3. View driving test centers sorted by distance from your location
4. See the earliest available time slot for each center
5. Use the refresh button to get the latest availability data

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Disclaimer

- Not affiliated with Service NSW or the New South Wales Government

## License

This project is licensed under the GPL3 License - see the LICENSE file for details.
