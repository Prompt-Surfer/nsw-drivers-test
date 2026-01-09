#![recursion_limit = "512"]
use axum::Router;
use leptos::prelude::*;
use leptos_axum::{generate_route_list, LeptosRoutes};
use nsw_closest_display::app::{shell, App};
use nsw_closest_display::data::booking::BookingManager;
use nsw_closest_display::settings::Settings;


#[tokio::main]
async fn main() {
    let conf = get_configuration(None).unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    let data_file_path = "data/bookings.json";
    match BookingManager::init_from_file(data_file_path) {
        Ok(_) => println!("BookingManager initialized from file"),
        Err(e) => println!("Failed to initialize BookingManager from file: {}", e),
    }

    // Initialize settings for later use (scraping started via UI)
    let _ = Settings::from_yaml("settings.yaml").unwrap();
    println!("INFO: Server started. Scraping can be triggered from the UI.");

    let app = Router::new()
        .route("/api/test_scrape", axum::routing::post(|| async {
            println!("INFO: Test scrape endpoint called");
            let settings = Settings::from_yaml("settings.yaml").unwrap();
            let locations = vec!["621".to_string(), "17".to_string(), "96".to_string()];
            tokio::spawn(async move {
                use nsw_closest_display::data::rta::scrape_rta_parallel;
                let result = scrape_rta_parallel(
                    locations,
                    &settings,
                    "data/test_bookings.json",
                    |booking, _| {
                        println!("INFO: Successfully scraped location: {}", booking.location);
                    }
                ).await;
                match result {
                    Ok(_) => println!("INFO: Test scrape completed successfully"),
                    Err(e) => println!("ERROR: Test scrape failed: {:?}", e),
                }
            });
            "Test scrape started"
        }))
        .route("/api/*path", axum::routing::any(|req: axum::http::Request<axum::body::Body>| async {
            println!("DEBUG: Server function called: {:?}", req.uri().path());
            leptos_axum::handle_server_fns(req).await
        }))
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("listening on http://{}", &addr);
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for a purely client-side app
    // see lib.rs for hydration function instead
}
