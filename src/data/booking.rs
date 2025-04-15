use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::hash::{DefaultHasher, Hasher};
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};

use super::shared_booking::{BookingData, LocationBookings, TimeSlot};
use crate::settings::Settings;

static BOOKING_DATA: OnceLock<Arc<RwLock<(BookingData, String)>>> = OnceLock::new();
static BACKGROUND_RUNNING: OnceLock<Arc<RwLock<bool>>> = OnceLock::new();

fn get_booking_data() -> &'static Arc<RwLock<(BookingData, String)>> {
    BOOKING_DATA.get_or_init(|| Arc::new(RwLock::new((BookingData::default(), String::new()))))
}

fn get_background_status() -> &'static Arc<RwLock<bool>> {
    BACKGROUND_RUNNING.get_or_init(|| Arc::new(RwLock::new(false)))
}

pub struct BookingManager;

impl BookingManager {
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

        serde_json::to_string_pretty(&*data_guard)
            .map_err(|e| format!("Failed to serialize data: {}", e))
            .and_then(|json_str| {
                fs::write(file_path, json_str)
                    .map_err(|e| format!("Failed to write to file: {}", e))
            })
    }

    fn clean_data(results: Vec<LocationBookings>) -> Vec<LocationBookings> {
        results.into_iter().map(|mut location| {
            location.slots.retain(|slot| slot.availability);
            location
        }).collect()
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

    pub fn update_data(mut new_results: Vec<LocationBookings>) {
        new_results = Self::clean_data(new_results);
        let updated_data = BookingData {
            results: new_results,
            last_updated: Some(chrono::Utc::now().to_rfc3339()),
        };

        let hash = updated_data.calculate_hash();

        let mut data_guard = get_booking_data().write().unwrap();
        *data_guard = (updated_data, hash);
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
            let update_interval = Duration::from_secs(settings.refresh_time * 3600);

            while *running_status.read().unwrap() {
                BookingManager::perform_update(locations.clone(), &file_path, settings.clone())
                    .await;

                tokio::time::sleep(update_interval).await;
            }
        });
    }

    pub fn stop_background_updates() {
        let mut running = get_background_status().write().unwrap();
        *running = false;
    }

    pub async fn perform_update(locations: Vec<String>, file_path: &str, settings: Settings) {
        let max_retries = settings.retries;

        if locations.is_empty() {
            return;
        }

        let mut final_results: Option<HashMap<String, LocationBookings>> = None;

        for attempt in 1..=max_retries {
            println!("INFO: Scraping attempt {}/{}...", attempt, max_retries);
            match super::rta::scrape_rta_timeslots(locations.clone(), &settings).await // Pass Vec<&str>
            {
                Ok(result_map) => {
                    println!(
                        "INFO: Successfully scraped {} locations in attempt {}.",
                        result_map.len(), attempt
                    );
                    let owned_result_map: HashMap<String, LocationBookings> = result_map
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v))
                        .collect();
                    final_results = Some(owned_result_map);
                    break;
                }
                Err(e) => {
                    eprintln!(
                        "ERROR: Scraping failed on attempt {}/{}: {:?}",
                        attempt, max_retries, e
                    );
                    if attempt == max_retries {
                        eprintln!(
                            "ERROR: Failed to scrape locations after {} attempts. No data will be updated.",
                             max_retries
                        );
                        return;
                    }
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }

        if let Some(result_map) = final_results {
            let all_results: Vec<LocationBookings> = result_map.into_values().collect();
            Self::update_data(all_results);

        }

        if let Err(e) = Self::save_to_file(file_path) {
            eprintln!("ERROR: Failed to save booking data to file '{}': {}", file_path, e);
        } else {
             println!("INFO: Update process complete. Data saved to '{}'.", file_path);
        }
    }

    pub async fn perform_updates(initial_locations: Vec<String>, file_path: &str, settings: Settings) {
        let max_retries = settings.retries;

        let mut remaining_locations: HashSet<String> = initial_locations.into_iter().collect();
        let mut successful_results: HashMap<String, LocationBookings> = HashMap::new();

        for attempt in 1..=max_retries {
            if remaining_locations.is_empty() {
                break;
            }

            let current_batch: Vec<String> = remaining_locations.iter().cloned().collect();

            match super::rta::scrape_rta_timeslots(current_batch, &settings).await {
                Ok(partial_result_map) => {
                    let received_count = partial_result_map.len();

                    for (location_str, booking_data) in partial_result_map {
                        if remaining_locations.remove(&location_str) {
                             successful_results.insert(location_str.clone(), booking_data);
                        }
                    }

                }
                Err(e) => {
                    eprintln!(
                        "Scraping failed entirely on attempt {}/{}: {:?}",
                        attempt, max_retries, e
                    );
                }
            }

            if !remaining_locations.is_empty() && attempt < max_retries {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }

        if !successful_results.is_empty() {
            Self::update_data(successful_results.into_values().collect());
            if let Err(e) = Self::save_to_file(file_path) {
                eprintln!("Failed to save booking data to file: {}", e);
            }
        }
    }

}
