use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::{Duration, Utc};

use crate::models::{CallOption, MarketData, StockInformation};

#[derive(Debug, Clone, PartialEq)]
pub enum MarketDataError {
    InvalidTicker,
    TickerNotFound(String),
    InvalidData(String),
}

pub trait MarketDataProvider: Send + Sync {
    fn get_market_data(
        &self,
        ticker: &str,
    ) -> Pin<Box<dyn Future<Output = Result<MarketData, MarketDataError>> + Send + '_>>;
}

#[derive(Clone)]
pub struct RecommendationEngine {
    provider: Arc<dyn MarketDataProvider>,
}

impl RecommendationEngine {
    pub fn new(provider: Arc<dyn MarketDataProvider>) -> Self {
        Self { provider }
    }

    pub async fn get_market_data(&self, ticker: &str) -> Result<MarketData, MarketDataError> {
        self.provider.get_market_data(ticker).await
    }
}

#[derive(Debug, Default)]
pub struct MockMarketDataProvider;

impl MockMarketDataProvider {
    fn profile(ticker: &str) -> Option<(f64, [f64; 5])> {
        match ticker {
            "AAPL" => Some((227.65, [220.0, 225.0, 230.0, 235.0, 240.0])),
            "MSFT" => Some((510.20, [500.0, 505.0, 510.0, 515.0, 520.0])),
            "NVDA" => Some((182.40, [175.0, 180.0, 185.0, 190.0, 195.0])),
            "TSLA" => Some((442.80, [420.0, 430.0, 440.0, 450.0, 460.0])),
            _ => None,
        }
    }
}

impl MarketDataProvider for MockMarketDataProvider {
    fn get_market_data(
        &self,
        ticker: &str,
    ) -> Pin<Box<dyn Future<Output = Result<MarketData, MarketDataError>> + Send + '_>> {
        let ticker = ticker.to_string();
        Box::pin(async move {
            let ticker = ticker.trim().to_uppercase();
            if ticker.is_empty()
                || ticker.len() > 10
                || !ticker.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '.' | '-')
                })
            {
                return Err(MarketDataError::InvalidTicker);
            }

            let (current_price, strikes) = Self::profile(&ticker)
                .ok_or_else(|| MarketDataError::TickerNotFound(ticker.clone()))?;
            let now = Utc::now();
            let expirations = [21, 45, 75];
            let mut call_options = Vec::new();

            for (expiration_index, days) in expirations.into_iter().enumerate() {
                let expiration_date = (now + Duration::days(days)).format("%Y-%m-%d").to_string();
                for (strike_index, strike) in strikes.into_iter().enumerate() {
                    let moneyness = (strike - current_price) / current_price;
                    let delta = (0.72 - moneyness * 1.8 - expiration_index as f64 * 0.025)
                        .clamp(0.12, 0.82);
                    let time_value = 2.80 - moneyness * 10.0 - expiration_index as f64 * 0.18;
                    let mid = (time_value + (strike_index as f64 * 0.08)).max(0.35);
                    let bid = (mid - 0.08).max(0.01);
                    let ask = mid + 0.10;
                    let theta =
                        -(0.018 + expiration_index as f64 * 0.005 + strike_index as f64 * 0.002);
                    let implied_volatility =
                        (0.28 + (strike_index as f64 * 0.015) + (expiration_index as f64 * 0.018))
                            .min(0.60);

                    let option = CallOption {
                        strike_price: strike,
                        expiration_date: expiration_date.clone(),
                        bid_price: Some(bid),
                        ask_price: Some(ask),
                        delta: Some(delta),
                        theta: Some(theta),
                        implied_volatility: Some(implied_volatility),
                        contract_multiplier: 100,
                    };
                    if is_valid_call_option(&option) {
                        call_options.push(option);
                    }
                }
            }

            if call_options.is_empty() {
                return Err(MarketDataError::InvalidData(
                    "mock provider returned no valid call options".into(),
                ));
            }

            let data = MarketData {
                stock: StockInformation {
                    ticker,
                    current_price,
                    quote_timestamp: now.to_rfc3339(),
                    data_source: "SIMULATED_MOCK_DATA".into(),
                    disclaimer:
                        "Simulated quotes for development only. Not real market prices or trading advice."
                            .into(),
                },
                call_options,
            };
            validate_market_data(&data)?;
            Ok(data)
        })
    }
}

