
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
            <tr class="hover:bg-gray-50 group transition-colors cursor-pointer relative"
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
