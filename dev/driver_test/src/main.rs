use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use thirtyfour::components::SelectElement;
use thirtyfour::{By, DesiredCapabilities, WebDriver};
use thirtyfour::prelude::*;

use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Deserialize, Clone)]
pub struct Settings {
    pub headless: bool,
    pub username: String,
    pub password: String,
    pub have_booking: bool,
    pub selenium_driver_url: String,
    pub selenium_element_timout: u64,
    pub selenium_element_polling: u64,
}

impl Settings {
    pub fn from_yaml<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        dotenv::from_path("../../.env").ok();
        
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        
        let mut settings: Settings = serde_yaml::from_str(&contents)?;
        
        settings.username = parse_env_var(&settings.username)?;
        settings.password = parse_env_var(&settings.password)?;
        
        Ok(settings)
    }
}

fn parse_env_var(value: &str) -> Result<String, Box<dyn std::error::Error>> {
    if value.starts_with("${") && value.ends_with("}") {
        let env_name = &value[2..value.len() - 1];
        match env::var(env_name) {
            Ok(val) => Ok(val),
            Err(_) => Err(format!("Environment variable '{}' not found", env_name).into()),
        }
    } else {
        Ok(value.to_string())
    }
}

use std::{cmp::Ordering, hash::{DefaultHasher, Hash, Hasher}};

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub struct TimeSlot {
    pub availability: bool,
    pub slot_number: Option<u32>,
    #[serde(rename = "startTime")]
    pub start_time: String,
}

impl PartialEq for TimeSlot {
    fn eq(&self, other: &Self) -> bool {
        self.start_time == other.start_time
    }
}

impl Eq for TimeSlot {}

impl PartialOrd for TimeSlot {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimeSlot {
    fn cmp(&self, other: &Self) -> Ordering {
        self.start_time.cmp(&other.start_time)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub struct LocationBookings {
    pub location: String,
    pub slots: Vec<TimeSlot>,
    pub next_available_date: Option<String>,
}

impl LocationBookings {
    pub fn calculate_hash(&self) -> String {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish().to_string()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Hash)]
pub struct BookingData {
    pub results: Vec<LocationBookings>,
    pub last_updated: Option<String>,
}

impl BookingData {
    pub fn calculate_hash(&self) -> String {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish().to_string()
    }
}

pub async fn scrape_rta_timeslots<'a>(
    locations: Vec<&'a str>,
    settings: &Settings
) -> WebDriverResult<HashMap<&'a str, LocationBookings>> {

    let mut location_bookings: HashMap<&'a str, LocationBookings> = HashMap::new();

    let mut caps = DesiredCapabilities::chrome();
    if settings.headless {
        caps.add_arg("--headless")?;
    }
    caps.add_arg("--no-sandbox")?;
    caps.add_arg("--disable-dev-shm-usage")?;
    caps.add_arg("user-agent=Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/103.0.5060.114 Safari/537.36")?;
    caps.add_arg("--disable-blink-features=AutomationControlled")?;

    let driver = WebDriver::new(settings.selenium_driver_url.clone(), caps).await?;

    let timeout = Duration::from_millis(settings.selenium_element_timout);
    let polling = Duration::from_millis(settings.selenium_element_polling);

    driver.execute("Object.defineProperty(navigator, 'webdriver', {get: () => undefined})", vec![]).await?;
    driver.goto("https://www.myrta.com/wps/portal/extvp/myrta/login/").await?;

    let username_input = driver.query(By::Id("widget_cardNumber")).first().await?;
    username_input.wait_until().wait(timeout, polling).displayed().await?;
    username_input.send_keys(&settings.username).await?;

    let password_input = driver.query(By::Id("widget_password")).first().await?;
    password_input.wait_until().wait(timeout, polling).displayed().await?;
    password_input.send_keys(&settings.password).await?;

    let next_button = driver.query(By::Id("nextButton")).first().await?;
    next_button.wait_until().wait(timeout, polling).has_attribute("aria-disabled", "false").await?;
    next_button.click().await?;

