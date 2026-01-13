use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use thirtyfour::components::SelectElement;
use thirtyfour::prelude::*;
use thirtyfour::{By, DesiredCapabilities, WebDriver};
use tokio::sync::{mpsc, Mutex};

use super::location::LocationManager;
use super::shared_booking::{LocationBookings, TimeSlot};
use crate::settings::Settings;

/// Result from a single worker's scraping session
#[derive(Debug)]
pub struct WorkerResult {
    pub worker_id: u8,
    pub successful_locations: Vec<String>,
    pub results: Vec<LocationBookings>,
}

async fn random_sleep(min_millis: u64, max_millis: u64) {
    if min_millis >= max_millis {
        tokio::time::sleep(Duration::from_millis(min_millis)).await;
        return;
    }
    let duration = rand::thread_rng().gen_range(min_millis..max_millis);
    tokio::time::sleep(Duration::from_millis(duration)).await;
}

async fn type_like_human(
    element: &WebElement,
    text: &str,
    min_delay_ms: u64,
    max_delay_ms: u64,
) -> WebDriverResult<()> {
    for char in text.chars() {
        element.send_keys(char.to_string()).await?;
        random_sleep(min_delay_ms, max_delay_ms).await;
    }
    Ok(())
}

async fn log_page_snapshot(driver: &WebDriver, label: &str) {
    if let Ok(url) = driver.current_url().await {
        eprintln!("[PAGE SNAPSHOT - {}] URL: {}", label, url);
    }

    if let Ok(title) = driver.title().await {
        eprintln!("[PAGE SNAPSHOT - {}] Title: {}", label, title);
    }

    if let Ok(ready_state) = driver.execute("return document.readyState;", vec![]).await {
        eprintln!(
            "[PAGE SNAPSHOT - {}] readyState: {}",
            label,
            ready_state.json()
        );
    }

    if let Ok(source) = driver.page_source().await {
        let preview: String = source.chars().take(400).collect();
        eprintln!(
            "[PAGE SNAPSHOT - {}] HTML (first 400 chars): {}",
            label,
            preview.replace('\n', " ")
        );
    }
}

/// Create a configured WebDriver instance
async fn create_driver(settings: &Settings) -> WebDriverResult<WebDriver> {
    let mut caps = DesiredCapabilities::chrome();
    if settings.headless {
        caps.add_arg("--headless=new")?;
    }

    caps.add_arg("--no-sandbox")?;
    caps.add_arg("--disable-dev-shm-usage")?;
    caps.add_arg("--disable-gpu")?;
    caps.add_arg("--window-size=1920,1080")?;
    caps.add_arg("--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36")?;

    let driver = WebDriver::new(settings.selenium_driver_url.clone(), caps).await?;

    driver
        .execute(
            r#"
        Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
        window.chrome = window.chrome || {};
        window.chrome.runtime = window.chrome.runtime || {};
        try {
            let key = Object.keys(window).find(key => key.startsWith('cdc_'));
            if (key) { delete window[key]; }
            let docKey = Object.keys(document).find(key => key.startsWith('cdc_'));
            if (docKey) { delete document[docKey]; }
        } catch (e) { console.debug('Error removing cdc keys:', e); }
    "#,
            Vec::new(),
        )
        .await?;

    Ok(driver)
}

