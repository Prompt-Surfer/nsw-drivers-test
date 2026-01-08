use std::collections::HashMap;
use std::time::Duration;

use leptos::prelude::*;
use leptos::server_fn::error::NoCustomError;
use reqwest::header;
use serde::{Deserialize, Serialize};
use web_sys::wasm_bindgen::prelude::Closure;

use crate::data::location::LocationManager;
use crate::data::shared_booking::TimeSlot;
use crate::pages::location_table::LocationsTable;
use crate::settings::AlertConfig;
use crate::utils::date::TimeDisplay;
use crate::utils::geocoding::geocode_address;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScrapingStatusResponse {
    pub is_running: bool,
    pub current_location: Option<String>,
    pub completed_count: usize,
    pub total_count: usize,
    pub estimated_remaining_secs: Option<u64>,
    pub error_message: Option<String>,
    pub last_completed_at: Option<String>,
    pub last_duration_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationBookingViewModel {
    pub location: String,
    pub earliest_slot: Option<TimeSlot>,
    pub latest_slot: Option<TimeSlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingResponse {
    pub bookings: Vec<LocationBookingViewModel>,
    pub last_updated: Option<String>,
    pub etag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationDetailBookingResponse {
    pub location: String,
    pub slots: Vec<TimeSlot>,
    pub etag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlertSettingsResponse {
    pub alerts_enabled: bool,
    pub alerts: Vec<AlertConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotAlertResponse {
    pub location_id: String,
    pub location_name: String,
    pub slot_time: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationOption {
    pub id: String,
    pub name: String,
}

#[server(GetBookings)]
pub async fn get_location_bookings(
    client_etag: String,
) -> Result<Option<BookingResponse>, ServerFnError> {
    use crate::data::booking::BookingManager;
    use axum::http::HeaderValue;
    use axum::http::StatusCode;

    let response = expect_context::<leptos_axum::ResponseOptions>();

    let (booking_data, server_etag) = BookingManager::get_data();
    if client_etag == server_etag {
        return Ok(None);
    }

    let view_models: Vec<_> = booking_data
        .results
        .iter()
        .map(|location_booking| {
            let available_slots: Vec<_> = location_booking
                .slots
                .iter()
                .filter(|slot| slot.availability)
                .collect();
            
            let earliest_slot = available_slots
                .iter()
                .min_by(|a, b| a.cmp(b))
                .cloned()
                .cloned();
            
            let latest_slot = available_slots
                .iter()
                .max_by(|a, b| a.cmp(b))
                .cloned()
                .cloned();

            LocationBookingViewModel {
                location: location_booking.location.clone(),
                earliest_slot,
                latest_slot,
            }
        })
        .collect();

    Ok(Some(BookingResponse {
        bookings: view_models,
        last_updated: booking_data.last_updated.clone(),
        etag: server_etag,
    }))
}

#[server(GetLocationDetails)]
pub async fn get_location_details(
    location_id: String,
    client_etag: String,
) -> Result<Option<LocationDetailBookingResponse>, ServerFnError> {
    use crate::data::booking::BookingManager;

    let (location_booking, server_etag) = BookingManager::get_location_data(location_id).ok_or(
        ServerFnError::<NoCustomError>::ServerError("Location not found".into()),
    )?;

    if client_etag == server_etag {
        return Ok(None);
    }

    Ok(Some(LocationDetailBookingResponse {
        location: location_booking.location,
        slots: location_booking.slots,
        etag: server_etag,
    }))
}

#[server(StartScraping)]
pub async fn start_scraping() -> Result<String, ServerFnError> {
    use crate::data::booking::BookingManager;
    use crate::settings::Settings;
    use std::fs::File;
    use std::io::Read;
    
    // Load settings
    let settings = Settings::from_yaml("settings.yaml")
        .map_err(|e| ServerFnError::<NoCustomError>::ServerError(format!("Failed to load settings: {}", e)))?;
    
    // Use get_scrape_locations which respects alert settings
    let locations = if settings.get_scrape_locations().is_empty() {
        // Load all locations from centres.json
        let mut file = File::open("data/centres.json")
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(format!("Failed to open centres.json: {}", e)))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(format!("Failed to read centres.json: {}", e)))?;
        let locs: Vec<crate::data::location::Location> = serde_json::from_str(&contents)
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(format!("Failed to parse centres.json: {}", e)))?;
        locs.into_iter().map(|l| l.id.to_string()).collect()
    } else {
        settings.get_scrape_locations()
    };
    
    let count = locations.len();
    
    // Trigger scraping
    BookingManager::trigger_single_scrape(
        locations,
        "data/bookings.json".to_string(),
        settings,
    ).map_err(|e| ServerFnError::<NoCustomError>::ServerError(e))?;
    
    Ok(format!("Scraping started for {} locations", count))
}

#[server(GetScrapingStatus)]
pub async fn get_scraping_status() -> Result<ScrapingStatusResponse, ServerFnError> {
    use crate::data::booking::BookingManager;
    
    let status = BookingManager::get_scraping_status();
    
    Ok(ScrapingStatusResponse {
        is_running: status.is_running,
        current_location: status.current_location,
        completed_count: status.completed_locations.len(),
        total_count: status.total_locations,
        estimated_remaining_secs: status.estimated_remaining_secs,
        error_message: status.error_message,
        last_completed_at: status.last_completed_at,
        last_duration_secs: status.last_duration_secs,
    })
}

#[server(GetAlertSettings)]
pub async fn get_alert_settings() -> Result<AlertSettingsResponse, ServerFnError> {
    use crate::settings::Settings;
    
    let settings = Settings::from_yaml("settings.yaml")
        .map_err(|e| ServerFnError::<NoCustomError>::ServerError(format!("Failed to load settings: {}", e)))?;
    
    Ok(AlertSettingsResponse {
        alerts_enabled: settings.alerts_enabled,
        alerts: settings.alerts,
    })
}

#[server(SaveAlertSettings)]
pub async fn save_alert_settings(
    alerts_enabled: bool,
    alerts: Vec<AlertConfig>,
) -> Result<(), ServerFnError> {
    use crate::settings::Settings;
    use crate::notifications::NotificationManager;
    
    let mut settings = Settings::from_yaml("settings.yaml")
        .map_err(|e| ServerFnError::<NoCustomError>::ServerError(format!("Failed to load settings: {}", e)))?;
    
    settings.alerts_enabled = alerts_enabled;
    settings.alerts = alerts;
    
    // Update locations to match enabled alerts when alerts are enabled
    if alerts_enabled {
        settings.locations = settings.alerts
            .iter()
            .filter(|a| a.enabled)
            .map(|a| a.location_id.clone())
            .collect();
    }
    
    settings.save_to_yaml("settings.yaml")
        .map_err(|e| ServerFnError::<NoCustomError>::ServerError(format!("Failed to save settings: {}", e)))?;
    
    // Clear notification history when settings change
    NotificationManager::clear_notification_history();
    
    Ok(())
}

#[server(GetActiveAlerts)]
pub async fn get_active_alerts() -> Result<Vec<SlotAlertResponse>, ServerFnError> {
    use crate::notifications::NotificationManager;
    
    let alerts = NotificationManager::get_active_alerts();
    
    Ok(alerts.into_iter().map(|a| SlotAlertResponse {
        location_id: a.location_id,
        location_name: a.location_name,
        slot_time: a.slot_time,
        created_at: a.created_at,
    }).collect())
}

#[server(DismissAlert)]
pub async fn dismiss_alert(location_id: String, slot_time: String) -> Result<(), ServerFnError> {
    use crate::notifications::NotificationManager;
    
    NotificationManager::dismiss_alert(&location_id, &slot_time);
    Ok(())
}

#[server(DismissAllAlerts)]
pub async fn dismiss_all_alerts() -> Result<(), ServerFnError> {
    use crate::notifications::NotificationManager;
    
    NotificationManager::dismiss_all_alerts();
    Ok(())
}

#[server(GetAllLocations)]
pub async fn get_all_locations() -> Result<Vec<LocationOption>, ServerFnError> {
    use crate::data::location::LocationManager;
    
    let manager = LocationManager::new();
    let locations = manager.get_all();
    
    Ok(locations.into_iter().map(|l| LocationOption {
        id: l.id.to_string(),
        name: l.name,
    }).collect())
}

#[component]
pub fn AlertBanner(
    alerts: ReadSignal<Vec<SlotAlertResponse>>,
    on_dismiss: impl Fn(String, String) + 'static + Copy + Send + Sync,
    on_dismiss_all: impl Fn() + 'static + Copy + Send + Sync,
) -> impl IntoView {
    view! {
        {move || {
            let current_alerts = alerts.get();
            if current_alerts.is_empty() {
                view! { <div class="hidden"></div> }.into_any()
            } else {
                view! {
                    <div class="mb-6 p-4 bg-amber-50 border-2 border-amber-400 rounded-lg">
                        <div class="flex items-center justify-between mb-3">
                            <h3 class="text-lg font-semibold text-amber-800 flex items-center gap-2">
                                <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9" />
                                </svg>
                                "🚗 Slots Available!"
                            </h3>
                            <button
                                class="text-sm text-amber-600 hover:text-amber-800 underline"
                                on:click=move |_| on_dismiss_all()
                            >
                                "Dismiss All"
                            </button>
                        </div>
                        <div class="space-y-2">
                            {current_alerts.into_iter().map(|alert| {
                                let loc_id = alert.location_id.clone();
                                let slot = alert.slot_time.clone();
                                view! {
                                    <div class="flex items-center justify-between bg-white p-3 rounded border border-amber-200">
                                        <div>
                                            <span class="font-medium text-amber-900">{alert.location_name}</span>
                                            <span class="text-amber-700 ml-2">{alert.slot_time}</span>
                                        </div>
                                        <button
                                            class="text-amber-500 hover:text-amber-700"
                                            on:click=move |_| on_dismiss(loc_id.clone(), slot.clone())
                                        >
                                            <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
                                                <path fill-rule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clip-rule="evenodd" />
                                            </svg>
                                        </button>
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    </div>
                }.into_any()
            }
        }}
    }
}

#[component]
pub fn AlertToggle(
    alerts_enabled: ReadSignal<bool>,
    on_toggle: impl Fn(bool) + 'static + Copy + Send + Sync,
) -> impl IntoView {
    view! {
        <div class="mb-6 p-4 bg-gray-50 border border-gray-200 rounded-lg">
            <div class="flex items-center justify-between">
                <div class="flex items-center gap-3">
                    <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 text-gray-600" viewBox="0 0 20 20" fill="currentColor">
                        <path d="M10 2a6 6 0 00-6 6v3.586l-.707.707A1 1 0 004 14h12a1 1 0 00.707-1.707L16 11.586V8a6 6 0 00-6-6zM10 18a3 3 0 01-3-3h6a3 3 0 01-3 3z" />
                    </svg>
                    <div>
                        <span class="font-semibold text-gray-700">"Alerts"</span>
                        <p class="text-sm text-gray-500">"Enable alerts and check locations in the table below to monitor"</p>
                    </div>
                </div>
                <div class="flex items-center gap-2">
                    {move || if alerts_enabled.get() {
                        view! { <span class="text-xs bg-green-100 text-green-800 px-2 py-0.5 rounded-full">"Enabled"</span> }.into_any()
                    } else {
                        view! { <span class="text-xs bg-gray-100 text-gray-600 px-2 py-0.5 rounded-full">"Disabled"</span> }.into_any()
                    }}
                    <button
                        class="relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-green-500 focus:ring-offset-2"
                        class:bg-green-600=move || alerts_enabled.get()
                        class:bg-gray-200=move || !alerts_enabled.get()
                        on:click=move |_| on_toggle(!alerts_enabled.get())
                    >
                        <span
                            class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out"
                            class:translate-x-5=move || alerts_enabled.get()
                            class:translate-x-0=move || !alerts_enabled.get()
                        ></span>
                    </button>
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn HomePage() -> impl IntoView {
    let (address_input, set_address_input) = create_signal(String::new());
    let (latitude, set_latitude) = create_signal(-33.8688197);
    let (longitude, set_longitude) = create_signal(151.2092955);
    let (current_location_name, set_current_location_name) = create_signal("Sydney".to_string());
    let (geocoding_status, set_geocoding_status) = create_signal::<Option<String>>(None);
    let (is_loading, set_is_loading) = create_signal(false);

    let (last_updated, set_last_updated) = create_signal::<Option<String>>(None);

    let (bookings, set_bookings) = create_signal(Vec::<LocationBookingViewModel>::new());
    let (is_fetching_bookings, set_is_fetching_bookings) = create_signal(false);

    let (booking_etag, set_booking_etag) = create_signal(String::new());

    let (reset_sort_trigger, set_reset_sort_trigger) = create_signal(());

    let location_manager = LocationManager::new();
    
    // Scraping control state
    let (scraping_status, set_scraping_status) = create_signal(ScrapingStatusResponse::default());
    let (scraping_message, set_scraping_message) = create_signal::<Option<String>>(None);
    
    // Alert state
    let (active_alerts, set_active_alerts) = create_signal::<Vec<SlotAlertResponse>>(vec![]);
    let (alerts_enabled, set_alerts_enabled) = create_signal(false);
    let (alert_configs, set_alert_configs) = create_signal::<Vec<AlertConfig>>(vec![]);

    let fetch_scraping_status = move || {
        leptos::task::spawn_local(async move {
            match get_scraping_status().await {
                Ok(status) => {
                    set_scraping_status(status);
                }
                Err(e) => {
                    leptos::logging::log!("Error fetching scraping status: {:?}", e);
                }
            }
        });
    };
    
    let fetch_active_alerts = move || {
        leptos::task::spawn_local(async move {
            match get_active_alerts().await {
                Ok(alerts) => {
                    set_active_alerts(alerts);
                }
                Err(e) => {
                    leptos::logging::log!("Error fetching active alerts: {:?}", e);
                }
            }
        });
    };
    
    let fetch_alert_settings = move || {
        leptos::task::spawn_local(async move {
            match get_alert_settings().await {
                Ok(settings) => {
                    set_alerts_enabled(settings.alerts_enabled);
                    set_alert_configs(settings.alerts);
                }
                Err(e) => {
                    leptos::logging::log!("Error fetching alert settings: {:?}", e);
                }
            }
        });
    };
    
    let save_current_settings = move || {
        let enabled = alerts_enabled.get_untracked();
        let configs = alert_configs.get_untracked();
        leptos::task::spawn_local(async move {
            if let Err(e) = save_alert_settings(enabled, configs).await {
                leptos::logging::log!("Error saving alert settings: {:?}", e);
            }
        });
    };
    
    let handle_start_scraping = move |_| {
        set_scraping_message(Some("Starting scraper...".to_string()));
        
        leptos::task::spawn_local(async move {
            match start_scraping().await {
                Ok(msg) => {
                    set_scraping_message(Some(msg));
                }
                Err(e) => {
                    set_scraping_message(Some(format!("Error: {:?}", e)));
                }
            }
        });
    };
    
    let handle_dismiss_alert = move |location_id: String, slot_time: String| {
        leptos::task::spawn_local(async move {
            if let Err(e) = dismiss_alert(location_id, slot_time).await {
                leptos::logging::log!("Error dismissing alert: {:?}", e);
            }
            fetch_active_alerts();
        });
    };
    
    let handle_dismiss_all = move || {
        leptos::task::spawn_local(async move {
            if let Err(e) = dismiss_all_alerts().await {
                leptos::logging::log!("Error dismissing all alerts: {:?}", e);
            }
            fetch_active_alerts();
        });
    };
    
    let handle_toggle_alerts_enabled = move |enabled: bool| {
        set_alerts_enabled(enabled);
        save_current_settings();
    };
    
    let handle_toggle_alert = move |location_id: String, location_name: String, enabled: bool| {
        set_alert_configs.update(|configs| {
            if let Some(config) = configs.iter_mut().find(|c| c.location_id == location_id) {
                config.enabled = enabled;
            } else if enabled {
                configs.push(AlertConfig {
                    location_id,
                    location_name,
                    enabled: true,
                    months: vec![3, 4, 5, 6], // Default to Mar-Jun
                });
            }
        });
        save_current_settings();
    };
    
    let handle_toggle_month = move |location_id: String, month: u32| {
        set_alert_configs.update(|configs| {
            if let Some(config) = configs.iter_mut().find(|c| c.location_id == location_id) {
                if config.months.contains(&month) {
                    config.months.retain(|&m| m != month);
                } else {
                    config.months.push(month);
                    config.months.sort();
                }
            }
        });
        save_current_settings();
    };
    
    // Poll scraping status and alerts when page loads
    #[cfg(not(feature = "ssr"))]
    {
        fetch_scraping_status();
        fetch_active_alerts();
        fetch_alert_settings();
        
        Effect::new(move |_| {
            let handle = set_interval_with_handle(
                move || {
                    fetch_scraping_status();
                    fetch_active_alerts();
                },
                Duration::from_secs(2),
            )
            .expect("failed to set scraping status interval");

            on_cleanup(move || {
                handle.clear();
            });

            || {}
        });
    }

    let fetch_bookings = move || {
        set_is_fetching_bookings(true);

        leptos::task::spawn_local(async move {
            match get_location_bookings(booking_etag.get_untracked()).await {
                Ok(data) => {
                    match data {
                        Some(data) => {
                            set_bookings(data.bookings);
                            set_last_updated(data.last_updated);
                            set_booking_etag(data.etag);
                        }
                        None => {}
                    };
                }
                Err(err) => {
                    leptos::logging::log!("Error fetching bookings: {:?}", err);
                }
            }
            set_is_fetching_bookings(false);
        });
    };

    #[cfg(not(feature = "ssr"))]
    fetch_bookings();

    #[cfg(not(feature = "ssr"))]
    Effect::new(move |_| {
        leptos::logging::log!("Setting up client-side refresh mechanism");

        let handle = set_interval_with_handle(
            move || {
                leptos::logging::log!("Triggering refresh");
                fetch_bookings();
            },
            Duration::from_secs(600),
        )
        .expect("failed to set interval");

        on_cleanup(move || {
            handle.clear();
        });

        || {}
    });

    let handle_geocode = move |_| {
        let address = address_input.get();
        if address.is_empty() {
            set_geocoding_status(Some("Please enter a location".to_string()));
            return;
        }

        set_geocoding_status(Some("Searching...".to_string()));
        set_is_loading(true);

        leptos::task::spawn_local(async move {
            match geocode_address(&address).await {
                Ok(result) => {
                    set_latitude(result.latitude);
                    set_longitude(result.longitude);
                    set_current_location_name(result.display_name);
                    set_geocoding_status(None);
                    set_is_loading(false);
                    set_reset_sort_trigger(());
                }
                Err(err) => {
                    set_geocoding_status(Some(format!("Error: {}", err)));
                    set_is_loading(false);
                }
            }
        });
    };

    use leptos::wasm_bindgen::JsCast;
    use web_sys::Geolocation;

    #[cfg(not(feature = "ssr"))]
    {
        create_effect(move |_| {
            if let Some(window) = web_sys::window() {
                if let Ok(geolocation) = window.navigator().geolocation() {
                    let success_callback = Closure::<dyn FnMut(web_sys::Position)>::new(
                        move |position: web_sys::Position| {
                            set_latitude(position.coords().latitude());
                            set_longitude(position.coords().longitude());
                            set_address_input(format!(
                                "{}, {}",
                                position.coords().latitude(),
                                position.coords().longitude()
                            ));
                            handle_geocode(());
                        },
                    );

                    let _ =
                        geolocation.get_current_position(success_callback.as_ref().unchecked_ref());

                    success_callback.forget();
                }
            }
        });
    }

    view! {
        <div class="max-w-4xl mx-auto p-4">
            <div class="flex justify-between items-center mb-6">
                <h2 class="text-2xl font-bold text-gray-800">NSW Available Drivers Tests</h2>
            </div>

            // Alert Banner (shows when there are active alerts)
            <AlertBanner 
                alerts=active_alerts
                on_dismiss=handle_dismiss_alert
                on_dismiss_all=handle_dismiss_all
            />

            // Alert Toggle (simplified)
            <AlertToggle
                alerts_enabled=alerts_enabled
                on_toggle=handle_toggle_alerts_enabled
            />

            // Scraping Control Panel
            <div class="mb-6 p-4 bg-gray-50 border border-gray-200 rounded-lg">
                <div class="flex items-center justify-between mb-3">
                    <h3 class="text-lg font-semibold text-gray-700">Scraping Control</h3>
                    <button
                        class="px-4 py-2 bg-green-600 text-white rounded-md hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-green-500 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
                        on:click=handle_start_scraping
                        disabled=move || scraping_status.get().is_running
                    >
                        {move || if scraping_status.get().is_running { "Scraping..." } else { "Start Scraping" }}
                    </button>
                </div>
                
                // Status message
                {move || scraping_message.get().map(|msg| view! {
                    <div class="mb-3 text-sm text-blue-600">{msg}</div>
                })}
                
                // Progress bar with three states: idle, running, completed
                {move || {
                    let status = scraping_status.get();
                    
                    // State C: Completed (not running, but has completion info)
                    if !status.is_running && status.last_completed_at.is_some() {
                        let duration_text = status.last_duration_secs.map(|secs| {
                            let mins = secs / 60;
                            let secs_rem = secs % 60;
                            if mins > 0 {
                                format!("Total time: {}m {}s", mins, secs_rem)
                            } else {
                                format!("Total time: {}s", secs_rem)
                            }
                        }).unwrap_or_default();
                        
                        view! {
                            <div class="space-y-2">
                                <div class="flex justify-between text-sm">
                                    <span class="text-green-600 font-medium flex items-center gap-1">
                                        <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
                                            <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
                                        </svg>
                                        "Scraping completed!"
                                    </span>
                                    <span class="text-gray-600">{duration_text}</span>
                                </div>
                                <div class="w-full bg-gray-200 rounded-full h-3">
                                    <div class="bg-green-500 h-3 rounded-full w-full"></div>
                                </div>
                                <div class="text-sm text-gray-500">
                                    {format!("Successfully scraped {}/{} locations", status.completed_count, status.total_count)}
                                </div>
                                {status.error_message.clone().map(|err| view! {
                                    <div class="text-sm text-red-600">{err}</div>
                                })}
                            </div>
                        }.into_any()
                    }
                    // State B: Currently running
                    else if status.is_running {
                        let progress = if status.total_count > 0 {
                            (status.completed_count as f64 / status.total_count as f64 * 100.0) as u32
                        } else {
                            0
                        };
                        
                        let eta_text = status.estimated_remaining_secs.map(|secs| {
                            let mins = secs / 60;
                            let secs_rem = secs % 60;
                            if mins > 0 {
                                format!("~{}m {}s remaining", mins, secs_rem)
                            } else {
                                format!("~{}s remaining", secs_rem)
                            }
                        }).unwrap_or_else(|| "Calculating...".to_string());
                        
                        view! {
                            <div class="space-y-2">
                                <div class="flex justify-between text-sm text-gray-600">
                                    <span>
                                        {format!("Progress: {}/{} locations", status.completed_count, status.total_count)}
                                    </span>
                                    <span>{eta_text}</span>
                                </div>
                                <div class="w-full bg-gray-200 rounded-full h-3">
                                    <div 
                                        class="bg-green-500 h-3 rounded-full transition-all duration-500"
                                        style=format!("width: {}%", progress)
                                    ></div>
                                </div>
                                {status.current_location.clone().map(|loc| view! {
                                    <div class="text-sm text-gray-500">
                                        "Currently scraping: " <span class="font-medium">{loc}</span>
                                    </div>
                                })}
                                {status.error_message.clone().map(|err| view! {
                                    <div class="text-sm text-red-600">{err}</div>
                                })}
                            </div>
                        }.into_any()
                    }
                    // State A: Idle (never ran or reset)
                    else {
                        view! { 
                            <div class="text-sm text-gray-500">
                                "No scraping in progress. Click \"Start Scraping\" to begin."
                            </div> 
                        }.into_any()
                    }
                }}
            </div>

            <div class="mb-6">
                <div class="flex flex-wrap gap-4 items-end">
                    <div class="flex flex-col flex-grow">
                        <label for="address" class="text-sm font-medium text-gray-700 mb-1">
                            Search by Postcode, Address, or Suburb:
                        </label>
                        <input
                            id="address"
                            type="text"
                            class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                            placeholder="e.g., Sydney, 2000, 42 Wallaby Way"
                            prop:value={address_input}
                            on:input=move |ev| set_address_input(event_target_value(&ev))
                            on:keydown=move |ev| {
                                if ev.key() == "Enter" {
                                    handle_geocode(());
                                }
                            }
                        />
                        <p class="mt-1 text-xs text-gray-500 italic">Your search is securely processed through nominatim.org, a trusted open-source geolocation service. No personal or identifying information is shared during this process.</p>
                    </div>
                </div>

                <div class="flex items-center gap-4 mt-2 w-full">
                    <button
                        class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 transition-colors"
                        on:click=move |_| handle_geocode(())
                    >
                        Search
                    </button>

                    <div class="ml-auto text-sm text-gray-500">
                        {move || match last_updated.get() {
                            Some(time) => view! {
                                <span>"Data last updated: " <TimeDisplay iso_time={time} /></span>
                            }.into_any(),
                            None => view! { <span>"Data last updated: unknown"</span> }.into_any(),
                        }}
                    </div>
                </div>

                <div class="mt-2">
                    {move || {
                        match geocoding_status.get() {
                            Some(status) => view! {
                                <div class="text-sm mt-2 text-amber-600">
                                    {status}
                                </div>
                            }.into_any(),
                            None => view! { <div class="hidden"></div> }.into_any()
                        }
                    }}
                </div>

                <div class="mt-4 flex flex-wrap gap-4 items-end">
                    <div class="flex flex-wrap gap-4">
                        <div class="flex flex-col">
                            <label class="text-sm font-medium text-gray-700 mb-1">Current Coordinates:</label>
                            <div class="text-sm text-gray-600">
                                {move || format!("Lat: {:.6}, Lng: {:.6}", latitude.get(), longitude.get())}
                            </div>
                        </div>

                        <div class="flex flex-col">
                            <label class="text-sm font-medium text-gray-700 mb-1">Location:</label>
                            <div class="text-sm text-gray-600 max-w-md truncate">
                                {move || current_location_name.get()}
                            </div>
                        </div>
                    </div>
                </div>

                <p class="mt-1 text-xs text-gray-500 italic">
                  "Disclaimer: Pass rates shown are calculated based on the "
                  <span class="text-amber-600">center</span> " of the customer's local government
                  area (LGA) and weighted according to proximity to nearby testing centers. "
                  <span class="text-amber-600">These rates are estimates only.</span>
                  " Data is from 2022-2025 C Class Driver tests."
                </p>
            </div>

            <LocationsTable
                bookings=bookings
                is_loading=is_fetching_bookings
                latitude=latitude
                longitude=longitude
                location_manager=location_manager.clone()
                reset_sort_trigger=reset_sort_trigger
                alerts_enabled=alerts_enabled
                alert_configs=alert_configs
                on_toggle_alert=handle_toggle_alert
                on_toggle_month=handle_toggle_month
            />

            <div class="mt-6 flex justify-between items-center">
                <div class="text-sm text-gray-500">
                    <p>Location search results are made using "https://nominatim.org/" and are always done on your browser, your location information never touches our servers</p>
                    <p>Note: Distances are calculated using the Haversine formula and represent "as the crow flies" distance.</p>
                    <p>You can support me by giving me a github star</p>
                </div>

                <div class="flex gap-2">
                    <a
                        href="https://github.com/teehee567/nsw-drivers-test"
                        target="_blank"
                        class="px-3 py-1.5 bg-gray-800 text-white rounded-md hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-gray-500 transition-colors inline-flex items-center justify-center gap-2"
                    >
                        <i class="fab fa-github"></i>
                        <span>View on GitHub</span>
                    </a>
                </div>
            </div>
        </div>
    }
}
