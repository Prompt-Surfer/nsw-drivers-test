use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertConfig {
    pub location_id: String,
    pub location_name: String,
    pub enabled: bool,
    #[serde(default)]
    pub months: Vec<u32>, // 1-12 for Jan-Dec
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Settings {
    pub headless: bool,
    pub username: String,
    pub password: String,
    pub have_booking: bool,
    pub selenium_driver_url: String,
    pub selenium_element_timout: u64,
    pub selenium_element_polling: u64,
    pub retries: u64,
    pub scrape_refresh_time_min: u64,
    #[serde(default)]
    pub locations: Vec<String>,
    #[serde(default)]
    pub date_filter_start: Option<String>,
    #[serde(default)]
    pub date_filter_end: Option<String>,
    #[serde(default)]
    pub alerts: Vec<AlertConfig>,
    #[serde(default)]
    pub alerts_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            headless: false,
            username: String::new(),
            password: String::new(),
            have_booking: false,
            selenium_driver_url: String::new(),
            selenium_element_timout: 20000,
            selenium_element_polling: 100,
            retries: 4,
            scrape_refresh_time_min: 120,
            locations: vec![],
            date_filter_start: None,
            date_filter_end: None,
            alerts: vec![],
            alerts_enabled: false,
        }
    }
}

impl Settings {
    pub fn from_yaml<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        // Load .env from current directory
        match dotenv() {
            Ok(path) => println!("INFO: Loaded .env from {:?}", path),
            Err(e) => eprintln!("WARN: Could not load .env file: {}", e),
        }

        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let mut settings: Settings = serde_yaml::from_str(&contents)?;

        settings.username = parse_env_var(&settings.username)?;
        settings.password = parse_env_var(&settings.password)?;

        Ok(settings)
    }

    pub fn save_to_yaml<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        // Create a copy with placeholders for sensitive data
        let mut settings_to_save = self.clone();
        settings_to_save.username = "${MYRTA_USERNAME}".to_string();
        settings_to_save.password = "${MYRTA_PASSWORD}".to_string();

        let yaml = serde_yaml::to_string(&settings_to_save)?;
        let mut file = File::create(path)?;
        file.write_all(yaml.as_bytes())?;
        Ok(())
    }

    /// Get locations to scrape based on alert configuration
    /// If alerts are enabled, only scrape locations that have alerts enabled
    pub fn get_scrape_locations(&self) -> Vec<String> {
        if self.alerts_enabled && !self.alerts.is_empty() {
            self.alerts
                .iter()
                .filter(|a| a.enabled)
                .map(|a| a.location_id.clone())
                .collect()
        } else {
            self.locations.clone()
        }
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