/// Login to RTA and navigate to the location selection page
async fn login_and_navigate_to_booking(driver: &WebDriver, settings: &Settings) -> WebDriverResult<()> {
    let timeout = Duration::from_millis(settings.selenium_element_timout);
    let polling = Duration::from_millis(settings.selenium_element_polling);

    driver
        .goto("https://www.myrta.com/wps/portal/extvp/myrta/login/")
        .await?;
    random_sleep(1000, 2000).await;

    let username_input = driver
        .query(By::Id("widget_cardNumber"))
        .wait(timeout, polling)
        .first()
        .await?;
    random_sleep(200, 500).await;
    
    // Clear any existing text before typing (handles retry scenarios with leftover text)
    username_input.clear().await?;
    random_sleep(100, 200).await;
    
    type_like_human(&username_input, &settings.username, 60, 180).await?;
    random_sleep(300, 700).await;

    let password_input = driver
        .query(By::Id("widget_password"))
        .wait(timeout, polling)
        .first()
        .await?;

    random_sleep(200, 500).await;
    
    // Clear any existing text before typing
    password_input.clear().await?;
    random_sleep(100, 200).await;
    
    type_like_human(&password_input, &settings.password, 60, 180).await?;
    random_sleep(400, 800).await;

    let next_button = driver
        .query(By::Id("nextButton"))
        .wait(timeout, polling)
        .first()
        .await?;
    random_sleep(250, 600).await;
    next_button.click().await?;

    random_sleep(2000, 4000).await;

    if settings.have_booking {
        let manage_booking = driver
            .query(By::XPath("//*[text()=\"Manage booking\"]"))
            .first()
            .await?;
        manage_booking
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        manage_booking.click().await?;
        random_sleep(1500, 2500).await;

        let change_location = driver.query(By::Id("changeLocationButton")).first().await?;
        change_location
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        change_location.click().await?;
        random_sleep(1000, 2000).await;
    } else {
        let book_test = driver
            .query(By::XPath("//*[text()='Book test']"))
            .wait(timeout, polling)
            .first()
            .await?;
        random_sleep(200, 500).await;
        book_test.click().await?;
        random_sleep(1500, 2500).await;

        let car_option = driver.query(By::Id("CAR")).first().await?;
        car_option
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        car_option.click().await?;
        random_sleep(500, 1000).await;

        let test_item = driver
            .query(By::XPath(
                "//fieldset[@id='DC']/span[contains(@class, 'rms_testItemResult')]",
            ))
            .first()
            .await?;
        test_item
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        test_item.click().await?;
        random_sleep(500, 1000).await;

        let next_button = driver.query(By::Id("nextButton")).first().await?;
        next_button
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        next_button.click().await?;
        random_sleep(1500, 2500).await;

        let check_terms = driver.query(By::Id("checkTerms")).first().await?;
        check_terms
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(100, 300).await;
        check_terms.click().await?;
        random_sleep(500, 1000).await;

        let next_button_terms = driver.query(By::Id("nextButton")).first().await?;
        next_button_terms
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        next_button_terms.click().await?;
        random_sleep(1000, 2000).await;
    }

    Ok(())
}

/// Login to RTA portal and navigate to driving test site selector page
/// The browser stays open for manual interaction
pub async fn login_to_portal_only(settings: &Settings) -> WebDriverResult<String> {
    println!("INFO: Opening browser and logging into RTA portal...");
    
    let driver = create_driver(settings).await?;
    
    login_and_navigate_to_booking(&driver, settings).await?;
    
    // Get the current URL to confirm we're on the right page
    let current_url = driver.current_url().await?;
    println!("INFO: Successfully logged in. Browser is now on: {}", current_url);
    
    // Don't quit the driver - leave browser open for user
    // The driver handle will be dropped but browser stays open
    std::mem::forget(driver);
    
    Ok(format!("Logged in successfully. Browser is open at the site selector page."))
}

/// Scrape a single location and return the result
async fn scrape_single_location(
    driver: &WebDriver,
    location: &str,
    settings: &Settings,
) -> WebDriverResult<LocationBookings> {
    let timeout = Duration::from_millis(settings.selenium_element_timout);
    let polling = Duration::from_millis(settings.selenium_element_polling);

    random_sleep(1000, 2000).await;

    let location_select_dropdown = driver.query(By::Id("rms_batLocLocSel")).first().await?;
    location_select_dropdown.wait_until().wait(timeout, polling).displayed().await?;
    random_sleep(200, 400).await;
    location_select_dropdown.click().await?;
    random_sleep(500, 1000).await;

    let select_element_query = driver.query(By::Id("rms_batLocationSelect2"));
    let select_element = select_element_query.wait(timeout, polling).first().await?;
    select_element.wait_until().wait(timeout, polling).displayed().await?;
    let select_box = SelectElement::new(&select_element).await?;

    select_box.select_by_value(location).await?;

    random_sleep(2500, 4000).await;

    let next_button_loc = driver.query(By::Id("nextButton")).first().await?;
    next_button_loc.wait_until().wait(timeout, polling).displayed().await?;
    random_sleep(200, 500).await;
    next_button_loc.click().await?;

    random_sleep(1000, 2000).await;

    match driver.query(By::Id("getEarliestTime")).first().await {
        Ok(element) => {
            if element.is_clickable().await.unwrap_or(false) {
                random_sleep(200, 400).await;
                if let Err(e) = element.click().await {
                    eprintln!("WARN: Failed to click 'Get Earliest Time' for {}: {}", location, e);
                } else {
                    random_sleep(2500, 4500).await;
                }
            } else {
                random_sleep(500, 1000).await;
            }
        },
        Err(_) => {
            random_sleep(500, 1000).await;
        },
    }

    random_sleep(1000, 2500).await;

    let timeslots = driver.execute("return timeslots", vec![]).await?;

    let next_available_date = timeslots.json()
        .get("ajaxresult")
        .and_then(|ajax| ajax.get("slots"))
        .and_then(|slots| slots.get("nextAvailableDate"))
        .and_then(|date| date.as_str())
        .map(|s| s.to_string());

    let slots: Vec<TimeSlot> = timeslots.json()
        .get("ajaxresult")
        .and_then(|ajax| ajax.get("slots"))
        .and_then(|slots| slots.get("listTimeSlot"))
        .and_then(|list| serde_json::from_value(list.clone()).ok())
        .unwrap_or_else(Vec::new);

    let location_result = LocationBookings {
        location: location.to_string(),
        slots,
        next_available_date,
    };

    random_sleep(800, 1500).await;

    let another_location_link = driver.query(By::Id("anotherLocationLink")).first().await?;
    another_location_link.wait_until().wait(timeout, polling).displayed().await?;
    random_sleep(200, 500).await;
    another_location_link.click().await?;

    Ok(location_result)
}

