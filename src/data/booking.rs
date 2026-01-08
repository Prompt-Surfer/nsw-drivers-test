use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::hash::{DefaultHasher, Hasher};
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};

use super::location::LocationManager;
use super::shared_booking::{BookingData, LocationBookings, TimeSlot};
use crate::settings::Settings;

static BOOKING_DATA: OnceLock<Arc<RwLock<(BookingData, String)>>> = OnceLock::new();
static BACKGROUND_RUNNING: OnceLock<Arc<RwLock<bool>>> = OnceLock::new();
static SCRAPING_STATUS: OnceLock<Arc<RwLock<ScrapingStatus>>> = OnceLock::new();
static LOCATION_TIMES: OnceLock<Arc<RwLock<HashMap<String, u64>>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScrapingStatus {
    pub is_running: bool,
    pub current_location: Option<String>,
    pub completed_locations: Vec<String>,
    pub remaining_locations: Vec<String>,
    pub total_locations: usize,
    pub estimated_remaining_secs: Option<u64>,
    pub current_location_start_time: Option<u64>,
    pub error_message: Option<String>,
    pub last_completed_at: Option<String>,
    pub last_duration_secs: Option<u64>,
}

fn get_scraping_status() -> &'static Arc<RwLock<ScrapingStatus>> {
    SCRAPING_STATUS.get_or_init(|| Arc::new(RwLock::new(ScrapingStatus::default())))
}

fn get_location_times() -> &'static Arc<RwLock<HashMap<String, u64>>> {
    LOCATION_TIMES.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

fn get_booking_data() -> &'static Arc<RwLock<(BookingData, String)>> {
    BOOKING_DATA.get_or_init(|| Arc::new(RwLock::new((BookingData::default(), String::new()))))
}

fn get_background_status() -> &'static Arc<RwLock<bool>> {
    BACKGROUND_RUNNING.get_or_init(|| Arc::new(RwLock::new(false)))
}

pub struct BookingManager;

impl BookingManager {
    /// Convert location ID to location name for display
    fn get_location_name(location_id: &str) -> String {
        location_id.parse::<u32>()
            .ok()
            .and_then(|id| LocationManager::new().get_by_id(id))
            .map(|loc| loc.name)
            .unwrap_or_else(|| location_id.to_string())
    }

    pub fn get_data() -> (BookingData, String) {
        get_booking_data().read().unwrap().clone()
    }

    pub fn get_location_data(location_id: String) -> Option<(LocationBookings, String)> {
        Self::get_data()
            .0
            .results
            .iter()
            .find(|booking| booking.location == location_id)
            .and_then(|booking| Some((booking.clone(), booking.calculate_hash())))
    }

    pub fn get_location_slots(location_code: &str) -> Option<Vec<TimeSlot>> {
        let data_guard = get_booking_data().read().unwrap();
        data_guard
            .0
            .results
            .iter()
            .find(|loc| loc.location == location_code)
            .map(|loc| loc.slots.clone())
    }

    pub fn get_available_slots() -> Vec<(String, TimeSlot)> {
        let data_guard = get_booking_data().read().unwrap();
        let mut available = Vec::new();

        for loc in &data_guard.0.results {
            for slot in &loc.slots {
                if slot.availability {
                    available.push((loc.location.clone(), slot.clone()));
                }
            }
        }

        available
    }

    pub fn init_from_file(file_path: &str) -> Result<(), String> {
        if !Path::new(file_path).exists() {
            println!("No path for booking data");
            return Ok(());
        }

        fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read file: {}", e))
            .and_then(|json_str| {
                serde_json::from_str::<BookingData>(&json_str)
                    .map_err(|e| format!("Failed to parse JSON: {}", e))
                    .map(|data| {
                        let hash = data.calculate_hash();
                        let mut data_guard = get_booking_data().write().unwrap();
                        *data_guard = (data, hash);
                    })
            })
    }

    pub fn save_to_file(file_path: &str) -> Result<(), String> {
        let data_guard = get_booking_data().read().unwrap();

        serde_json::to_string_pretty(&data_guard.0)
            .map_err(|e| format!("Failed to serialize data: {}", e))
            .and_then(|json_str| {
                fs::write(file_path, json_str)
                    .map_err(|e| format!("Failed to write to file: {}", e))
            })
    }

