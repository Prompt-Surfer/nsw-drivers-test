use std::collections::HashMap;
use std::time::Duration;

use leptos::prelude::*;
use leptos::server_fn::error::NoCustomError;
use reqwest::header;
use serde::{Deserialize, Serialize};
use web_sys::wasm_bindgen::prelude::Closure;

use crate::data::location::LocationManager;
use crate::data::shared_booking::TimeSlot;
use crate::utils::date::format_iso_date;
use crate::utils::geocoding::geocode_address;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationBookingViewModel {
    pub location: String,
    pub earliest_slot: Option<TimeSlot>,
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
        // WARN: for some reason this makes it open in hte browser
        // response.set_status(StatusCode::NOT_MODIFIED);
        return Ok(None);
    }

    let view_models: Vec<_> = booking_data
        .results
        .iter()
        .map(|location_booking| {
            let earliest_slot = location_booking
                .slots
                .iter()
                .filter(|slot| slot.availability)
                .min_by(|a, b| a.start_time.cmp(&b.start_time))
                .cloned();

            LocationBookingViewModel {
                location: location_booking.location.clone(),
                earliest_slot,
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
        // WARN: for some reason this makes it open in hte browser
        // response.set_status(StatusCode::NOT_MODIFIED);
        return Ok(None);
    }

    Ok(Some(LocationDetailBookingResponse {
        location: location_booking.location,
        slots: location_booking.slots,
        etag: server_etag,
    }))
}

#[component]
fn ExpandedLocationDetails(location_id: String, expanded: ReadSignal<bool>) -> impl IntoView {
    let (slots, set_slots) = create_signal(Vec::<TimeSlot>::new());
    let (is_loading, set_is_loading) = create_signal(false);
    let (error, set_error) = create_signal::<Option<String>>(None);

    let (location_etag, set_location_etag) = create_signal(String::new());

    let slots_by_date = create_memo(move |_| {
        let mut grouped: HashMap<String, Vec<TimeSlot>> = HashMap::new();

        for slot in slots.get().iter() {
            if slot.availability {
                if let Some(date_part) = slot.start_time.split_whitespace().next() {
                    let entry = grouped
                        .entry(date_part.to_string())
                        .or_insert_with(Vec::new);
                    entry.push(slot.clone());
                }
            }
        }

        let mut dates: Vec<_> = grouped.into_iter().collect();
        dates.sort_by(|(date_a, _), (date_b, _)| {
            let parts_a: Vec<&str> = date_a.split('/').collect();
            let parts_b: Vec<&str> = date_b.split('/').collect();

            if parts_a.len() == 3 && parts_b.len() == 3 {
                let year_compare = parts_a[2].cmp(parts_b[2]);
                if year_compare != std::cmp::Ordering::Equal {
                    return year_compare;
                }

                let month_compare = parts_a[1].cmp(parts_b[1]);
                if month_compare != std::cmp::Ordering::Equal {
                    return month_compare;
                }

                return parts_a[0].cmp(parts_b[0]);
            }

            date_a.cmp(date_b)
        });

        dates
    });

    create_effect(move |_| {
        if expanded.get() {
            let location_id_clone = location_id.clone();

            set_is_loading(true);
            set_error(None);

            leptos::task::spawn_local(async move {
                match get_location_details(location_id_clone, location_etag.get_untracked()).await {
                    Ok(response) => match response {
                        Some(response) => {
                            set_slots(response.slots);
                            set_location_etag(response.etag);
                        }
                        None => {}
                    },
                    Err(err) => {
                        set_error(Some(format!("Error loading details: {}", err)));
                    }
                }
                set_is_loading(false);
            });
        }
    });

    view! {
        <Show when=move || expanded.get()>
            <tr>
                <td colspan="5" class="px-6 py-4 bg-gray-50">
                    {move || {
                        if is_loading.get() {
                            view! {
                                <div class="flex justify-center items-center py-4">
                                    <div class="animate-spin rounded-full h-8 w-8 border-t-2 border-b-2 border-blue-500"></div>
                                </div>
                            }.into_any()
                        } else if let Some(err) = error.get() {
                            view! {
                                <div class="text-red-500 py-2">{err}</div>
                            }.into_any()
                        } else {
                            let dates = slots_by_date.get();

                            if dates.is_empty() {
                                view! {
                                    <div class="text-gray-500 py-2 text-center">No available slots</div>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="max-h-80 overflow-y-auto">
                                        <h3 class="text-lg font-medium mb-2">Available Times</h3>
                                        <div class="space-y-4">
                                            {dates.into_iter().map(|(date, slots)| {
                                                view! {
                                                    <div class="border-b border-gray-200 pb-2">
                                                        <h4 class="font-medium text-gray-700 mb-1">{date}</h4>
                                                        <div class="flex flex-wrap gap-2">
                                                            {slots.into_iter().map(|slot| {
                                                                let time_only = slot.start_time
                                                                    .split_whitespace()
                                                                    .nth(1)
                                                                    .unwrap_or(&slot.start_time)
                                                                    .to_string();

                                                                view! {
                                                                    <span class="inline-block bg-green-100 text-green-800 px-2 py-1 text-sm rounded">
                                                                        {time_only}
                                                                    </span>
                                                                }
                                                            }).collect::<Vec<_>>()}
                                                        </div>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    </div>
                                }.into_any()
                            }
                        }
                    }}
                </td>
            </tr>
        </Show>
    }
}

#[component]
fn LocationRow(
    loc: crate::data::location::Location,
    distance: f64,
    earliest_slot: Option<TimeSlot>,
    is_loading: ReadSignal<bool>,
) -> impl IntoView {
    let (expanded, set_expanded) = create_signal(false);

    let toggle_expand = move |_| {
        set_expanded.update(|val| *val = !*val);
    };

    let total_tests = loc.passes + loc.failures;
    let low_data = total_tests < 1000;

    view! {
        <>
            <tr class="hover:bg-gray-50 group transition-colors cursor-pointer border-l-4 border-transparent hover:border-blue-300 relative"
                on:click=toggle_expand>

                <td class="px-2 py-3 md:px-4 md:py-3 whitespace-nowrap text-sm font-medium text-gray-900 truncate">
                    {loc.name}
                </td>

                <td class="px-1 py-3 md:px-3 md:py-3 whitespace-nowrap text-sm text-gray-500">
                    {format!("{:.1}", distance)}
                </td>

                <td class="px-1 py-3 md:px-3 md:py-3 whitespace-nowrap text-sm text-gray-500">
                    {match earliest_slot {
                        Some(slot) => view! {
                            <span class="text-green-600 font-medium">{slot.start_time}</span>
                        }.into_any(),
                        None => {
                            if is_loading.get_untracked() {
                                view! { <span class="text-gray-400">Loading...</span> }.into_any()
                            } else {
                                view! { <span class="text-gray-400">No availability</span> }.into_any()
                            }
                        }
                    }}
                </td>

                <td class="px-1 py-3 md:px-3 md:py-3 whitespace-nowrap text-sm text-gray-500">
                    {move || {
                        let pass_rate = loc.pass_rate;
                        let color_class = if low_data {
                            "bg-yellow-500"
                        } else if pass_rate >= 90.0 {
                            "bg-green-500"
                        } else if pass_rate >= 80.0 {
                            "bg-green-400"
                        } else if pass_rate >= 70.0 {
                            "bg-green-300"
                        } else if pass_rate >= 60.0 {
                            "bg-green-200"
                        } else if pass_rate >= 50.0 {
                            "bg-green-100"
                        } else {
                            "bg-gray-100"
                        };

                        view! {
                            <div class="flex items-center gap-1">
                                <span class={format!("px-1 py-0.5 md:px-2 md:py-1 rounded-md text-gray-900 text-xs md:text-sm {}", color_class)}>
                                    <span class="md:hidden">{format!("{:.0}%", pass_rate)}</span>
                                    <span class="hidden md:inline">{format!("{:.1}%", pass_rate)}</span>
                                </span>

                                {if low_data {
                                    let (tooltip_visible, set_tooltip_visible) = create_signal(false);

                                    view! {
                                        <div class="relative inline-block ml-0.5">
                                            <span
                                                class="text-red-700 cursor-help"
                                                on:mouseenter=move |_| set_tooltip_visible(true)
                                                on:mouseleave=move |_| set_tooltip_visible(false)
                                            >
                                                <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4 md:h-5 md:w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                                                </svg>
                                            </span>
                                            <div
                                                class={move || format!("absolute left-0 bottom-full mb-2 inline-block max-w-40 bg-gray-700 bg-opacity-90 text-white text-xs rounded py-1.5 px-2 z-10 shadow-md transition-opacity duration-150 {} {}",
                                                    if tooltip_visible.get() { "opacity-100" } else { "opacity-0" },
                                                    if tooltip_visible.get() { "pointer-events-auto" } else { "pointer-events-none" }
                                                )}
                                            >
                                                Less than 1000 tests
                                            </div>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <span></span> }.into_any()
                                }}
                            </div>
                        }
                    }}
                </td>

                <td class="px-6 py-4 whitespace-nowrap text-sm text-center">
                    <span class={move || {
                        if expanded.get() {
                            "rotate-180 inline-block transition-all duration-200 text-blue-600"
                        } else {
                            "inline-block transition-all duration-200 text-gray-500"
                        }
                    }}>
                        <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
                            <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
                        </svg>
                    </span>
                </td>
            </tr>

            <ExpandedLocationDetails
                location_id=loc.id.to_string()
                expanded=expanded
            />
        </>
    }
}

#[component]
fn LocationsTable(
    bookings: ReadSignal<Vec<LocationBookingViewModel>>,
    is_loading: ReadSignal<bool>,
    latitude: ReadSignal<f64>,
    longitude: ReadSignal<f64>,
    location_manager: LocationManager,
) -> impl IntoView {
    let booking_map = create_memo(move |_| {
        bookings
            .get()
            .into_iter()
            .map(|booking| (booking.location.clone(), booking.earliest_slot))
            .collect::<HashMap<String, Option<TimeSlot>>>()
    });

    let sorted_locations = create_memo(move |_| {
        let locations_by_distance =
            location_manager.get_by_distance(latitude.get(), longitude.get());
        locations_by_distance
    });

    view! {
        <div>
            <div class="md:hidden flex justify-center items-center bg-blue-50 p-3 mb-3 rounded-lg border border-blue-200">
                <div class="flex items-center gap-2 text-sm text-blue-800">
                    <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
                        <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd" />
                    </svg>
                    <span>Tap any location to view available time slots</span>
                </div>
            </div>

            <div class="hidden md:flex mb-3 text-sm text-gray-600 bg-blue-50 p-3 rounded-md items-center gap-2 border border-blue-200">
                <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 text-blue-500" viewBox="0 0 20 20" fill="currentColor">
                    <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd" />
                </svg>
                <span>Click on any row to view available time slots for that location</span>
            </div>
            <div class="overflow-x-auto">
                <table class="min-w-full bg-white border border-gray-200 rounded-lg overflow-hidden table-fixed">
                    <colgroup>
                        <col style="width: 15%;" />
                        <col style="width: 12%;" />
                        <col style="width: 28%;" />
                        <col style="width: 15%;" />
                        <col style="width: 10%;" />
                    </colgroup>
                    <thead class="bg-gray-50">
                        <tr>
                            <th class="px-2 py-2 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Name</th>
                            <th class="px-1 py-2 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                <span class="hidden md:inline">Distance</span>
                                <span class="md:hidden">Dist</span>
                            </th>
                            <th class="px-1 py-2 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                <span class="hidden md:inline">Earliest Slot</span>
                                <span class="md:hidden">Slot</span>
                            </th>
                            <th class="px-1 py-2 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                <span class="hidden md:inline">Pass Rate</span>
                                <span class="md:hidden">Pass %</span>
                            </th>
                            <th class="px-1 py-2 text-center text-xs font-medium text-gray-500 uppercase tracking-wider">
                                <span class="sr-only">Details</span>
                            </th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-gray-200">
                        {move || {
                            let locations = sorted_locations.get();
                            let booking_data = booking_map.get();

                            locations.into_iter().map(|(loc, distance)| {
                                let location_id = loc.id.to_string();
                                let earliest_slot = booking_data.get(&location_id).cloned().flatten();

                                view! {
                                    <LocationRow
                                        loc=loc
                                        distance=distance
                                        earliest_slot=earliest_slot
                                        is_loading=is_loading
                                    />
                                }
                            }).collect::<Vec<_>>()
                        }}
                    </tbody>
                </table>
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

    let location_manager = LocationManager::new();

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

                <div class="flex items-center gap-4 mt-2 w-full">
                    <button
                        class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 transition-colors"
                        on:click=move |_| handle_geocode(())
                    >
                        Search
                    </button>

                    <div class="ml-auto text-sm text-gray-500">
                        {move || match last_updated.get() {
                            Some(time) => format!("Data last updated: {}", format_iso_date(&time)),
                            None => "Data last updated: unknown".to_string(),
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