/// Navigate back to the location selector page
async fn navigate_to_location_selector(driver: &WebDriver, settings: &Settings) -> WebDriverResult<()> {
    let timeout = Duration::from_millis(settings.selenium_element_timout);
    let polling = Duration::from_millis(settings.selenium_element_polling);
    
    // Try clicking "Another Location" link first (most common case)
    if let Ok(link) = driver.query(By::Id("anotherLocationLink")).first().await {
        if link.is_displayed().await.unwrap_or(false) {
            link.click().await?;
            random_sleep(1000, 2000).await;
            
            // Verify we're on the location selector page
            if driver.query(By::Id("rms_batLocLocSel")).first().await.is_ok() {
                return Ok(());
            }
        }
    }
    
    // Try navigating via the booking management page
    if let Ok(change_location) = driver.query(By::Id("changeLocationButton")).first().await {
        if change_location.is_displayed().await.unwrap_or(false) {
            change_location.click().await?;
            random_sleep(1000, 2000).await;
            return Ok(());
        }
    }
    
    // If all else fails, check if we're already on the location selector
    driver.query(By::Id("rms_batLocLocSel"))
        .wait(timeout, polling)
        .first()
        .await?;
    
    Ok(())
}

/// Attempt recovery after a failed location scrape - returns true if recovery succeeded
async fn attempt_recovery(driver: &WebDriver, settings: &Settings, location: &str) -> bool {
    log_page_snapshot(driver, &format!("location-{}", location)).await;

    // Try to navigate back to location selector
    match navigate_to_location_selector(driver, settings).await {
        Ok(_) => {
            println!("INFO: Recovery successful - back at location selector");
            true
        }
        Err(e) => {
            eprintln!("WARN: Recovery navigation failed: {}", e);
            false
        }
    }
}

/// Login with retry logic
async fn login_with_retry(driver: &WebDriver, settings: &Settings, worker_id: u8, max_retries: u8) -> Result<(), String> {
    for attempt in 1..=max_retries {
        match login_and_navigate_to_booking(driver, settings).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                let error_str = e.to_string();
                eprintln!("ERROR: [Worker {}] Login attempt {}/{} failed: {}", worker_id, attempt, max_retries, error_str);
                
                if attempt < max_retries {
                    // Check if it's a recoverable error (element not found, timeout, etc.)
                    if error_str.contains("no such element") || error_str.contains("timeout") || error_str.contains("stale") {
                        println!("INFO: [Worker {}] Retrying login in 3 seconds...", worker_id);
                        random_sleep(3000, 5000).await;
                        
                        // Navigate back to login page for retry
                        if let Err(nav_err) = driver.goto("https://www.myrta.com/wps/portal/extvp/myrta/login/").await {
                            eprintln!("WARN: [Worker {}] Failed to navigate to login page: {}", worker_id, nav_err);
                        }
                        random_sleep(1000, 2000).await;
                    } else {
                        return Err(error_str);
                    }
                } else {
                    return Err(error_str);
                }
            }
        }
    }
    Err("Max login retries exceeded".to_string())
}

