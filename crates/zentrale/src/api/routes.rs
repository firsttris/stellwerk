//! The UI's API: overview, folders, matrix drafts, devices, pairing codes, activity.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use super::error::{ApiError, ApiResult};
use super::session::Admin;
use crate::app::App;
use crate::model::{Device, DeviceKind, Folder, Share};
use crate::pairing::{CODE_LIFETIME_SECS, code_hash, fingerprint, new_code};
use crate::store::{DeviceUpdate, Event, ShareChange, now};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    #[serde(flatten)]
    device: Device,
    online: bool,
    online_since: Option<i64>,
    fingerprint: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HubView {
    syncthing_id: String,
    address: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    revision: u64,
    hub: HubView,
    devices: Vec<DeviceView>,
    folders: Vec<Folder>,
    shares: Vec<Share>,
}

fn device_view(app: &App, device: Device) -> DeviceView {
    let online_since = app.online_devices().get(&device.id).copied();
    DeviceView {
        fingerprint: fingerprint(&device.public_key),
        online: online_since.is_some(),
        online_since,
        device,
    }
}

pub async fn overview(State(app): State<App>, _: Admin) -> ApiResult<Json<Overview>> {
    let config = app.store().config().await?;
    let hub = app.hub();
    Ok(Json(Overview {
        revision: config.revision,
        hub: HubView {
            syncthing_id: hub.syncthing_id.clone(),
            address: hub.address.clone(),
        },
        devices: config
            .devices
            .into_iter()
            .map(|device| device_view(&app, device))
            .collect(),
        folders: config.folders,
        shares: config.shares,
    }))
}

#[derive(Debug, Deserialize)]
pub struct NewFolder {
    label: String,
}

pub async fn create_folder(
    State(app): State<App>,
    _: Admin,
    Json(body): Json<NewFolder>,
) -> ApiResult<(StatusCode, Json<Folder>)> {
    let folder = app.store().create_folder(&body.label, now()).await?;
    app.changed().await?;
    Ok((StatusCode::CREATED, Json(folder)))
}

pub async fn rename_folder(
    State(app): State<App>,
    _: Admin,
    Path(id): Path<String>,
    Json(body): Json<NewFolder>,
) -> ApiResult<StatusCode> {
    app.store().rename_folder(&id, &body.label).await?;
    app.changed().await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_folder(
    State(app): State<App>,
    _: Admin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    app.store().delete_folder(&id).await?;
    app.changed().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct Draft {
    changes: Vec<ShareChange>,
}

#[derive(Debug, Serialize)]
pub struct Revision {
    revision: u64,
}

/// "Apply" in the matrix: all changes of the draft as one revision.
pub async fn apply_shares(
    State(app): State<App>,
    _: Admin,
    Json(draft): Json<Draft>,
) -> ApiResult<Json<Revision>> {
    let revision = app.store().apply_shares(&draft.changes).await?;
    app.changed().await?;
    Ok(Json(Revision { revision }))
}

#[derive(Debug, Deserialize)]
pub struct DevicePatch {
    name: Option<String>,
    kind: Option<DeviceKind>,
    approved: Option<bool>,
}

pub async fn update_device(
    State(app): State<App>,
    _: Admin,
    Path(id): Path<String>,
    Json(patch): Json<DevicePatch>,
) -> ApiResult<Json<DeviceView>> {
    if patch
        .name
        .as_deref()
        .is_some_and(|name| name.trim().is_empty())
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "a device needs a name",
        ));
    }
    let update = DeviceUpdate {
        name: patch.name.map(|name| name.trim().to_string()),
        kind: patch.kind,
        approved: patch.approved,
    };
    let device = app.store().update_device(&id, &update).await?;
    app.changed().await?;
    Ok(Json(device_view(&app, device)))
}

pub async fn delete_device(
    State(app): State<App>,
    _: Admin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    app.store().delete_device(&id).await?;
    app.changed().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Enrollment {
    code: String,
    expires_at: i64,
}

/// "Pair device": a new one-time code. Only its hash is stored.
pub async fn create_enrollment(
    State(app): State<App>,
    _: Admin,
) -> ApiResult<(StatusCode, Json<Enrollment>)> {
    let code = new_code();
    let expires_at = now() + CODE_LIFETIME_SECS;
    app.store()
        .create_enrollment(&code_hash(&code), expires_at)
        .await?;
    Ok((StatusCode::CREATED, Json(Enrollment { code, expires_at })))
}

#[derive(Debug, Deserialize)]
pub struct EnrollmentQuery {
    code: String,
}

#[derive(Debug, Serialize)]
pub struct EnrollmentStatus {
    device: Option<DeviceView>,
}

/// Whether a device has used the code yet; the pairing dialog polls this. A POST, so the
/// code does not end up in access logs.
pub async fn enrollment_status(
    State(app): State<App>,
    _: Admin,
    Json(query): Json<EnrollmentQuery>,
) -> ApiResult<Json<EnrollmentStatus>> {
    let device = match app.store().enrolled_device(&code_hash(&query.code)).await? {
        Some(id) => Some(device_view(&app, app.store().device(&id).await?)),
        None => None,
    };
    Ok(Json(EnrollmentStatus { device }))
}

#[derive(Debug, Deserialize)]
pub struct EventQuery {
    limit: Option<u32>,
}

pub async fn events(
    State(app): State<App>,
    _: Admin,
    Query(query): Query<EventQuery>,
) -> ApiResult<Json<Vec<Event>>> {
    let limit = query.limit.unwrap_or(50).min(500);
    Ok(Json(app.store().events(limit).await?))
}