    fn clean_data(results: Vec<LocationBookings>, settings: &Settings) -> Vec<LocationBookings> {
        let start_date = settings.date_filter_start.as_ref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let end_date = settings.date_filter_end.as_ref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        
        results
            .into_iter()
            .map(|mut location| {
                location.slots.retain(|slot| {
                    if !slot.availability {
                        return false;
                    }
                    // Apply date filter if configured
                    // start_time format is "DD/MM/YYYY HH:MM"
                    if let Some(date_str) = slot.start_time.split(' ').next() {
                        if let Ok(slot_date) = NaiveDate::parse_from_str(date_str, "%d/%m/%Y") {
                            if let Some(start) = start_date {
                                if slot_date < start {
                                    return false;
                                }
                            }
                            if let Some(end) = end_date {
                                if slot_date > end {
                                    return false;
                                }
                            }
                        }
                    }
                    true
                });
                location
            })
            .collect()
    }

    pub fn update_date() {
        let (cloned_results, new_hash_data) = {
            let data_read_guard = get_booking_data().read().unwrap();

            let new_data = BookingData {
                results: data_read_guard.0.results.clone(),
                last_updated: Some(chrono::Utc::now().to_rfc3339()),
            };

            let new_hash = new_data.calculate_hash();
            (new_data, new_hash)
        };

        let mut data_guard = get_booking_data().write().unwrap();
        *data_guard = (cloned_results, new_hash_data);
    }

    pub fn update_data(mut new_results: Vec<LocationBookings>, settings: &Settings) {
        new_results = Self::clean_data(new_results, settings);
        let updated_data = BookingData {
            results: new_results,
            last_updated: Some(chrono::Utc::now().to_rfc3339()),
        };

        let hash = updated_data.calculate_hash();

        let mut data_guard = get_booking_data().write().unwrap();
        *data_guard = (updated_data, hash);
    }
    
    /// Merge a single location result into existing data (for incremental updates)
    pub fn merge_location_data(location_booking: LocationBookings, settings: &Settings) {
        let cleaned = Self::clean_data(vec![location_booking], settings);
        if cleaned.is_empty() {
            return;
        }
        let location_booking = cleaned.into_iter().next().unwrap();
        
        let mut data_guard = get_booking_data().write().unwrap();
        
        // Remove existing data for this location
        data_guard.0.results.retain(|loc| loc.location != location_booking.location);
        
        // Add new data
        data_guard.0.results.push(location_booking);
        data_guard.0.last_updated = Some(chrono::Utc::now().to_rfc3339());
        
        // Update hash
        data_guard.1 = data_guard.0.calculate_hash();
    }

    pub fn start_background_updates(locations: Vec<String>, file_path: String, settings: Settings) {
        {
            let mut running = get_background_status().write().unwrap();
            if *running {
                return;
            }
            *running = true;
        }

        let running_status = Arc::clone(get_background_status());

        tokio::spawn(async move {
            let update_interval = Duration::from_secs(settings.scrape_refresh_time_min * 60);

            while *running_status.read().unwrap() {
                BookingManager::perform_update(locations.clone(), &file_path, settings.clone())
                    .await;

                tokio::time::sleep(update_interval).await;
            }
        });
    }
    
    /// Trigger a single scraping run (no repeat loop)
    pub fn trigger_single_scrape(locations: Vec<String>, file_path: String, settings: Settings) -> Result<(), String> {
        // Check if already running
        if Self::is_scraping_running() {
            return Err("Scraping is already in progress".to_string());
        }
        
        // Initialize status
        Self::update_scraping_status(|status| {
            status.is_running = true;
            status.current_location = None;
            status.completed_locations = vec![];
            status.remaining_locations = locations.clone();
            status.total_locations = locations.len();
            status.estimated_remaining_secs = Self::calculate_remaining_estimate(&locations);
            status.error_message = None;
        });
        
        tokio::spawn(async move {
            BookingManager::perform_update_with_tracking(locations, &file_path, settings).await;
            
            // Mark as complete
            Self::update_scraping_status(|status| {
                status.is_running = false;
                status.current_location = None;
            });
        });
        
        Ok(())
    }

    pub fn stop_background_updates() {
        let mut running = get_background_status().write().unwrap();
        *running = false;
        
        // Also update scraping status
        let mut status = get_scraping_status().write().unwrap();
        status.is_running = false;
    }
    
    pub fn get_scraping_status() -> ScrapingStatus {
        get_scraping_status().read().unwrap().clone()
    }
    
    pub fn is_scraping_running() -> bool {
        get_scraping_status().read().unwrap().is_running
    }
    
    fn update_scraping_status(f: impl FnOnce(&mut ScrapingStatus)) {
        let mut status = get_scraping_status().write().unwrap();
        f(&mut status);
    }
    
    fn record_location_time(location: &str, duration_secs: u64) {
        let mut times = get_location_times().write().unwrap();
        times.insert(location.to_string(), duration_secs);
    }
    
    fn get_estimated_time(location: &str) -> Option<u64> {
        let times = get_location_times().read().unwrap();
        times.get(location).copied()
    }
    