/// Run a single worker that pulls work from a shared queue
async fn run_worker(
    worker_id: u8,
    work_queue: Arc<Mutex<Vec<String>>>,
    settings: Settings,
    result_tx: mpsc::Sender<(u8, String, Result<LocationBookings, String>)>,
) {
    println!("INFO: [Worker {}] Starting", worker_id);
    
    // Check if there's any work before starting browser
    {
        let queue = work_queue.lock().await;
        if queue.is_empty() {
            println!("INFO: [Worker {}] No work in queue, exiting without starting browser", worker_id);
            return;
        }
    }
    
    let driver = match create_driver(&settings).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ERROR: [Worker {}] Failed to create driver: {}", worker_id, e);
            return;
        }
    };

    // Login with retry
    if let Err(e) = login_with_retry(&driver, &settings, worker_id, 3).await {
        eprintln!("ERROR: [Worker {}] Failed to login after retries: {}", worker_id, e);
        let _ = driver.quit().await;
        return;
    }

    println!("INFO: [Worker {}] Login successful, ready to process locations", worker_id);

    let location_manager = LocationManager::new();
    let mut consecutive_failures = 0;
    const MAX_CONSECUTIVE_FAILURES: u8 = 3;

    loop {
        // Get next location from the shared queue
        let location = {
            let mut queue = work_queue.lock().await;
            queue.pop()
        };

        let location = match location {
            Some(loc) => loc,
            None => {
                println!("INFO: [Worker {}] No more work in queue. Finishing.", worker_id);
                break;
            }
        };

        let location_name = location.parse::<u32>()
            .ok()
            .and_then(|id| location_manager.get_by_id(id))
            .map(|loc| loc.name.clone())
            .unwrap_or_else(|| location.clone());
        
        println!("INFO: [Worker {}] Processing: {}", worker_id, location_name);

        match scrape_single_location(&driver, &location, &settings).await {
            Ok(booking_data) => {
                println!("INFO: [Worker {}] {} - {} slots found", 
                    worker_id, location_name, booking_data.slots.len());
                let _ = result_tx.send((worker_id, location.clone(), Ok(booking_data))).await;
                consecutive_failures = 0;
            }
            Err(e) => {
                let error_str = e.to_string();
                eprintln!("ERROR: [Worker {}] Failed processing {}: {}", worker_id, location_name, error_str);
                consecutive_failures += 1;

                // Check if this is a session/login error that requires re-login
                let needs_relogin = error_str.contains("checkTerms") 
                    || error_str.contains("no such element")
                    || error_str.contains("invalid session")
                    || error_str.contains("session deleted");

                if needs_relogin && consecutive_failures < MAX_CONSECUTIVE_FAILURES {
                    println!("INFO: [Worker {}] Session error detected, attempting re-login...", worker_id);
                    
                    // Re-add this location to the queue for retry
                    {
                        let mut queue = work_queue.lock().await;
                        queue.push(location.clone());
                    }
                    
                    // Navigate to login and re-authenticate
                    if let Err(nav_err) = driver.goto("https://www.myrta.com/wps/portal/extvp/myrta/login/").await {
                        eprintln!("ERROR: [Worker {}] Failed to navigate to login: {}", worker_id, nav_err);
                        let _ = result_tx.send((worker_id, location, Err(error_str))).await;
                        break;
                    }
                    
                    random_sleep(2000, 3000).await;
                    
                    if let Err(login_err) = login_with_retry(&driver, &settings, worker_id, 2).await {
                        eprintln!("ERROR: [Worker {}] Re-login failed: {}", worker_id, login_err);
                        let _ = result_tx.send((worker_id, location, Err(format!("Re-login failed: {}", login_err)))).await;
                        break;
                    }
                    
                    println!("INFO: [Worker {}] Re-login successful, continuing work", worker_id);
                    consecutive_failures = 0;
                } else if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    eprintln!("ERROR: [Worker {}] Too many consecutive failures, stopping worker", worker_id);
                    let _ = result_tx.send((worker_id, location, Err(error_str))).await;
                    break;
                } else {
                    // Try to recover to location selector
                    if attempt_recovery(&driver, &settings, &location).await {
                        consecutive_failures = 0;
                    }
                    let _ = result_tx.send((worker_id, location, Err(error_str))).await;
                }
            }
        }

        random_sleep(1500, 3000).await;
    }

    println!("INFO: [Worker {}] Finished. Quitting driver.", worker_id);
    let _ = driver.quit().await;
}

/// Parallel scraping function that spawns multiple browser workers with shared work queue
pub async fn scrape_rta_parallel<F>(
    locations: Vec<String>,
    settings: &Settings,
    file_path: &str,
    mut on_location_scraped: F,
) -> WebDriverResult<Vec<String>>
where
    F: FnMut(LocationBookings, &str),
{
    let num_workers = settings.parallel_workers.max(1);
    
    if num_workers == 1 {
        // Fall back to sequential scraping for single worker
        return scrape_rta_timeslots_incremental(locations, settings, file_path, on_location_scraped).await;
    }

    let total_locations = locations.len();
    
    // Create shared work queue (workers pop from the end for efficiency)
    let work_queue = Arc::new(Mutex::new(locations));
    
    println!("INFO: Starting parallel scrape with {} workers for {} locations (shared queue)", num_workers, total_locations);

    let (tx, mut rx) = mpsc::channel::<(u8, String, Result<LocationBookings, String>)>(total_locations * 2);
    
    // Spawn workers - all workers share the same queue
    let mut handles = Vec::new();
    for worker_id in 1..=num_workers {
        let settings_clone = settings.clone();
        let tx_clone = tx.clone();
        let queue_clone = Arc::clone(&work_queue);
        
        let handle = tokio::spawn(async move {
            run_worker(worker_id, queue_clone, settings_clone, tx_clone).await;
        });
        handles.push(handle);
    }

    // Drop the original sender so the channel closes when all workers are done
    drop(tx);

    // Collect results
    let mut successful_locations = Vec::new();
    let file_path_owned = file_path.to_string();
    
    while let Some((worker_id, location, result)) = rx.recv().await {
        match result {
            Ok(booking_data) => {
                on_location_scraped(booking_data, &file_path_owned);
                successful_locations.push(location);
            }
            Err(e) => {
                eprintln!("ERROR: [Worker {}] Location {} failed: {}", worker_id, location, e);
            }
        }
    }

    // Wait for all workers to complete
    for handle in handles {
        let _ = handle.await;
    }

    println!("INFO: Parallel scrape complete. {}/{} locations successful", 
        successful_locations.len(), total_locations);

    Ok(successful_locations)
}

