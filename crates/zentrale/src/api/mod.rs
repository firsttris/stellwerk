//! The HTTP API the web UI uses. Everything but the session endpoints needs the admin.

mod error;
mod routes;
mod session;

use axum::Router;
use axum::routing::{get, patch, post};

pub use error::ApiError;

use crate::app::App;

pub fn router() -> Router<App> {
    Router::new()
        .route("/api/session", get(session::info))
        .route("/api/login", post(session::login))
        .route("/api/logout", post(session::logout))
        .route("/api/overview", get(routes::overview))
        .route("/api/folders", post(routes::create_folder))
        .route(
            "/api/folders/{id}",
            patch(routes::update_folder).delete(routes::delete_folder),
        )
        .route("/api/shares", post(routes::apply_shares))
        .route(
            "/api/devices/{id}",
            patch(routes::update_device).delete(routes::delete_device),
        )
        .route("/api/enrollments", post(routes::create_enrollment))
        .route("/api/enrollments/status", post(routes::enrollment_status))
        .route("/api/events", get(routes::events))
}