    fn calculate_remaining_estimate(remaining: &[String]) -> Option<u64> {
        let times = get_location_times().read().unwrap();
        if times.is_empty() {
            return None;
        }
        
        let total: u64 = remaining.iter()
            .filter_map(|loc| times.get(loc).copied())
            .sum();
        
        // For locations without history, use average
        let known_count = remaining.iter().filter(|loc| times.contains_key(*loc)).count();
        let unknown_count = remaining.len() - known_count;
        
        if unknown_count > 0 && !times.is_empty() {
            let avg: u64 = times.values().sum::<u64>() / times.len() as u64;
            Some(total + (unknown_count as u64 * avg))
        } else if total > 0 {
            Some(total)
        } else {
            None
        }
    }

    pub async fn perform_update(locations: Vec<String>, file_path: &str, settings: Settings) {
        let start_time = Instant::now();
        let max_retries = settings.retries;
        let total_locations = locations.len();

        let mut scraped_count = 0usize;
        let mut remaining_locations = locations.clone();

        for attempt in 1..=max_retries {
            if remaining_locations.is_empty() {
                println!("INFO: All locations successfully scraped.");
                break;
            }

            println!(
                "INFO: Scraping attempt {}/{} for {} locations...",
                attempt,
                max_retries,
                remaining_locations.len()
            );

            match super::rta::scrape_rta_timeslots_incremental(
                remaining_locations.clone(), 
                &settings,
                file_path,
                |location_booking, fp| {
                    // Callback for each successfully scraped location
                    Self::merge_location_data(location_booking, &settings);
                    
                    // Save incrementally after each location
                    if let Err(e) = Self::save_to_file(fp) {
                        eprintln!("WARN: Failed to save incremental update: {}", e);
                    }
                }
            ).await {
                Ok(successful_locations) => {
                    println!(
                        "INFO: Successfully scraped {}/{} locations in attempt {}.",
                        successful_locations.len(),
                        remaining_locations.len(),
                        attempt
                    );

                    scraped_count += successful_locations.len();
                    remaining_locations.retain(|loc| !successful_locations.contains(loc));

                    if remaining_locations.is_empty() {
                        println!(
                            "INFO: All locations successfully scraped after {} attempts.",
                            attempt
                        );
                        break;
                    } else {
                        println!(
                            "WARN: {} locations still need to be scraped.",
                            remaining_locations.len()
                        );
                    }
                }
                Err(e) => {
                    eprintln!(
                        "ERROR: Scraping failed on attempt {}/{}: {:?}",
                        attempt, max_retries, e
                    );

                    if attempt == max_retries {
                        eprintln!(
                            "ERROR: Failed to scrape {} locations after {} attempts.",
                            remaining_locations.len(),
                            max_retries
                        );
                        if scraped_count == 0 {
                            eprintln!("ERROR: No data was successfully scraped.");
                        } else {
                            eprintln!(
                                "WARNING: Partial data collected. Successfully scraped {}/{} locations.",
                                scraped_count, total_locations
                            );
                        }
                    }
                }
            }

            if attempt < max_retries && !remaining_locations.is_empty() {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }

        // Final save to ensure everything is persisted
        if let Err(e) = Self::save_to_file(file_path) {
            eprintln!(
                "ERROR: Failed to save booking data to file '{}': {}",
                file_path, e
            );
        }

        let elapsed = start_time.elapsed();
        let minutes = elapsed.as_secs() / 60;
        let seconds = elapsed.as_secs() % 60;
        let millis = elapsed.subsec_millis();
        println!(
            "INFO: Total scraping time: {}m {}s {}ms ({}/{} locations)",
            minutes,
            seconds,
            millis,
            scraped_count,
            total_locations
        );
    }
    
    /// Perform update with status tracking for UI
    pub async fn perform_update_with_tracking(locations: Vec<String>, file_path: &str, settings: Settings) {
        use std::time::Instant as StdInstant;
        
        let start_time = Instant::now();
        let max_retries = settings.retries;
        let total_locations = locations.len();

        let mut scraped_count = 0usize;
        let mut remaining_locations = locations.clone();

        for attempt in 1..=max_retries {
            if remaining_locations.is_empty() {
                println!("INFO: All locations successfully scraped.");
                break;
            }

            println!(
                "INFO: Scraping attempt {}/{} for {} locations...",
                attempt,
                max_retries,
                remaining_locations.len()
            );

            // Track timing per location
            let location_start_times: std::sync::Arc<std::sync::Mutex<HashMap<String, StdInstant>>> = 
                std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
            
            let times_clone = location_start_times.clone();
            let settings_clone = settings.clone();
            
            // Update status before starting
            Self::update_scraping_status(|status| {
                status.remaining_locations = remaining_locations.clone();
                status.estimated_remaining_secs = Self::calculate_remaining_estimate(&remaining_locations);
            });

            match super::rta::scrape_rta_timeslots_incremental(
                remaining_locations.clone(), 
                &settings,
                file_path,
                move |location_booking, fp| {
                    let location = location_booking.location.clone();
                    
                    // Record timing for this location
                    let mut times = times_clone.lock().unwrap();
                    if let Some(start) = times.remove(&location) {
                        let duration = start.elapsed().as_secs();
                        Self::record_location_time(&location, duration);
                    }
                    
                    // Merge data
                    Self::merge_location_data(location_booking, &settings_clone);
                    
                    // Update status
                    Self::update_scraping_status(|status| {
                        status.completed_locations.push(location.clone());
                        status.remaining_locations.retain(|l| l != &location);
                        status.current_location = status.remaining_locations.first()
                            .map(|id| Self::get_location_name(id));
                        status.estimated_remaining_secs = Self::calculate_remaining_estimate(&status.remaining_locations);
                    });
                    
                    // Save incrementally
                    if let Err(e) = Self::save_to_file(fp) {
                        eprintln!("WARN: Failed to save incremental update: {}", e);
                    }
                }
            ).await {
                Ok(successful_locations) => {
                    println!(
                        "INFO: Successfully scraped {}/{} locations in attempt {}.",
                        successful_locations.len(),
                        remaining_locations.len(),
                        attempt
                    );

                    scraped_count += successful_locations.len();
                    remaining_locations.retain(|loc| !successful_locations.contains(loc));

                    if remaining_locations.is_empty() {
                        println!(
                            "INFO: All locations successfully scraped after {} attempts.",
                            attempt
                        );
                        break;
                    } else {
                        println!(
                            "WARN: {} locations still need to be scraped.",
                            remaining_locations.len()
                        );
                    }
                }
                Err(e) => {
                    eprintln!(
                        "ERROR: Scraping failed on attempt {}/{}: {:?}",
                        attempt, max_retries, e
                    );
                    
                    Self::update_scraping_status(|status| {
                        status.error_message = Some(format!("Attempt {}/{} failed: {:?}", attempt, max_retries, e));
                    });

                    if attempt == max_retries {
                        eprintln!(
                            "ERROR: Failed to scrape {} locations after {} attempts.",
                            remaining_locations.len(),
                            max_retries
                        );
                    }
                }
            }

            if attempt < max_retries && !remaining_locations.is_empty() {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }

        // Final save
        if let Err(e) = Self::save_to_file(file_path) {
            eprintln!(
                "ERROR: Failed to save booking data to file '{}': {}",
                file_path, e
            );
        }

        // Check for alerts after scraping
        Self::check_and_notify_alerts(&settings);

        let elapsed = start_time.elapsed();
        let duration_secs = elapsed.as_secs();
        
        // Update status with completion info
        Self::update_scraping_status(|status| {
            status.last_completed_at = Some(chrono::Utc::now().to_rfc3339());
            status.last_duration_secs = Some(duration_secs);
        });
        
        println!(
            "INFO: Total scraping time: {}m {}s ({}/{} locations)",
            duration_secs / 60,
            duration_secs % 60,
            scraped_count,
            total_locations
        );
    }

    /// Check available slots against alert configurations and send notifications
    pub fn check_and_notify_alerts(settings: &Settings) {
        use crate::data::location::LocationManager;
        use crate::notifications::NotificationManager;

        if !settings.alerts_enabled || settings.alerts.is_empty() {
            return;
        }

        let location_manager = LocationManager::new();
        let data_guard = get_booking_data().read().unwrap();
        
        // Build available slots with location names
        let available_slots: Vec<_> = data_guard.0.results
            .iter()
            .map(|loc| {
                let location_name = location_manager
                    .get_by_id(loc.location.parse().unwrap_or(0))
                    .map(|l| l.name.clone())
                    .unwrap_or_else(|| loc.location.clone());
                (loc.location.clone(), location_name, loc.slots.clone())
            })
            .collect();

        // Check for matches
        let matches = NotificationManager::check_for_matches(&settings.alerts, &available_slots);
        
        if !matches.is_empty() {
            println!("INFO: Found {} matching slots for alerts", matches.len());
            
            // Process and get new alerts
            let new_alerts = NotificationManager::process_matches(matches);
            
            if !new_alerts.is_empty() {
                println!("INFO: Sending {} new notifications", new_alerts.len());
                NotificationManager::notify_new_alerts(&new_alerts);
            }
        }
    }
}