/// Scrape with incremental callback - calls on_location_scraped after each successful location
pub async fn scrape_rta_timeslots_incremental<F>(
    locations: Vec<String>,
    settings: &Settings,
    file_path: &str,
    mut on_location_scraped: F,
) -> WebDriverResult<Vec<String>>
where
    F: FnMut(LocationBookings, &str),
{
    let mut successful_locations: Vec<String> = Vec::new();
    let total = locations.len();

    let mut caps = DesiredCapabilities::chrome();
    if settings.headless {
        caps.add_arg("--headless=new")?;
    }

    caps.add_arg("--no-sandbox")?;
    caps.add_arg("--disable-dev-shm-usage")?;
    caps.add_arg("--disable-gpu")?;
    caps.add_arg("--window-size=1920,1080")?;
    caps.add_arg("--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36");

    let driver = WebDriver::new(settings.selenium_driver_url.clone(), caps).await?;

    driver
        .execute(
            r#"
        Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
        window.chrome = window.chrome || {};
        window.chrome.runtime = window.chrome.runtime || {};
        try {
            let key = Object.keys(window).find(key => key.startsWith('cdc_'));
            if (key) { delete window[key]; }
            let docKey = Object.keys(document).find(key => key.startsWith('cdc_'));
            if (docKey) { delete document[docKey]; }
        } catch (e) { console.debug('Error removing cdc keys:', e); }
    "#,
            Vec::new(),
        )
        .await?;

    let timeout = Duration::from_millis(settings.selenium_element_timout);
    let polling = Duration::from_millis(settings.selenium_element_polling);

    driver
        .goto("https://www.myrta.com/wps/portal/extvp/myrta/login/")
        .await?;
    random_sleep(1000, 2000).await;

    let username_input = driver
        .query(By::Id("widget_cardNumber"))
        .wait(timeout, polling)
        .first()
        .await?;
    random_sleep(200, 500).await;
    type_like_human(&username_input, &settings.username, 60, 180).await?;
    random_sleep(300, 700).await;

    let password_input = driver
        .query(By::Id("widget_password"))
        .wait(timeout, polling)
        .first()
        .await?;

    random_sleep(200, 500).await;
    type_like_human(&password_input, &settings.password, 60, 180).await?;
    random_sleep(400, 800).await;

    let next_button = driver
        .query(By::Id("nextButton"))
        .wait(timeout, polling)
        .first()
        .await?;
    random_sleep(250, 600).await;
    next_button.click().await?;

    random_sleep(2000, 4000).await;
    
    // Debug: log page state after login attempt
    println!("DEBUG: Checking page state after login...");
    log_page_snapshot(&driver, "after-login").await;

    if settings.have_booking {
        let manage_booking = driver
            .query(By::XPath("//*[text()=\"Manage booking\"]"))
            .first()
            .await?;
        manage_booking
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        manage_booking.click().await?;
        random_sleep(1500, 2500).await;

        let change_location = driver.query(By::Id("changeLocationButton")).first().await?;
        change_location
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        change_location.click().await?;
        random_sleep(1000, 2000).await;
    } else {
        println!("DEBUG: Looking for 'Book test' button...");
        let book_test = driver
            .query(By::XPath("//*[text()='Book test']"))
            .wait(timeout, polling)
            .first()
            .await?;
        random_sleep(200, 500).await;
        book_test.click().await?;
        random_sleep(1500, 2500).await;

        let car_option = driver.query(By::Id("CAR")).first().await?;
        car_option
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        car_option.click().await?;
        random_sleep(500, 1000).await;

        let test_item = driver
            .query(By::XPath(
                "//fieldset[@id='DC']/span[contains(@class, 'rms_testItemResult')]",
            ))
            .first()
            .await?;
        test_item
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        test_item.click().await?;
        random_sleep(500, 1000).await;

        let next_button = driver.query(By::Id("nextButton")).first().await?;
        next_button
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        next_button.click().await?;
        random_sleep(1500, 2500).await;

        let check_terms = driver.query(By::Id("checkTerms")).first().await?;
        check_terms
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(100, 300).await;
        check_terms.click().await?;
        random_sleep(500, 1000).await;

        let next_button_terms = driver.query(By::Id("nextButton")).first().await?;
        next_button_terms
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        next_button_terms.click().await?;
        random_sleep(1000, 2000).await;
    }

    let location_manager = LocationManager::new();
    
    for (idx, location) in locations.iter().enumerate() {
        let location_name = location.parse::<u32>()
            .ok()
            .and_then(|id| location_manager.get_by_id(id))
            .map(|loc| loc.name.clone())
            .unwrap_or_else(|| location.clone());
        println!("INFO: [{}/{}] Processing location: {}", idx + 1, total, location_name);
        
        let process_result: WebDriverResult<LocationBookings> = async {
            random_sleep(1000, 2000).await;

            let location_select_dropdown = driver.query(By::Id("rms_batLocLocSel")).first().await?;
            location_select_dropdown.wait_until().wait(timeout, polling).displayed().await?;
            random_sleep(200, 400).await;
            location_select_dropdown.click().await?;
            random_sleep(500, 1000).await;

            let select_element_query = driver.query(By::Id("rms_batLocationSelect2"));
            let select_element = select_element_query.wait(timeout, polling).first().await?;
            select_element.wait_until().wait(timeout, polling).displayed().await?;
            let select_box = SelectElement::new(&select_element).await?;

            if let Err(e) = select_box.select_by_value(&location).await {
                 eprintln!("ERROR: Failed to select location '{}' in dropdown: {}.", location, e);
                 return Err(e);
            }

            random_sleep(2500, 4000).await;

            let next_button_loc = driver.query(By::Id("nextButton")).first().await?;
            next_button_loc.wait_until().wait(timeout, polling).displayed().await?;
            random_sleep(200, 500).await;
            next_button_loc.click().await?;

            random_sleep(1000, 2000).await;

            match driver.query(By::Id("getEarliestTime")).first().await {
                Ok(element) => {
                     if element.is_clickable().await.unwrap_or(false) {
                         random_sleep(200, 400).await;
                         if let Err(e) = element.click().await {
                            eprintln!("WARN: Failed to click 'Get Earliest Time' for {}: {}", location, e);
                         } else {
                             random_sleep(2500, 4500).await;
                         }
                     } else {
                         random_sleep(500, 1000).await;
                     }
                },
                Err(_) => {
                    random_sleep(500, 1000).await;
                },
            }

            random_sleep(1000, 2500).await;

            let timeslots = driver.execute("return timeslots", vec![]).await?;

            let next_available_date = timeslots.json()
                .get("ajaxresult")
                .and_then(|ajax| ajax.get("slots"))
                .and_then(|slots| slots.get("nextAvailableDate"))
                .and_then(|date| date.as_str())
                .map(|s| s.to_string());

            let slots: Vec<TimeSlot> = timeslots.json()
                .get("ajaxresult")
                .and_then(|ajax| ajax.get("slots"))
                .and_then(|slots| slots.get("listTimeSlot"))
                .and_then(|list| serde_json::from_value(list.clone()).ok())
                .unwrap_or_else(Vec::new);

            println!("INFO: [{}/{}] {} - {} slots found. Next available: {:?}", 
                idx + 1, total, location, slots.len(), next_available_date);

            let location_result = LocationBookings {
                location: location.to_string(),
                slots,
                next_available_date,
            };

            random_sleep(800, 1500).await;

            let another_location_link = driver.query(By::Id("anotherLocationLink")).first().await?;
            another_location_link.wait_until().wait(timeout, polling).displayed().await?;
            random_sleep(200, 500).await;
            another_location_link.click().await?;

            Ok(location_result)

        }.await;

        match process_result {
            Ok(booking_data) => {
                // Call callback to save incrementally
                on_location_scraped(booking_data, file_path);
                successful_locations.push(location.clone());
            }
            Err(e) => {
                eprintln!("ERROR: Failed processing location {}: {}", location, e);
                log_page_snapshot(&driver, &format!("location-{}", location)).await;

                match driver.query(By::Id("anotherLocationLink")).first().await {
                    Ok(link) => {
                        if link.is_displayed().await.unwrap_or(false) {
                            eprintln!("INFO: Attempting recovery click on 'Another Location'.");
                            if let Err(click_err) = link.click().await {
                                eprintln!("WARN: Recovery click failed: {}", click_err);
                            }
                        }
                    }
                    Err(_) => {
                        eprintln!("WARN: Recovery link not found.");
                    }
                }
                random_sleep(2000, 3000).await;
                continue;
            }
        }
        random_sleep(1500, 3000).await;
    }

    println!("INFO: Finished scraping. Quitting driver.");
    driver.quit().await?;

    Ok(successful_locations)
}

