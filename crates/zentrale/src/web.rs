//! The web UI from `web/dist`, embedded in release builds. Unknown paths outside `/api` get
//! `index.html`, so the UI's own routes work on reload.

use axum::body::Body;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../web/dist"]
#[allow_missing = true]
struct Assets;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    match file(path).or_else(|| file("index.html")) {
        Some(response) => response,
        None => (
            StatusCode::NOT_FOUND,
            "The web UI is not built: run `pnpm --filter web build`, then build the Zentrale again.",
        )
            .into_response(),
    }
}

fn file(path: &str) -> Option<Response> {
    let asset = Assets::get(if path.is_empty() { "index.html" } else { path })?;
    let mime = asset.metadata.mimetype().to_string();
    // Vite puts a content hash into every file under assets/; index.html must always be fresh.
    let cache = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    Some(
        (
            [
                (header::CONTENT_TYPE, mime),
                (header::CACHE_CONTROL, cache.to_string()),
            ],
            Body::from(asset.data.into_owned()),
        )
            .into_response(),
    )
}
