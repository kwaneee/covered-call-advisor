use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
pub struct UserSettings {
    pub stock_ticker: String,
    pub shares_owned: i64,
    pub min_preferred_dte: i32,
    pub max_preferred_dte: i32,
    pub min_preferred_delta: f64,
    pub max_preferred_delta: f64,
    pub min_acceptable_premium_per_share: f64,
    pub cost_basis_per_share: Option<f64>,
    pub email_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
pub struct ActivePosition {
    pub stock_ticker: String,
    pub strike_price: f64,
    pub expiration_date: String,
    pub contracts: i64,
    pub premium_received_per_share: f64,
    pub cost_basis_per_share: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StockInformation {
    pub ticker: String,
    pub current_price: f64,
    pub quote_timestamp: String,
    pub data_source: String,
    pub disclaimer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallOption {
    pub strike_price: f64,
    pub expiration_date: String,
    pub bid_price: Option<f64>,
    pub ask_price: Option<f64>,
    pub delta: Option<f64>,
    pub theta: Option<f64>,
    pub implied_volatility: Option<f64>,
    pub contract_multiplier: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketData {
    pub stock: StockInformation,
    pub call_options: Vec<CallOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoveredCallRecommendation {
    pub strike: f64,
    pub expiration: String,
    pub dte: i64,
    pub delta: f64,
    pub bid: f64,
    pub ask: f64,
    pub midpoint_premium: f64,
    pub gross_premium_per_contract: f64,
    pub potential_profit_if_called_away: Option<f64>,
    pub contracts_recommended: i64,
    pub contract_multiplier: i32,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActivePositionMonitor {
    pub position: ActivePosition,
    pub dte: i64,
    pub shares_covered: i64,
    pub potential_profit_if_called_away: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecommendationStatus {
    Recommendations,
    Wait,
    ActivePosition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecommendationResponse {
    pub status: RecommendationStatus,
    pub recommendations: Vec<CoveredCallRecommendation>,
    pub active_position: Option<ActivePositionMonitor>,
    pub wait_reason: Option<String>,
}
