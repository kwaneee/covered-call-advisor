use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::{Duration, NaiveDate, Utc};

use crate::models::{
    ActivePosition, ActivePositionMonitor, CallOption, CoveredCallRecommendation, MarketData,
    RecommendationResponse, RecommendationStatus, StockInformation, UserSettings,
};

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

    pub async fn recommend(
        &self,
        settings: &UserSettings,
        active_position: Option<&ActivePosition>,
    ) -> Result<RecommendationResponse, MarketDataError> {
        if let Some(position) = active_position {
            return Ok(evaluate_active_position(position, Utc::now()));
        }

        let market_data = self
            .provider
            .get_market_data(&settings.stock_ticker)
            .await?;
        Ok(evaluate_recommendations(settings, &market_data, Utc::now()))
    }
}

const MAX_STOCK_QUOTE_AGE_MINUTES: i64 = 15;

pub fn evaluate_active_position(
    position: &ActivePosition,
    as_of: chrono::DateTime<Utc>,
) -> RecommendationResponse {
    let dte = expiration_dte(&position.expiration_date, as_of).unwrap_or(-1);
    let profit = position.cost_basis_per_share.map(|cost_basis| {
        (position.strike_price - cost_basis + position.premium_received_per_share)
            * position.contracts as f64
            * 100.0
    });

    RecommendationResponse {
        status: RecommendationStatus::ActivePosition,
        recommendations: Vec::new(),
        active_position: Some(ActivePositionMonitor {
            position: position.clone(),
            dte,
            shares_covered: position.contracts.saturating_mul(100),
            potential_profit_if_called_away: profit,
        }),
        wait_reason: Some(
            "An active covered call position exists. It is being monitored; no new call will be recommended."
                .into(),
        ),
    }
}

pub fn evaluate_recommendations(
    settings: &UserSettings,
    market_data: &MarketData,
    as_of: chrono::DateTime<Utc>,
) -> RecommendationResponse {
    if !stock_quote_is_fresh(&market_data.stock, as_of) {
        return wait("Stock quote is missing, invalid, or stale.");
    }
    if market_data.call_options.is_empty() {
        return wait("No option quotes are available.");
    }

    let dte_midpoint =
        (settings.min_preferred_dte as f64 + settings.max_preferred_dte as f64) / 2.0;
    let dte_span = (settings.max_preferred_dte - settings.min_preferred_dte)
        .abs()
        .max(1) as f64;
    let delta_midpoint = (settings.min_preferred_delta + settings.max_preferred_delta) / 2.0;
    let delta_span = (settings.max_preferred_delta - settings.min_preferred_delta)
        .abs()
        .max(f64::EPSILON);

    let mut ranked: Vec<(f64, CoveredCallRecommendation)> = Vec::new();

    for option in &market_data.call_options {
        if !is_valid_call_option(option) {
            continue;
        }
        let Some(expiration) =
            chrono::NaiveDate::parse_from_str(&option.expiration_date, "%Y-%m-%d").ok()
        else {
            continue;
        };
        let dte = expiration_dte_from_date(expiration, as_of);
        if dte < settings.min_preferred_dte as i64 || dte > settings.max_preferred_dte as i64 {
            continue;
        }
        let Some(delta) = option.delta else {
            continue;
        };
        if delta < settings.min_preferred_delta || delta > settings.max_preferred_delta {
            continue;
        }

        let bid = option.bid_price.expect("validated bid");
        let ask = option.ask_price.expect("validated ask");
        let midpoint = (bid + ask) / 2.0;
        if midpoint < settings.min_acceptable_premium_per_share {
            continue;
        }

        let max_contracts = (settings.shares_owned / i64::from(option.contract_multiplier)).max(0);
        if max_contracts == 0 {
            continue;
        }

        let gross_premium = midpoint * f64::from(option.contract_multiplier);
        let potential_profit = settings.cost_basis_per_share.map(|cost_basis| {
            (option.strike_price - cost_basis + midpoint) * f64::from(option.contract_multiplier)
        });
        let dte_distance = ((dte as f64 - dte_midpoint).abs() / dte_span).max(0.0);
        let delta_distance = ((delta - delta_midpoint).abs() / delta_span).max(0.0);
        let score = dte_distance + delta_distance;
        let explanation = format!(
            "DTE {dte} is in the preferred range, delta {delta:.2} is in range, midpoint premium ${midpoint:.2}/share meets the minimum, and {max_contracts} contract(s) are fully covered."
        );
        ranked.push((
            score,
            CoveredCallRecommendation {
                strike: option.strike_price,
                expiration: option.expiration_date.clone(),
                dte,
                delta,
                bid,
                ask,
                midpoint_premium: midpoint,
                gross_premium_per_contract: gross_premium,
                potential_profit_if_called_away: potential_profit,
                contracts_recommended: max_contracts,
                contract_multiplier: option.contract_multiplier,
                explanation,
            },
        ));
    }

    if ranked.is_empty() {
        return wait(
            "No call options meet the DTE, delta, premium, and share-coverage requirements.",
        );
    }

    ranked.sort_by(|(score_a, option_a), (score_b, option_b)| {
        score_a
            .total_cmp(score_b)
            .then_with(|| {
                option_b
                    .gross_premium_per_contract
                    .total_cmp(&option_a.gross_premium_per_contract)
            })
            .then_with(|| option_a.strike.total_cmp(&option_b.strike))
    });

    RecommendationResponse {
        status: RecommendationStatus::Recommendations,
        recommendations: ranked.into_iter().map(|(_, option)| option).collect(),
        active_position: None,
        wait_reason: None,
    }
}

