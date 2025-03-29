use std::fs::OpenOptions;
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thirtyfour::components::SelectElement;
use thirtyfour::{By, DesiredCapabilities, WebDriver};
use thirtyfour::prelude::*;

use crate::settings::Settings;

use super::shared_booking::{LocationBookings, TimeSlot};

pub async fn scrape_rta_timeslots(
    location: &str, 
    settings: &Settings
) -> WebDriverResult<LocationBookings> {

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
    
    driver.execute(
        "Object.defineProperty(navigator, 'webdriver', {get: () => undefined})",
        vec![]
    ).await?;
    
    let result = async {

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
            
            let location_select = driver.query(By::Id("rms_batLocLocSel")).first().await?;
            location_select.wait_until().wait(timeout, polling).displayed().await?;
            location_select.click().await?;
            
        }
        
        let location_select = driver.query(By::Id("rms_batLocLocSel")).first().await?;
        location_select.wait_until().wait(timeout, polling).displayed().await?;
        location_select.click().await?;
        
        let select_element = driver.query(By::Id("rms_batLocationSelect2")).first().await?;
        select_element.wait_until().wait(timeout, polling).displayed().await?;
        let select_box = SelectElement::new(&select_element).await?;
        select_box.select_by_value(location).await?;
        
        let next_button = driver.query(By::Id("nextButton")).first().await?;
        next_button.wait_until().wait(timeout, polling).displayed().await?;
        next_button.click().await?;
        
        match driver.query(By::Id("getEarliestTime")).first().await {
            Ok(element) => {
                if element.is_displayed().await? && element.is_enabled().await? {
                    element.click().await?;
                }
            },
            Err(_) => {},
        }
        
        let timeslots = driver.execute_script("return timeslots", vec![]).await?;
        
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
        Ok(location_result)
    }.await;
    
    driver.quit().await?;
    
    result
}
