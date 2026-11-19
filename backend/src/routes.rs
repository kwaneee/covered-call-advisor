use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::{Value, json};

use crate::db::{
    DbPool, close_position, get_position, get_settings, upsert_position, upsert_settings,
};
use crate::market_data::{MarketDataError, RecommendationEngine};
use crate::models::{ActivePosition, MarketData, UserSettings};

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub recommendation_engine: RecommendationEngine,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/market-data/{ticker}", get(get_market_data_handler))
        .route(
            "/api/settings",
            get(get_settings_handler)
                .post(upsert_settings_handler)
                .put(upsert_settings_handler),
        )
        .route(
            "/api/position",
            get(get_position_handler)
                .post(upsert_position_handler)
                .put(upsert_position_handler)
                .delete(close_position_handler),
        )
        .with_state(Arc::new(state))
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn get_market_data_handler(
    State(state): State<Arc<AppState>>,
    Path(ticker): Path<String>,
) -> Result<Json<MarketData>, ApiError> {
    match state.recommendation_engine.get_market_data(&ticker).await {
        Ok(data) => Ok(Json(data)),
        Err(MarketDataError::InvalidTicker) => {
            Err(ApiError::bad_request("ticker contains invalid characters"))
        }
        Err(MarketDataError::TickerNotFound(ticker)) => Err(ApiError::not_found(format!(
            "simulated market data not found for {ticker}"
        ))),
        Err(MarketDataError::InvalidData(message)) => Err(ApiError::internal(format!(
            "invalid simulated market data: {message}"
        ))),
    }
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

async fn get_settings_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<UserSettings>, ApiError> {
    match get_settings(&state.pool).await {
        Ok(Some(settings)) => Ok(Json(settings)),
        Ok(None) => Err(ApiError::not_found("user settings not found")),
        Err(error) => Err(ApiError::internal(format!("database error: {error}"))),
    }
}

async fn upsert_settings_handler(
    State(state): State<Arc<AppState>>,
    Json(settings): Json<UserSettings>,
) -> Result<Json<UserSettings>, ApiError> {
    upsert_settings(&state.pool, &settings)
        .await
        .map(Json)
        .map_err(ApiError::bad_request)
}

async fn get_position_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ActivePosition>, ApiError> {
    match get_position(&state.pool).await {
        Ok(Some(position)) => Ok(Json(position)),
        Ok(None) => Err(ApiError::not_found("active position not found")),
        Err(error) => Err(ApiError::internal(format!("database error: {error}"))),
    }
}

async fn upsert_position_handler(
    State(state): State<Arc<AppState>>,
    Json(position): Json<ActivePosition>,
) -> Result<Json<ActivePosition>, ApiError> {
    upsert_position(&state.pool, &position)
        .await
        .map(Json)
        .map_err(|message| {
            if message.starts_with("database error") {
                ApiError::internal(message)
            } else {
                ApiError::bad_request(message)
            }
        })
}

async fn close_position_handler(
    State(state): State<Arc<AppState>>,
) -> Result<StatusCode, ApiError> {
    close_position(&state.pool)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|error| ApiError::internal(format!("database error: {error}")))
}
