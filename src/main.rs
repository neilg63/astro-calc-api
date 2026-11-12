use std::sync::Arc;

use astro_calc_api::config::Config;
use astro_calc_api::{AppState, router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load()?;
    let data_dir = config.data_dir.display();
    let state = AppState::load_with_table(&config.data_dir, &config.station_table)
        .map_err(|e| format!("cannot load ephemeris files from {data_dir}: {e}"))?;
    println!(
        "ephemeris: {} bodies from {data_dir}; station table: {}",
        state.eph.targets().len(),
        if state.stations.is_some() { "loaded" } else { "not found, stations are computed live" }
    );
    let listener = tokio::net::TcpListener::bind(config.address()).await?;
    println!("listening on http://{}", listener.local_addr()?);
    axum::serve(listener, router(Arc::new(state)))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
