//! REST API over `astro-calc-core`.
//!
//! Every endpoint is a `GET` that answers for one moment (`dt` or `jd`), optionally at one
//! place, and returns JSON. See the README for the parameters.

pub mod config;
pub mod error;
pub mod handlers;
pub mod params;

use std::path::Path;
use std::sync::Arc;

use astro_calc_core::{EphemError, Ephemeris, StationTable};
use axum::Router;
use axum::routing::get;

/// What the handlers share: the ephemeris and, if present, a pre-computed station table.
pub struct AppState {
    pub eph: Ephemeris,
    pub stations: Option<StationTable>,
}

impl AppState {
    /// Loads every `.bsp` file in `data_dir`, and `stations.bin` there if it was built from
    /// the same ephemeris.
    pub fn load(data_dir: impl AsRef<Path>) -> Result<Self, EphemError> {
        let table = data_dir.as_ref().join("stations.bin");
        Self::load_with_table(data_dir, table)
    }

    /// Loads every `.bsp` file in `data_dir` and the station table at `table`, which is
    /// ignored if it is missing or was built from a different ephemeris.
    pub fn load_with_table(data_dir: impl AsRef<Path>, table: impl AsRef<Path>) -> Result<Self, EphemError> {
        let eph = Ephemeris::open_dir(&data_dir)?;
        let stations = StationTable::open(table).ok().filter(|t| t.matches(&eph));
        Ok(Self { eph, stations })
    }
}

/// The application's routes.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(handlers::index))
        .route("/bodies", get(handlers::bodies))
        .route("/stars", get(handlers::stars))
        .route("/position", get(handlers::position))
        .route("/positions", get(handlers::positions))
        .route("/progress", get(handlers::progress))
        .route("/ascendant", get(handlers::ascendant))
        .route("/angles", get(handlers::angles))
        .route("/houses", get(handlers::houses))
        .route("/ayanamsha", get(handlers::ayanamsha))
        .route("/rise-set", get(handlers::rise_set))
        .route("/stations", get(handlers::stations))
        .route("/moon-phases", get(handlers::moon_phases))
        .route("/indian-day", get(handlers::indian_day))
        .route("/pheno", get(handlers::pheno))
        .route("/aspects", get(handlers::aspects))
        .fallback(handlers::not_found)
        .with_state(state)
}
