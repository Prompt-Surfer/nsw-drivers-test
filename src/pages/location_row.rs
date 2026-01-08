use std::collections::HashMap;
use std::time::Duration;

use leptos::prelude::*;
use leptos::callback::Callback;
use leptos::server_fn::error::NoCustomError;
use reqwest::header;
use serde::{Deserialize, Serialize};
use web_sys::wasm_bindgen::prelude::Closure;

use crate::data::location::LocationManager;
use crate::data::shared_booking::TimeSlot;
use crate::settings::AlertConfig;
use crate::utils::date::format_iso_date;
use crate::utils::geocoding::geocode_address;

use crate::pages::location_details::ExpandedLocationDetails;

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

#[component]
pub fn LocationRow(
    loc: crate::data::location::Location,
    distance: f64,
    earliest_slot: Option<TimeSlot>,
    latest_slot: Option<TimeSlot>,
    is_loading: ReadSignal<bool>,
    alerts_enabled: bool,
    alert_config: Option<AlertConfig>,
    #[prop(into)] on_toggle_alert: Callback<bool>,
    #[prop(into)] on_toggle_month: Callback<u32>,
) -> impl IntoView {
    let (expanded, set_expanded) = create_signal(false);
    let (show_months, set_show_months) = create_signal(false);

    let toggle_expand = move |_| {
        set_expanded.update(|val| *val = !*val);
    };

    let total_tests = loc.passes + loc.failures;
    let low_data = total_tests < 1000;
    
    let is_alert_checked = alert_config.as_ref().map(|c| c.enabled).unwrap_or(false);
    let selected_months = alert_config.as_ref().map(|c| c.months.clone()).unwrap_or_default();

    view! {
        <>
            <tr class="hover:bg-gray-50 group transition-colors relative">
                // Alert checkbox column
                <td class="px-2 py-3 whitespace-nowrap text-center" on:click=|e| e.stop_propagation()>
                    {if alerts_enabled {
                        let months_clone = selected_months.clone();
                        view! {
                            <div class="relative inline-block">
                                <div class="flex items-center justify-center gap-1">
                                    <input
                                        type="checkbox"
                                        class="h-4 w-4 text-green-600 rounded border-gray-300 focus:ring-green-500 cursor-pointer"
                                        checked=is_alert_checked
                                        on:change=move |_| on_toggle_alert.run(!is_alert_checked)
                                    />
                                    {if is_alert_checked {
                                        view! {
                                            <button
                                                class="text-gray-400 hover:text-gray-600 p-0.5"
                                                title="Select months"
                                                on:click=move |e| {
                                                    e.stop_propagation();
                                                    set_show_months.update(|v| *v = !*v);
                                                }
                                            >
                                                <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
                                                    <path fill-rule="evenodd" d="M6 2a1 1 0 00-1 1v1H4a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V6a2 2 0 00-2-2h-1V3a1 1 0 10-2 0v1H7V3a1 1 0 00-1-1zm0 5a1 1 0 000 2h8a1 1 0 100-2H6z" clip-rule="evenodd" />
                                                </svg>
                                            </button>
                                        }.into_any()
                                    } else {
                                        view! { <span></span> }.into_any()
                                    }}
                                </div>
                                // Month dropdown
                                {move || if show_months.get() && is_alert_checked {
                                    let months_for_render = months_clone.clone();
                                    view! {
                                        <div class="absolute z-50 left-0 top-full mt-1 p-2 bg-white border border-gray-200 rounded-lg shadow-lg min-w-[180px]">
                                            <div class="text-xs font-medium text-gray-700 mb-2">Select months:</div>
                                            <div class="grid grid-cols-4 gap-1">
                                                {MONTHS.iter().enumerate().map(|(idx, month)| {
                                                    let month_num = (idx + 1) as u32;
                                                    let is_selected = months_for_render.contains(&month_num);
                                                    view! {
                                                        <button
                                                            class="px-1.5 py-1 text-xs rounded transition-colors"
                                                            class:bg-green-600=is_selected
                                                            class:text-white=is_selected
                                                            class:bg-gray-100=!is_selected
                                                            class:text-gray-600=!is_selected
                                                            class:hover:bg-green-500=is_selected
                                                            class:hover:bg-gray-200=!is_selected
                                                            on:click=move |e| {
                                                                e.stop_propagation();
                                                                on_toggle_month.run(month_num);
                                                            }
                                                        >
                                                            {*month}
                                                        </button>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                            <button
                                                class="mt-2 w-full text-xs text-gray-500 hover:text-gray-700"
                                                on:click=move |e| {
                                                    e.stop_propagation();
                                                    set_show_months(false);
                                                }
                                            >
                                                "Close"
                                            </button>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <span></span> }.into_any()
                                }}
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <span class="text-gray-300" title="Enable alerts first">
                                <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4 mx-auto" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9" />
                                </svg>
                            </span>
                        }.into_any()
                    }}
                </td>

                <td class="px-2 py-3 md:px-4 md:py-3 whitespace-nowrap text-sm font-medium text-gray-900 truncate cursor-pointer"
                    on:click=toggle_expand>
                    <div class="flex items-center gap-1">
                        <span>{loc.name.clone()}</span>
                        {if is_alert_checked && alerts_enabled {
                            let month_count = selected_months.len();
                            view! {
                                <span class="text-xs bg-green-100 text-green-700 px-1.5 py-0.5 rounded-full" title={format!("{} months selected", month_count)}>
                                    "🔔"
                                </span>
                            }.into_any()
                        } else {
                            view! { <span></span> }.into_any()
                        }}
                    </div>
                </td>

                <td class="px-1 py-3 md:px-3 md:py-3 whitespace-nowrap text-sm text-gray-500 cursor-pointer"
                    on:click=toggle_expand>
                    {format!("{:.1}", distance)}
                </td>

                <td class="px-1 py-3 md:px-3 md:py-3 whitespace-nowrap text-sm text-gray-500 cursor-pointer"
                    on:click=toggle_expand>
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

                <td class="px-1 py-3 md:px-3 md:py-3 whitespace-nowrap text-sm text-gray-500 cursor-pointer"
                    on:click=toggle_expand>
                    {match latest_slot {
                        Some(slot) => view! {
                            <span class="text-orange-600 font-medium">{slot.start_time}</span>
                        }.into_any(),
                        None => {
                            if is_loading.get_untracked() {
                                view! { <span class="text-gray-400">Loading...</span> }.into_any()
                            } else {
                                view! { <span class="text-gray-400">-</span> }.into_any()
                            }
                        }
                    }}
                </td>

                <td class="px-1 py-3 md:px-3 md:py-3 whitespace-nowrap text-sm text-gray-500 cursor-pointer"
                    on:click=toggle_expand>
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

                <td class="px-6 py-4 whitespace-nowrap text-sm text-center cursor-pointer"
                    on:click=toggle_expand>
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