pub fn is_valid_call_option(option: &CallOption) -> bool {
    if !option.strike_price.is_finite() || option.strike_price <= 0.0 {
        return false;
    }
    if chrono::NaiveDate::parse_from_str(&option.expiration_date, "%Y-%m-%d").is_err() {
        return false;
    }
    if option.contract_multiplier <= 0 {
        return false;
    }

    let bid = match option.bid_price {
        Some(value) if value.is_finite() && value >= 0.0 => value,
        _ => return false,
    };
    let ask = match option.ask_price {
        Some(value) if value.is_finite() && value >= 0.0 => value,
        _ => return false,
    };
    if bid > ask {
        return false;
    }
    if let Some(delta) = option.delta
        && (!delta.is_finite() || !(0.0..=1.0).contains(&delta))
    {
        return false;
    }
    if let Some(theta) = option.theta
        && !theta.is_finite()
    {
        return false;
    }
    if let Some(implied_volatility) = option.implied_volatility
        && (!implied_volatility.is_finite() || implied_volatility < 0.0)
    {
        return false;
    }
    true
}

pub fn validate_market_data(data: &MarketData) -> Result<(), MarketDataError> {
    if !data.stock.current_price.is_finite() || data.stock.current_price <= 0.0 {
        return Err(MarketDataError::InvalidData(
            "stock quote is missing or invalid".into(),
        ));
    }
    if data.stock.quote_timestamp.is_empty()
        || chrono::DateTime::parse_from_rfc3339(&data.stock.quote_timestamp).is_err()
    {
        return Err(MarketDataError::InvalidData(
            "quote timestamp is missing or invalid".into(),
        ));
    }
    if data.call_options.is_empty() {
        return Err(MarketDataError::InvalidData(
            "option chain is missing".into(),
        ));
    }
    if !data.call_options.iter().all(is_valid_call_option) {
        return Err(MarketDataError::InvalidData(
            "option chain contains missing or invalid data".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_provider_returns_realistic_option_chain() {
        let provider = MockMarketDataProvider;
        let data = provider.get_market_data("AAPL").await.unwrap();

        assert_eq!(data.stock.ticker, "AAPL");
        assert_eq!(data.stock.data_source, "SIMULATED_MOCK_DATA");
        assert!(data.stock.current_price > 0.0);
        assert!(data.call_options.len() >= 10);
        assert!(
            data.call_options
                .windows(2)
                .any(|pair| pair[0].strike_price != pair[1].strike_price)
        );
        assert!(
            data.call_options
                .windows(2)
                .any(|pair| pair[0].expiration_date != pair[1].expiration_date)
        );
        assert!(validate_market_data(&data).is_ok());
    }

    #[tokio::test]
    async fn mock_provider_normalizes_ticker_and_handles_unknown_ticker() {
        let provider = MockMarketDataProvider;
        assert_eq!(
            provider
                .get_market_data(" msft ")
                .await
                .unwrap()
                .stock
                .ticker,
            "MSFT"
        );
        assert_eq!(
            provider.get_market_data("XYZ").await,
            Err(MarketDataError::TickerNotFound("XYZ".into()))
        );
        assert_eq!(
            provider.get_market_data("bad ticker").await,
            Err(MarketDataError::InvalidTicker)
        );
    }

    #[test]
    fn rejects_missing_or_invalid_option_data() {
        let mut option = CallOption {
            strike_price: 230.0,
            expiration_date: "2026-12-18".into(),
            bid_price: Some(2.40),
            ask_price: Some(2.50),
            delta: Some(0.45),
            theta: Some(-0.03),
            implied_volatility: Some(0.31),
            contract_multiplier: 100,
        };
        assert!(is_valid_call_option(&option));

        option.bid_price = None;
        assert!(!is_valid_call_option(&option));

        option.bid_price = Some(3.0);
        option.ask_price = Some(2.50);
        assert!(!is_valid_call_option(&option));
    }
}