pub async fn scrape_rta_timeslots(
    locations: Vec<String>,
    settings: &Settings,
) -> WebDriverResult<HashMap<String, LocationBookings>> {
    let mut location_bookings: HashMap<String, LocationBookings> = HashMap::new();

    let mut caps = DesiredCapabilities::chrome();
    if settings.headless {
        caps.add_arg("--headless=new")?;
    }

    caps.add_arg("--no-sandbox")?;
    caps.add_arg("--disable-dev-shm-usage")?;
    caps.add_arg("--disable-gpu")?;
    caps.add_arg("--window-size=1920,1080")?;
    caps.add_arg("--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36");

    let driver = WebDriver::new(settings.selenium_driver_url.clone(), caps).await?;

    driver
        .execute(
            r#"
        Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
        // Minimal spoofing of window.chrome, might need adjustment
        window.chrome = window.chrome || {};
        window.chrome.runtime = window.chrome.runtime || {};
        // Attempt to remove cdc_ properties (might not exist)
        try {
            let key = Object.keys(window).find(key => key.startsWith('cdc_'));
            if (key) { delete window[key]; }
            let docKey = Object.keys(document).find(key => key.startsWith('cdc_'));
            if (docKey) { delete document[docKey]; }
        } catch (e) { console.debug('Error removing cdc keys:', e); }
    "#,
            Vec::new(),
        )
        .await?;

    let timeout = Duration::from_millis(settings.selenium_element_timout);
    let polling = Duration::from_millis(settings.selenium_element_polling);

    driver
        .goto("https://www.myrta.com/wps/portal/extvp/myrta/login/")
        .await?;
    random_sleep(1000, 2000).await;

    let username_input = driver
        .query(By::Id("widget_cardNumber"))
        .wait(timeout, polling)
        .first()
        .await?;
    random_sleep(200, 500).await;
    type_like_human(&username_input, &settings.username, 60, 180).await?;
    random_sleep(300, 700).await;

    let password_input = driver
        .query(By::Id("widget_password"))
        .wait(timeout, polling)
        .first()
        .await?;

    random_sleep(200, 500).await;
    type_like_human(&password_input, &settings.password, 60, 180).await?;
    random_sleep(400, 800).await;

    let next_button = driver
        .query(By::Id("nextButton"))
        .wait(timeout, polling)
        .first()
        .await?;
    // next_button.wait_until().wait(timeout, polling).has_attribute("aria-disabled", "false").await?; // Alternative if clickable() doesn't work
    random_sleep(250, 600).await;
    next_button.click().await?;

    random_sleep(2000, 4000).await;

    if settings.have_booking {
        let manage_booking = driver
            .query(By::XPath("//*[text()=\"Manage booking\"]"))
            .first()
            .await?;
        manage_booking
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        manage_booking.click().await?;
        random_sleep(1500, 2500).await;

        let change_location = driver.query(By::Id("changeLocationButton")).first().await?;
        change_location
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        change_location.click().await?;
        random_sleep(1000, 2000).await;
    } else {
        let book_test = driver
            .query(By::XPath("//*[text()='Book test']"))
            .wait(timeout, polling)
            .first()
            .await?;
        random_sleep(200, 500).await;
        book_test.click().await?;
        random_sleep(1500, 2500).await;

        let car_option = driver.query(By::Id("CAR")).first().await?;
        car_option
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        car_option.click().await?;
        random_sleep(500, 1000).await;

        let test_item = driver
            .query(By::XPath(
                "//fieldset[@id='DC']/span[contains(@class, 'rms_testItemResult')]",
            ))
            .first()
            .await?;
        test_item
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        test_item.click().await?;
        random_sleep(500, 1000).await;

        let next_button = driver.query(By::Id("nextButton")).first().await?;
        next_button
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        next_button.click().await?;
        random_sleep(1500, 2500).await;

        let check_terms = driver.query(By::Id("checkTerms")).first().await?;
        check_terms
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(100, 300).await;
        check_terms.click().await?;
        random_sleep(500, 1000).await;

        let next_button_terms = driver.query(By::Id("nextButton")).first().await?;
        next_button_terms
            .wait_until()
            .wait(timeout, polling)
            .displayed()
            .await?;
        random_sleep(200, 500).await;
        next_button_terms.click().await?;
        random_sleep(1000, 2000).await;
    }

    for location in locations {
        // println!("INFO: Processing location: {}", location);
        let process_result: WebDriverResult<LocationBookings> = async {

            random_sleep(1000, 2000).await;

            let location_select_dropdown = driver.query(By::Id("rms_batLocLocSel")).first().await?;
            location_select_dropdown.wait_until().wait(timeout, polling).displayed().await?;
            random_sleep(200, 400).await;
            location_select_dropdown.click().await?;
            random_sleep(500, 1000).await;

            let select_element_query = driver.query(By::Id("rms_batLocationSelect2"));
            let select_element = select_element_query.wait(timeout, polling).first().await?;
            select_element.wait_until().wait(timeout, polling).displayed().await?;
            let select_box = SelectElement::new(&select_element).await?;

            if let Err(e) = select_box.select_by_value(&location).await {
                 eprintln!("ERROR: Failed to select location '{}' in dropdown: {}. Ensure the value is correct.", location, e);
                 return Err(e);
            }

            // println!("INFO: Selected location: {}", location);
            random_sleep(2500, 4000).await;

            let next_button_loc = driver.query(By::Id("nextButton")).first().await?;
            next_button_loc.wait_until().wait(timeout, polling).displayed().await?;
            random_sleep(200, 500).await;
            next_button_loc.click().await?;

            random_sleep(1000, 2000).await;

            match driver.query(By::Id("getEarliestTime")).first().await {
                Ok(element) => {
                     if element.is_clickable().await.unwrap_or(false) {
                         random_sleep(200, 400).await;
                         if let Err(e) = element.click().await {
                            eprintln!("WARN: Failed to click 'Get Earliest Time' button for {}: {}. Proceeding anyway.", location, e);
                         } else {
                             random_sleep(2500, 4500).await;
                         }
                     } else {
                         println!("INFO: 'Get Earliest Time' button found but not clickable (visible/enabled).");
                         random_sleep(500, 1000).await;
                     }
                },
                Err(_) => {
                    println!("INFO: 'Get Earliest Time' button not found for {}. Proceeding.", location);
                    random_sleep(500, 1000).await;
                },
            }

            random_sleep(1000, 2500).await;

            let timeslots = driver.execute("return timeslots", vec![]).await?;

            let next_available_date = timeslots.json()
                .get("ajaxresult")
                .and_then(|ajax| ajax.get("slots"))
                .and_then(|slots| slots.get("nextAvailableDate"))
                .and_then(|date| date.as_str())
                .map(|s| s.to_string());

            let slots: Vec<TimeSlot> = timeslots.json()
                .get("ajaxresult")
                .and_then(|ajax| ajax.get("slots"))
                .and_then(|slots| slots.get("listTimeSlot"))
                .and_then(|list| serde_json::from_value(list.clone()).ok())
                .unwrap_or_else(Vec::new);


            println!("INFO: Parsed {} slots for {}. Next available: {:?}", slots.len(), location, next_available_date);

            let location_result = LocationBookings {
                location: location.to_string(),
                slots,
                next_available_date,
            };

            random_sleep(800, 1500).await;

            let another_location_link = driver.query(By::Id("anotherLocationLink")).first().await?;
            another_location_link.wait_until().wait(timeout, polling).displayed().await?;
            random_sleep(200, 500).await;
            another_location_link.click().await?;

            Ok(location_result)

        }.await;

        match process_result {
            Ok(booking_data) => {
                location_bookings.insert(location.clone(), booking_data);
            }
            Err(e) => {
                eprintln!("ERROR: Failed processing location {}: {}", location, e);
                log_page_snapshot(&driver, &format!("location-{}", location)).await;

                match driver.query(By::Id("anotherLocationLink")).first().await {
                    Ok(link) => {
                        if link.is_displayed().await.unwrap_or(false) {
                            eprintln!("INFO: Attempting recovery click on 'Another Location'.");
                            if let Err(click_err) = link.click().await {
                                eprintln!("WARN: Recovery click failed: {}", click_err);
                            } else {
                                println!("INFO: Recovery click succeeded.");
                            }
                        } else {
                            eprintln!("WARN: Recovery link found but not displayed.");
                        }
                    }
                    Err(_) => {
                        eprintln!(
                            "WARN: Recovery link ('anotherLocationLink') not found. State unclear."
                        );
                    }
                }
                random_sleep(2000, 3000).await;
                continue;
            }
        }
        random_sleep(1500, 3000).await;
    }

    println!("INFO: Finished scraping all locations. Quitting driver.");
    driver.quit().await?;

    Ok(location_bookings)
}
