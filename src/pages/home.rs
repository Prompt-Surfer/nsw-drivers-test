use std::collections::HashMap;
use std::time::Duration;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::location::LocationManager;
use crate::data::shared_booking::TimeSlot;
use crate::utils::geocoding::geocode_address;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationBookingViewModel {
    pub location: String,
    pub earliest_slot: Option<TimeSlot>,
}

#[server(GetBookings)]
pub async fn get_location_bookings() -> Result<Vec<LocationBookingViewModel>, ServerFnError> {
    use crate::data::booking::BookingManager;
    
    let booking_data = BookingManager::get_data();
    
    let view_models: Vec<_> = booking_data.results.iter().map(|location_booking| {
        let earliest_slot = location_booking.slots.iter()
            .filter(|slot| slot.availability)
            .min_by(|a, b| a.start_time.cmp(&b.start_time))
            .cloned();
        
        LocationBookingViewModel {
            location: location_booking.location.clone(),
            earliest_slot
        }
    }).collect();
    
    Ok(view_models)
}

#[component]
fn LocationsTable(
    bookings: ReadSignal<Vec<LocationBookingViewModel>>,
    is_loading: ReadSignal<bool>,
    latitude: ReadSignal<f64>,
    longitude: ReadSignal<f64>,
    location_manager: LocationManager
) -> impl IntoView {
    let booking_map = create_memo(move |_| {
        bookings.get().into_iter()
            .map(|booking| (booking.location.clone(), booking.earliest_slot))
            .collect::<HashMap<String, Option<TimeSlot>>>()
    });
    
    let sorted_locations = create_memo(move |_| {
        let locations_by_distance = location_manager.get_by_distance(latitude.get(), longitude.get());
        locations_by_distance
    });

    view! {
        <div class="overflow-x-auto">
            <table class="min-w-full bg-white border border-gray-200 rounded-lg overflow-hidden">
                <thead class="bg-gray-50">
                    <tr>
                        <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Name</th>
                        <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Distance (km)</th>
                        <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Earliest Available Slot</th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-gray-200">
                    {move || {
                        let locations = sorted_locations.get();
                        let booking_data = booking_map.get();
                        log::info!("Rendering {} sorted locations", locations.len());
                        
                        locations.into_iter().map(|(loc, distance)| {
                            let location_id = loc.id.to_string();
                            let earliest_slot = booking_data.get(&location_id).cloned().flatten();
                            
                            view! {
                                <tr class="hover:bg-gray-50 transition-colors">
                                    <td class="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-900">{loc.name}</td>
                                    <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                                        {format!("{:.2}", distance)}
                                    </td>
                                    <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                                        {match earliest_slot {
                                            Some(slot) => view! { 
                                                <span class="text-green-600 font-medium">{slot.start_time}</span>
                                            }.into_any(),
                                            None => {
                                                if is_loading.get() {
                                                    view! { <span class="text-gray-400">Loading...</span> }.into_any()
                                                } else {
                                                    view! { <span class="text-gray-400">No availability</span> }.into_any()
                                                }
                                            }
                                        }}
                                    </td>
                                </tr>
                            }
                        }).collect::<Vec<_>>()
                    }}
                </tbody>
            </table>
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
    
    let (bookings, set_bookings) = create_signal(Vec::<LocationBookingViewModel>::new());
    let (is_fetching_bookings, set_is_fetching_bookings) = create_signal(false);
    
    let location_manager = LocationManager::new();
    
    let fetch_bookings = move || {
        set_is_fetching_bookings(true);
        
        leptos::task::spawn_local(async move {
            match get_location_bookings().await {
                Ok(data) => {
                    set_bookings(data);
                },
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
            Duration::from_secs(30)
        ).expect("failed to set interval");
        
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
                },
                Err(err) => {
                    set_geocoding_status(Some(format!("Error: {}", err)));
                    set_is_loading(false);
                }
            }
        });
    };

    view! {
        <div class="max-w-4xl mx-auto p-4">
            <div class="flex justify-between items-center mb-6">
                <h2 class="text-2xl font-bold text-gray-800">NSW Available Drivers Tests</h2>
                <a 
                    href="https://github.com/teehee567/nsw-drivers-test"
                    target="_blank"
                    class="px-3 py-1.5 bg-gray-800 text-white rounded-md hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-gray-500 transition-colors flex items-center gap-2"
                >
                    <i class="fab fa-github w-5 h-5"></i>
                    <span>View on GitHub</span>
                </a>
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
                        <p class="mt-1 text-xs text-gray-500 italic">Your search data is processed locally and not sent to our servers.</p>
                    </div>
                </div>
                    
                <button 
                    class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 transition-colors"
                    on:click=move |_| handle_geocode(())
                >
                    Search
                </button>
                
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
            </div>
            
            <LocationsTable 
                bookings=bookings
                is_loading=is_fetching_bookings
                latitude=latitude
                longitude=longitude
                location_manager=location_manager.clone()
            />
            
            <div class="mt-6 flex justify-between items-center">
                <div class="text-sm text-gray-500">
                    <p>Note: Distances are calculated using the Haversine formula and represent "as the crow flies" distance.</p>
                </div>
                
                <button 
                    class="px-3 py-1 text-sm bg-gray-100 text-gray-700 rounded-md hover:bg-gray-200 focus:outline-none focus:ring-2 focus:ring-gray-300 transition-colors flex items-center gap-1"
                    on:click=move |_| fetch_bookings()
                    disabled=is_fetching_bookings
                >
                    <span>{move || if is_fetching_bookings.get() { "Refreshing..." } else { "Refresh" }}</span>
                </button>
            </div>
        </div>
    }
}
