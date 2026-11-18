use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use eft_core::{validate_inventory, InventoryLayout, InventorySave};
use serde::Serialize;

use super::storage::{atomic_write, BackendState};

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Serialize)]
struct ValidationResponse {
    valid: bool,
    total_weight_grams: u64,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
struct SaveResponse {
    saved: bool,
    total_weight_grams: u64,
}

type ApiError = (StatusCode, Json<ErrorResponse>);

fn api_error(status: StatusCode, error: impl Into<String>) -> ApiError {
    (
        status,
        Json(ErrorResponse {
            error: error.into(),
        }),
    )
}

pub(super) fn app(state: BackendState) -> Router {
    Router::new()
        .route("/health", axum::routing::get(health))
        .route(
            "/inventory",
            axum::routing::get(get_inventory).put(save_inventory),
        )
        .route("/inventory/validate", post(validate))
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn validate(
    Json(layout): Json<InventoryLayout>,
) -> Result<Json<ValidationResponse>, ApiError> {
    validate_inventory(&layout)
        .map(|total_weight_grams| {
            Json(ValidationResponse {
                valid: true,
                total_weight_grams,
            })
        })
        .map_err(|error| api_error(StatusCode::UNPROCESSABLE_ENTITY, error.to_string()))
}

async fn get_inventory(
    State(state): State<BackendState>,
) -> Result<Json<InventoryLayout>, ApiError> {
    let _guard = state.file_lock.lock().await;
    let contents = match tokio::fs::read(state.inventory_file.as_path()).await {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(api_error(
                StatusCode::NOT_FOUND,
                "inventory not initialized",
            ));
        }
        Err(error) => {
            return Err(api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to read inventory: {error}"),
            ));
        }
    };

    let save = InventorySave::decode(&contents).map_err(|error| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("stored inventory is invalid JSON: {error}"),
        )
    })?;
    let layout = save.inventory;
    validate_inventory(&layout).map_err(|error| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("stored inventory failed validation: {error}"),
        )
    })?;

    Ok(Json(layout))
}

async fn save_inventory(
    State(state): State<BackendState>,
    Json(layout): Json<InventoryLayout>,
) -> Result<Json<SaveResponse>, ApiError> {
    let total_weight_grams = validate_inventory(&layout)
        .map_err(|error| api_error(StatusCode::UNPROCESSABLE_ENTITY, error.to_string()))?;
    let contents = InventorySave::new(layout.clone())
        .encode()
        .map_err(|error| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to serialize inventory: {error}"),
            )
        })?;

    let _guard = state.file_lock.lock().await;
    atomic_write(state.inventory_file.as_path(), &contents)
        .await
        .map_err(|error| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to save inventory: {error}"),
            )
        })?;

    Ok(Json(SaveResponse {
        saved: true,
        total_weight_grams,
    }))
}
