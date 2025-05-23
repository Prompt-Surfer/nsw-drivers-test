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

use crate::pages::home::LocationBookingViewModel;

use crate::pages::location_row::LocationRow;

#[component]
pub fn LocationsTable(
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
