use dotenv::dotenv;
use serde::Deserialize;
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
    pub retries: u64,
    pub scrape_refresh_time_min: u64,
    #[serde(default)]
    pub locations: Vec<String>,
    #[serde(default)]
    pub date_filter_start: Option<String>,
    #[serde(default)]
    pub date_filter_end: Option<String>,
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
        
        // Debug: show credential lengths (not values)
        println!("INFO: Credentials loaded - username: {} chars, password: {} chars", 
            settings.username.len(), settings.password.len());

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