fn wait(reason: &str) -> RecommendationResponse {
    RecommendationResponse {
        status: RecommendationStatus::Wait,
        recommendations: Vec::new(),
        active_position: None,
        wait_reason: Some(reason.to_string()),
    }
}

fn expiration_dte(expiration: &str, as_of: chrono::DateTime<Utc>) -> Option<i64> {
    let date = NaiveDate::parse_from_str(expiration, "%Y-%m-%d").ok()?;
    Some(expiration_dte_from_date(date, as_of))
}

fn expiration_dte_from_date(expiration: NaiveDate, as_of: chrono::DateTime<Utc>) -> i64 {
    (expiration - as_of.date_naive()).num_days()
}

fn stock_quote_is_fresh(stock: &StockInformation, as_of: chrono::DateTime<Utc>) -> bool {
    if !stock.current_price.is_finite() || stock.current_price <= 0.0 {
        return false;
    }
    let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(&stock.quote_timestamp) else {
        return false;
    };
    let timestamp = timestamp.with_timezone(&Utc);
    let age = as_of.signed_duration_since(timestamp);
    age.num_seconds() >= 0 && age <= Duration::minutes(MAX_STOCK_QUOTE_AGE_MINUTES)
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
        Some(value) if value.is_finite() && value > 0.0 => value,
        _ => return false,
    };
    let ask = match option.ask_price {
        Some(value) if value.is_finite() && value > 0.0 => value,
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

    fn test_settings() -> UserSettings {
        UserSettings {
            stock_ticker: "AAPL".into(),
            shares_owned: 250,
            min_preferred_dte: 20,
            max_preferred_dte: 40,
            min_preferred_delta: 0.25,
            max_preferred_delta: 0.45,
            min_acceptable_premium_per_share: 2.0,
            cost_basis_per_share: Some(150.0),
            email_address: "owner@example.com".into(),
        }
    }

    fn test_as_of() -> chrono::DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-11-18T19:24:08Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn test_market(option_list: Vec<CallOption>) -> MarketData {
        MarketData {
            stock: StockInformation {
                ticker: "AAPL".into(),
                current_price: 220.0,
                quote_timestamp: "2026-11-18T19:20:08Z".into(),
                data_source: "TEST".into(),
                disclaimer: "test data".into(),
            },
            call_options: option_list,
        }
    }

    fn test_option(expiration: &str, strike: f64, bid: f64, ask: f64, delta: f64) -> CallOption {
        CallOption {
            strike_price: strike,
            expiration_date: expiration.into(),
            bid_price: Some(bid),
            ask_price: Some(ask),
            delta: Some(delta),
            theta: Some(-0.02),
            implied_volatility: Some(0.30),
            contract_multiplier: 100,
        }
    }

    #[test]
    fn filters_dte_delta_and_minimum_premium() {
        let settings = test_settings();
        let data = test_market(vec![
            test_option("2026-12-08", 230.0, 2.0, 2.20, 0.35),
            test_option("2027-01-30", 230.0, 3.0, 3.20, 0.35),
            test_option("2026-12-08", 230.0, 2.0, 2.20, 0.20),
            test_option("2026-12-08", 230.0, 1.0, 1.20, 0.35),
        ]);
        let response = evaluate_recommendations(&settings, &data, test_as_of());
        assert_eq!(response.status, RecommendationStatus::Recommendations);
        assert_eq!(response.recommendations.len(), 1);
        assert_eq!(response.recommendations[0].dte, 20);
    }

    #[test]
    fn rejects_contracts_not_fully_covered_and_calculates_premium() {
        let mut settings = test_settings();
        settings.shares_owned = 150;
        let data = test_market(vec![test_option("2026-12-08", 230.0, 2.40, 2.60, 0.35)]);
        let response = evaluate_recommendations(&settings, &data, test_as_of());
        assert_eq!(response.recommendations[0].contracts_recommended, 1);
        assert_eq!(response.recommendations[0].midpoint_premium, 2.50);
        assert_eq!(
            response.recommendations[0].gross_premium_per_contract,
            250.0
        );

        settings.shares_owned = 50;
        let response = evaluate_recommendations(&settings, &data, test_as_of());
        assert_eq!(response.status, RecommendationStatus::Wait);
    }

    #[test]
    fn uses_provider_contract_multiplier_for_coverage_and_premium() {
        let mut settings = test_settings();
        settings.shares_owned = 150;
        let mut option = test_option("2026-12-08", 230.0, 2.40, 2.60, 0.35);
        option.contract_multiplier = 50;
        let response =
            evaluate_recommendations(&settings, &test_market(vec![option]), test_as_of());
        assert_eq!(response.recommendations[0].contracts_recommended, 3);
        assert_eq!(
            response.recommendations[0].gross_premium_per_contract,
            125.0
        );
        assert_eq!(
            response.recommendations[0].potential_profit_if_called_away,
            Some(4125.0)
        );
    }

    #[test]
    fn calculates_potential_profit_only_with_cost_basis() {
        let settings = test_settings();
        let data = test_market(vec![test_option("2026-12-08", 230.0, 2.40, 2.60, 0.35)]);
        let response = evaluate_recommendations(&settings, &data, test_as_of());
        assert_eq!(
            response.recommendations[0].potential_profit_if_called_away,
            Some(8250.0)
        );

        let mut missing = settings;
        missing.cost_basis_per_share = None;
        let response = evaluate_recommendations(&missing, &data, test_as_of());
        assert_eq!(
            response.recommendations[0].potential_profit_if_called_away,
            None
        );
    }

    #[test]
    fn ranks_closest_preferences_first() {
        let settings = test_settings();
        let data = test_market(vec![
            test_option("2027-01-30", 230.0, 3.0, 3.20, 0.43),
            test_option("2026-12-18", 230.0, 3.0, 3.20, 0.35),
        ]);
        let response = evaluate_recommendations(&settings, &data, test_as_of());
        assert_eq!(response.recommendations[0].dte, 30);
        assert_eq!(response.recommendations[0].delta, 0.35);
    }

    #[test]
    fn returns_wait_when_nothing_qualifies() {
        let settings = test_settings();
        let data = test_market(vec![test_option("2026-11-20", 230.0, 1.0, 1.20, 0.10)]);
        let response = evaluate_recommendations(&settings, &data, test_as_of());
        assert_eq!(response.status, RecommendationStatus::Wait);
        assert!(response.wait_reason.is_some());
        assert!(response.recommendations.is_empty());
    }

    #[test]
    fn active_position_is_monitored_without_recommendations() {
        let position = ActivePosition {
            stock_ticker: "AAPL".into(),
            strike_price: 230.0,
            expiration_date: "2026-12-18".into(),
            contracts: 2,
            premium_received_per_share: 3.0,
            cost_basis_per_share: Some(150.0),
        };
        let response = evaluate_active_position(&position, test_as_of());
        assert_eq!(response.status, RecommendationStatus::ActivePosition);
        assert!(response.recommendations.is_empty());
        assert_eq!(response.active_position.unwrap().shares_covered, 200);
    }

    #[test]
    fn rejects_stale_missing_or_invalid_market_data() {
        let settings = test_settings();
        let mut data = test_market(vec![test_option("2026-12-08", 230.0, 2.0, 2.20, 0.35)]);
        data.stock.quote_timestamp = "2026-11-18T18:00:08Z".into();
        assert_eq!(
            evaluate_recommendations(&settings, &data, test_as_of()).status,
            RecommendationStatus::Wait
        );

        data.stock.quote_timestamp = "2026-11-18T19:20:08Z".into();
        data.call_options[0].bid_price = None;
        assert_eq!(
            evaluate_recommendations(&settings, &data, test_as_of()).status,
            RecommendationStatus::Wait
        );

        data.call_options.clear();
        assert_eq!(
            evaluate_recommendations(&settings, &data, test_as_of()).status,
            RecommendationStatus::Wait
        );
    }
}
