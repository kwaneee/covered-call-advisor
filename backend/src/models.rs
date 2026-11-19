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
