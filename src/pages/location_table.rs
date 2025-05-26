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

#[derive(Debug, Clone, Copy, PartialEq)]
enum SortColumn {
    Name,
    Distance,
    EarliestSlot,
    PassRate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SortDirection {
    Ascending,
    Descending,
}

#[component]
fn SortableHeader(
    column: SortColumn,
    current_sort: ReadSignal<SortColumn>,
    sort_direction: ReadSignal<SortDirection>,
    on_sort: WriteSignal<SortColumn>,
    title: &'static str,
    mobile_title: Option<&'static str>,
) -> impl IntoView {
    let sort_icon = move || {
        if current_sort.get() == column {
            match sort_direction.get() {
                SortDirection::Ascending => "↑",
                SortDirection::Descending => "↓",
            }
        } else {
            "↕"
        }
    };

    view! {
        <th class="px-1 py-2 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
            <button
                class="flex items-center gap-1 hover:text-gray-700 transition-colors"
                on:click=move |_| on_sort.set(column)
            >
                {move || {
                    if let Some(mobile) = mobile_title {
                        view! {
                            <>
                                <span class="hidden md:inline">{title}</span>
                                <span class="md:hidden">{mobile}</span>
                            </>
                        }.into_any()
                    } else {
                        view! {
                            <span>{title}</span>
                        }.into_any()
                    }
                }}
                <span class="text-gray-400">{sort_icon}</span>
            </button>
        </th>
    }
}

#[component]
fn TableHeader(
    sort_column: ReadSignal<SortColumn>,
    sort_direction: ReadSignal<SortDirection>,
    on_sort: WriteSignal<SortColumn>,
) -> impl IntoView {
    view! {
        <thead class="bg-gray-50">
            <tr>
                <th class="px-2 py-2 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    <button
                        class="flex items-center gap-1 hover:text-gray-700 transition-colors"
                        on:click=move |_| on_sort.set(SortColumn::Name)
                    >
                        <span>Name</span>
                        <span class="text-gray-400">
                            {move || if sort_column.get() == SortColumn::Name {
                                match sort_direction.get() {
                                    SortDirection::Ascending => "↑",
                                    SortDirection::Descending => "↓",
                                }
                            } else {
                                "↕"
                            }}
                        </span>
                    </button>
                </th>
                <SortableHeader
                    column=SortColumn::Distance
                    current_sort=sort_column
                    sort_direction=sort_direction
                    on_sort=on_sort
                    title="Distance"
                    mobile_title=Some("Dist")
                />
                <SortableHeader
                    column=SortColumn::EarliestSlot
                    current_sort=sort_column
                    sort_direction=sort_direction
                    on_sort=on_sort
                    title="Earliest Slot"
                    mobile_title=Some("Slot")
                />
                <SortableHeader
                    column=SortColumn::PassRate
                    current_sort=sort_column
                    sort_direction=sort_direction
                    on_sort=on_sort
                    title="Pass Rate"
                    mobile_title=Some("Pass %")
                />
                <th class="px-1 py-2 text-center text-xs font-medium text-gray-500 uppercase tracking-wider">
                    <span class="sr-only">Details</span>
                </th>
            </tr>
        </thead>
    }
}

#[component]
fn InfoBanner() -> impl IntoView {
    view! {
        <>
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
        </>
    }
}

#[component]
pub fn LocationsTable(
    bookings: ReadSignal<Vec<LocationBookingViewModel>>,
    is_loading: ReadSignal<bool>,
    latitude: ReadSignal<f64>,
    longitude: ReadSignal<f64>,
    location_manager: LocationManager,
) -> impl IntoView {
    let (sort_column, set_sort_column) = create_signal(SortColumn::Distance);
    let (sort_direction, set_sort_direction) = create_signal(SortDirection::Descending);

    let booking_map = create_memo(move |_| {
        bookings
            .get()
            .into_iter()
            .map(|booking| (booking.location.clone(), booking.earliest_slot))
            .collect::<HashMap<String, Option<TimeSlot>>>()
    });

    let sorted_locations = create_memo(move |_| {
        let mut locations_by_distance =
            location_manager.get_by_distance(latitude.get(), longitude.get());
        
        let booking_data = booking_map.get();
        let current_sort = sort_column.get();
        let current_direction = sort_direction.get();
        
        locations_by_distance.sort_by(|a, b| {
            let comparison = match current_sort {
                SortColumn::Name => a.0.name.cmp(&b.0.name),
                SortColumn::Distance => a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::EarliestSlot => {
                    let a_slot = booking_data.get(&a.0.id.to_string()).cloned().flatten();
                    let b_slot = booking_data.get(&b.0.id.to_string()).cloned().flatten();
                    
                    match (a_slot, b_slot) {
                        (Some(a_time), Some(b_time)) => a_time.start_time.cmp(&b_time.start_time),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    }
                },
                SortColumn::PassRate => {
                    a.0.pass_rate.partial_cmp(&b.0.pass_rate).unwrap_or(std::cmp::Ordering::Equal)
                },
            };
            
            match current_direction {
                SortDirection::Ascending => comparison,
                SortDirection::Descending => comparison.reverse(),
            }
        });
        
        locations_by_distance
    });

    let handle_sort = move |column: SortColumn| {
        if sort_column.get() == column {
            set_sort_direction.update(|dir| {
                *dir = match *dir {
                    SortDirection::Ascending => SortDirection::Descending,
                    SortDirection::Descending => SortDirection::Ascending,
                }
            });
        } else {
            set_sort_column.set(column);
            set_sort_direction.set(SortDirection::Descending);
        }
    };

    let (sort_trigger, set_sort_trigger) = create_signal(SortColumn::Distance);
    
    create_effect(move |_| {
        handle_sort(sort_trigger.get());
    });

    view! {
        <div>
            <InfoBanner />
            <div class="overflow-x-auto">
                <table class="min-w-full bg-white border border-gray-200 rounded-lg overflow-hidden table-fixed">
                    <colgroup>
                        <col style="width: 15%;" />
                        <col style="width: 12%;" />
                        <col style="width: 28%;" />
                        <col style="width: 15%;" />
                        <col style="width: 10%;" />
                    </colgroup>
                    <TableHeader
                        sort_column=sort_column
                        sort_direction=sort_direction
                        on_sort=set_sort_trigger
                    />
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