    if settings.have_booking {
        let manage_booking = driver.query(By::XPath("//*[text()=\"Manage booking\"]")).first().await?;
        manage_booking.wait_until().wait(timeout, polling).displayed().await?;
        manage_booking.click().await?;

        let change_location = driver.query(By::Id("changeLocationButton")).first().await?;
        change_location.wait_until().wait(timeout, polling).displayed().await?;
        change_location.click().await?;
    } else {
        let book_test = driver.query(By::XPath("//*[text()=\"Book test\"]")).first().await?;
        book_test.wait_until().wait(timeout, polling).displayed().await?;
        book_test.click().await?;

        let car_option = driver.query(By::Id("CAR")).first().await?;
        car_option.wait_until().wait(timeout, polling).displayed().await?;
        car_option.click().await?;

        let test_item = driver.query(By::XPath("//fieldset[@id='DC']/span[contains(@class, 'rms_testItemResult')]")).first().await?;
        test_item.wait_until().wait(timeout, polling).displayed().await?;
        test_item.click().await?;

        let next_button = driver.query(By::Id("nextButton")).first().await?;
        next_button.wait_until().wait(timeout, polling).displayed().await?;
        next_button.click().await?;

        let check_terms = driver.query(By::Id("checkTerms")).first().await?;
        check_terms.wait_until().wait(timeout, polling).displayed().await?;
        check_terms.click().await?;

        let next_button = driver.query(By::Id("nextButton")).first().await?;
        next_button.wait_until().wait(timeout, polling).displayed().await?;
        next_button.click().await?;

    }

    for location in locations.iter() {

        let process_result: WebDriverResult<LocationBookings> = async {

            tokio::time::sleep(Duration::from_secs(1)).await;

            let location_select_dropdown = driver.query(By::Id("rms_batLocLocSel")).first().await?;
            location_select_dropdown.wait_until().wait(timeout, polling).displayed().await?;
            location_select_dropdown.click().await?;

            let select_element = driver.query(By::Id("rms_batLocationSelect2")).first().await?;
            select_element.wait_until().wait(timeout, polling).displayed().await?;
            let select_box = SelectElement::new(&select_element).await?;

            // FIX: might not need htis
            let location_value = format!("{} Service NSW Centre", location);

            if let Err(e) = select_box.select_by_value(&location_value).await {
                 eprintln!("ERROR: Failed to select location '{}' in dropdown: {}. Ensure the value is correct.", location, e);
                 return Err(e);
            }

            println!("INFO: Selected location: {}", location);
            tokio::time::sleep(Duration::from_secs(3)).await;

            let next_button = driver.query(By::Id("nextButton")).first().await?;
            next_button.wait_until().wait(timeout, polling).displayed().await?;
            next_button.click().await?;

            tokio::time::sleep(Duration::from_secs(1)).await;

            match driver.query(By::Id("getEarliestTime")).first().await {
                Ok(element) => {
                    let is_displayed = element.is_displayed().await.unwrap_or(false);
                    let is_enabled = element.is_enabled().await.unwrap_or(false);
                    if is_displayed && is_enabled {
                         println!("INFO: Found 'Get Earliest Time' button, attempting click.");
                        if let Err(e) = element.click().await {
                            eprintln!("WARN: Failed to click 'Get Earliest Time' button for {}: {}. Proceeding anyway.", location, e);
                        } else {
                             println!("INFO: Clicked 'Get Earliest Time'.");
                        }
                    } else {
                        println!("INFO: 'Get Earliest Time' button found but not displayed/enabled.");
                    }
                },
                Err(_) => {
                    println!("INFO: 'Get Earliest Time' button not found for {}.", location);
                },
            }

            tokio::time::sleep(Duration::from_secs(3)).await;

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

            tokio::time::sleep(Duration::from_secs(1)).await;

            let another_location_link = driver.query(By::Id("anotherLocationLink")).first().await?;
            another_location_link.wait_until().wait(timeout, polling).displayed().await?;
            another_location_link.click().await?;

            Ok(location_result)


        }.await;

        match process_result {
            Ok(booking_data) => {
                location_bookings.insert(*location, booking_data);
            }
            Err(e) => {
                 match driver.query(By::Id("anotherLocationLink")).first().await {
                     Ok(link) => {
                         if let Err(click_err) = link.click().await {
                             eprintln!("WARN: Recovery click failed: {}", click_err);
                         } else {
                             println!("INFO: Recovery click succeeded.");
                         }
                     }
                     Err(_) => {
                         eprintln!("WARN: Recovery link not found.");
                     }
                 }
                 tokio::time::sleep(Duration::from_secs(2)).await;

                continue;
            }
        }
         tokio::time::sleep(Duration::from_secs(2)).await;

    }

    driver.quit().await?;

    Ok(location_bookings)
}


#[tokio::main]
async fn main() {
    let mut settings = Settings::from_yaml("../../settings.yaml").unwrap();
    settings.headless = false;
    let env_content = include_str!("../../../.env");

    let (username, password) = env_content
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim(), value.trim()))
        .fold((None, None), |(mut u, mut p), (k, v)| {
            if k == "USERNAME" { u = Some(v.to_string()); }
            if k == "PASSWORD" { p = Some(v.to_string()); }
            (u, p)
        });

    settings.username = username.unwrap();
    settings.password = password.unwrap();
    
    let locations = vec!["Finley", "Hornsby", "Armidale", "Auburn", "Ballina"];
    
    dbg!(scrape_rta_timeslots(locations, &settings).await);

}
