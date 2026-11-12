//! Errors returned to clients as JSON.

use astro_calc_core::{CalcError, EphemError};
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// An error with the HTTP status it maps to.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    /// A problem with the request's parameters (400).
    pub fn bad(message: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, message: message.into() }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self { status: StatusCode::NOT_FOUND, message: message.into() }
    }
}

impl From<CalcError> for ApiError {
    fn from(e: CalcError) -> Self {
        let status = match &e {
            // The date is outside the loaded ephemeris files, or the body's file is absent.
            CalcError::Ephem(EphemError::NoData { .. }) => StatusCode::UNPROCESSABLE_ENTITY,
            CalcError::Ephem(_) => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::BAD_REQUEST,
        };
        Self { status, message: e.to_string() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "valid": false, "error": self.message }))).into_response()
    }
}
