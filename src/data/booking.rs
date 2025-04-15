use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
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
            let update_interval = Duration::from_secs(6 * 3600);

            while *running_status.read().unwrap() {
                // BookingManager::perform_update(locations.clone(), &file_path, settings.clone())
                //     .await;
                
                BookingManager::update_date();

                tokio::time::sleep(update_interval).await;
            }
        });
    }

    pub fn stop_background_updates() {
        let mut running = get_background_status().write().unwrap();
        *running = false;
    }

    pub async fn perform_update(locations: Vec<String>, file_path: &str, settings: Settings) {
        const CHUNK_SIZE: usize = 1;
        const MAX_RETRIES: usize = 3;
        const RETRY_DELAY: u64 = 5;

        let mut all_results = Vec::new();

        for locations_chunk in locations.chunks(CHUNK_SIZE) {
            let locations_in_chunk: Vec<String> = locations_chunk.iter().cloned().collect();
            let mut tasks = Vec::new();

            for location in &locations_in_chunk {
                let location = location.clone();
                let settings = settings.clone();

                tasks.push(tokio::spawn(async move {
                    Self::scrape_location_with_retries(
                        &location,
                        settings.clone(),
                        MAX_RETRIES,
                        RETRY_DELAY,
                    )
                    .await
                }));
            }

            for task in tasks {
                match task.await {
                    Ok((location, result)) => match result {
                        Ok(booking_data) => {
                            all_results.push(booking_data);
                        }
                        Err(e) => {
                            eprintln!(
                                "Failed to scrape location {} after all retries: {}",
                                location, e
                            );
                        }
                    },
                    Err(e) => {
                        eprintln!("Task panicked: {:?}", e);
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        if !all_results.is_empty() {
            Self::update_data(all_results);
            if let Err(e) = Self::save_to_file(file_path) {
                eprintln!("Failed to save booking data to file: {}", e);
            }
        }
    }

    async fn scrape_location_with_retries(
        location: &str,
        settings: Settings,
        max_retries: usize,
        retry_delay: u64,
    ) -> (String, Result<LocationBookings, String>) {
        let location_str = location.to_string();

        for attempt in 1..=max_retries {
            match super::rta::scrape_rta_timeslots(location, &settings).await {
                Ok(result) => return (location_str, Ok(result)),
                Err(e) => {
                    if attempt == max_retries {
                        return (
                            location_str,
                            Err(format!("Failed after {} attempts: {:?}", max_retries, e)),
                        );
                    }

                    eprintln!(
                        "Error scraping location {} (attempt {}/{}): {:?}",
                        location, attempt, max_retries, e
                    );

                    tokio::time::sleep(Duration::from_secs(retry_delay)).await;
                }
            }
        }

        (
            location_str,
            Err("Unexpected error in retry logic".to_string()),
        )
    }
}
