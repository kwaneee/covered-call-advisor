use std::env;
use std::time::Duration as StdDuration;

use chrono::{Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc, Weekday};
use chrono_tz::America::Toronto;
use lettre::{
    Address, Message, SmtpTransport, Transport, message::Mailbox,
    transport::smtp::authentication::Credentials,
};

use crate::db::{DbPool, get_position, get_settings, record_notification, was_notification_sent};
use crate::market_data::{
    MarketDataError, RecommendationEngine, stock_quote_is_fresh, validate_market_data,
};
use crate::models::{RecommendationResponse, RecommendationStatus, UserSettings};

pub fn is_toronto_trading_day(date: NaiveDate) -> bool {
    if matches!(date.weekday(), Weekday::Sat | Weekday::Sun) {
        return false;
    }
    if us_market_holiday(date) {
        return false;
    }
    true
}

fn us_market_holiday(date: NaiveDate) -> bool {
    let year = date.year();
    let month = date.month();
    if observed_fixed(year, 1, 1) == date
        || observed_fixed(year, 6, 19) == date
        || observed_fixed(year, 7, 4) == date
        || observed_fixed(year, 12, 25) == date
    {
        return true;
    }

    if date == nth_weekday(year, 1, 1, 3)
        || date == nth_weekday(year, 2, 1, 3)
        || date == nth_weekday(year, 5, 1, 5)
        || date == nth_weekday(year, 9, 1, 1)
        || date == nth_weekday(year, 11, 4, 4)
    {
        return true;
    }

    if month >= 3 && month <= 4 {
        let easter = easter_sunday(year);
        if date == easter - Duration::days(2) {
            return true;
        }
    }
    false
}

fn easter_sunday(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year.div_euclid(100);
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32).expect("valid Easter date")
}

fn nth_weekday(year: i32, month: u32, weekday: u32, occurrence: u32) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("valid date");
    let offset = (weekday + 7 - first.weekday().num_days_from_sunday()) % 7;
    first + Duration::days((offset + 7 * (occurrence - 1)) as i64)
}

fn observed_fixed(year: i32, month: u32, day: u32) -> NaiveDate {
    let date = NaiveDate::from_ymd_opt(year, month, day).expect("valid date");
    match date.weekday() {
        Weekday::Sat => date - Duration::days(1),
        Weekday::Sun => date + Duration::days(1),
        _ => date,
    }
}

pub async fn scheduled_tick(pool: &DbPool, engine: &RecommendationEngine) {
    let now = Utc::now().with_timezone(&Toronto);
    let date = now.date_naive();
    if now.hour() != 10 || now.minute() > 1 {
        return;
    }
    if !is_toronto_trading_day(date) {
        return;
    }
    let date_key = date.format("%Y-%m-%d").to_string();
    match was_notification_sent(pool, &date_key).await {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => {
            eprintln!("Scheduled notification dedupe check failed: {error}");
            return;
        }
    }

    match run_notification_process(pool, engine, false).await {
        Ok(_) => {
            if let Err(error) = record_notification(pool, &date_key).await {
                eprintln!("Could not record sent notification for {date_key}: {error}");
            }
        }
        Err(error) => eprintln!("Scheduled notification failed: {error}"),
    }
}

pub async fn run_notification_process(
    pool: &DbPool,
    engine: &RecommendationEngine,
    is_manual: bool,
) -> Result<String, String> {
    let settings = get_settings(pool)
        .await
        .map_err(|error| format!("database error: {error}"))?
        .ok_or_else(|| "user settings are not configured".to_string())?;

    let market_data = engine
        .get_market_data(&settings.stock_ticker)
        .await
        .map_err(market_data_error_message)?;
    validate_market_data(&market_data).map_err(|error| market_data_error_message(error))?;

    let as_of = Utc::now();
    if !stock_quote_is_fresh(&market_data.stock, as_of) {
        let subject = "Covered Call Advisor - Data unavailable";
        let body = format!(
            "DATA UNAVAILABLE\n\nThe stock quote for {} is missing or stale. No recommendation or WAIT status was generated.\n\nQuote timestamp: {}\n",
            settings.stock_ticker, market_data.stock.quote_timestamp
        );
        send_email(&settings, subject, &body, &market_data.stock, is_manual).await?;
        return Ok(subject.to_string());
    }

    let position = get_position(pool)
        .await
        .map_err(|error| format!("database error: {error}"))?;

    let response = engine
        .recommend(&settings, position.as_ref())
        .await
        .map_err(market_data_error_message)?;
    let (subject, body) = format_notification(
        &settings,
        &market_data.stock.current_price,
        &response,
        &market_data.stock,
    );
    send_email(&settings, &subject, &body, &market_data.stock, is_manual).await?;
    Ok(subject)
}

fn market_data_error_message(error: MarketDataError) -> String {
    match error {
        MarketDataError::InvalidTicker => "saved stock ticker is invalid".to_string(),
        MarketDataError::TickerNotFound(ticker) => format!("market data not found for {ticker}"),
        MarketDataError::InvalidData(message) => format!("invalid market data: {message}"),
    }
}

fn format_notification(
    settings: &UserSettings,
    current_price: &f64,
    response: &RecommendationResponse,
    stock: &crate::models::StockInformation,
) -> (String, String) {
    let simulated = is_simulated(stock);
    let mut body = String::new();
    if simulated {
        body.push_str("SIMULATED EMAIL - THE BACKEND IS USING MOCK MARKET DATA\n\n");
    }

    match response.status {
        RecommendationStatus::ActivePosition => {
            let position = response.active_position.as_ref().expect("active position");
            body.push_str(&format!(
                "ACTIVE COVERED CALL UPDATE\n\nTicker: {}\nCurrent stock price: ${:.2}\nStrike: ${:.2}\nExpiration: {}\nDTE: {}\nContracts: {}\nShares covered: {}\nPremium received: ${:.2}/share\n",
                settings.stock_ticker,
                current_price,
                position.position.strike_price,
                position.position.expiration_date,
                position.dte,
                position.position.contracts,
                position.shares_covered,
                position.position.premium_received_per_share
            ));
            if let Some(profit) = position.potential_profit_if_called_away {
                body.push_str(&format!(
                    "Potential profit if called away: ${:.2}\n",
                    profit
                ));
            }
            (
                format!(
                    "Covered Call Advisor - Active position update: {}",
                    settings.stock_ticker
                ),
                body,
            )
        }
        RecommendationStatus::Recommendations => {
            body.push_str(&format!(
                "RECOMMENDATION\n\nTicker: {}\nCurrent stock price: ${:.2}\n",
                settings.stock_ticker, current_price
            ));
            for recommendation in &response.recommendations {
                body.push_str(&format!(
                    "\nStrike: ${:.2}\nExpiration: {}\nDTE: {}\nDelta: {:.2}\nBid: ${:.2}\nAsk: ${:.2}\nMidpoint premium: ${:.2}/share\nGross premium per contract: ${:.2}\n",
                    recommendation.strike,
                    recommendation.expiration,
                    recommendation.dte,
                    recommendation.delta,
                    recommendation.bid,
                    recommendation.ask,
                    recommendation.midpoint_premium,
                    recommendation.gross_premium_per_contract
                ));
                if let Some(profit) = recommendation.potential_profit_if_called_away {
                    body.push_str(&format!(
                        "Potential profit if called away: ${:.2}\n",
                        profit
                    ));
                }
                body.push_str(&format!("Why: {}\n", recommendation.explanation));
            }
            (
                format!(
                    "Covered Call Advisor - Recommendation: {}",
                    settings.stock_ticker
                ),
                body,
            )
        }
        RecommendationStatus::Wait => {
            body.push_str(&format!(
                "WAIT\n\nTicker: {}\nCurrent stock price: ${:.2}\n\n{}\n",
                settings.stock_ticker,
                current_price,
                response
                    .wait_reason
                    .as_deref()
                    .unwrap_or("No options currently qualify.")
            ));
            (
                format!("Covered Call Advisor - WAIT: {}", settings.stock_ticker),
                body,
            )
        }
    }
}

fn is_simulated(stock: &crate::models::StockInformation) -> bool {
    stock.data_source.to_ascii_uppercase().contains("SIMULATED")
        || stock.data_source.to_ascii_uppercase().contains("MOCK")
        || stock.disclaimer.to_ascii_uppercase().contains("SIMULATED")
}

async fn send_email(
    settings: &UserSettings,
    subject: &str,
    body: &str,
    stock: &crate::models::StockInformation,
    is_manual: bool,
) -> Result<(), String> {
    let server = env::var("SMTP_SERVER").map_err(|_| "SMTP_SERVER is not configured")?;
    let port = env::var("SMTP_PORT")
        .unwrap_or_else(|_| "587".to_string())
        .parse::<u16>()
        .map_err(|_| "SMTP_PORT must be a valid port")?;
    let username = env::var("SMTP_USERNAME").map_err(|_| "SMTP_USERNAME is not configured")?;
    let password = env::var("SMTP_PASSWORD").map_err(|_| "SMTP_PASSWORD is not configured")?;
    let from = env::var("SMTP_FROM").map_err(|_| "SMTP_FROM is not configured")?;

    let from_address: Address = from
        .parse()
        .map_err(|error| format!("SMTP_FROM is invalid: {error}"))?;
    let to_address: Address = settings
        .email_address
        .parse()
        .map_err(|error| format!("notification email is invalid: {error}"))?;
    let body = if is_manual {
        format!("MANUAL TEST EMAIL\n\n{body}")
    } else {
        body.to_string()
    };

    let message = Message::builder()
        .from(Mailbox::new(None, from_address))
        .to(Mailbox::new(None, to_address))
        .subject(subject)
        .body(body)
        .map_err(|error| format!("could not build email: {error}"))?;

    let transport = SmtpTransport::starttls_relay(&server)
        .map_err(|error| format!("could not configure SMTP STARTTLS relay: {error}"))?
        .port(port)
        .timeout(Some(StdDuration::from_secs(30)))
        .credentials(Credentials::new(username, password))
        .build();
    transport
        .send(&message)
        .map_err(|error| format!("could not send email: {error}"))?;

    let _ = stock;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekends_and_exchange_holidays_are_not_trading_days() {
        assert!(!is_toronto_trading_day(
            NaiveDate::from_ymd_opt(2026, 11, 21).unwrap()
        ));
        assert!(!is_toronto_trading_day(
            NaiveDate::from_ymd_opt(2026, 12, 25).unwrap()
        ));
        assert!(!is_toronto_trading_day(
            NaiveDate::from_ymd_opt(2026, 7, 3).unwrap()
        ));
        assert!(is_toronto_trading_day(
            NaiveDate::from_ymd_opt(2026, 11, 18).unwrap()
        ));
    }

    #[test]
    fn formats_wait_and_marks_mock_email() {
        let settings = UserSettings {
            stock_ticker: "AAPL".into(),
            shares_owned: 100,
            min_preferred_dte: 21,
            max_preferred_dte: 45,
            min_preferred_delta: 0.20,
            max_preferred_delta: 0.35,
            min_acceptable_premium_per_share: 1.25,
            cost_basis_per_share: None,
            email_address: "owner@example.com".into(),
        };
        let response = RecommendationResponse {
            status: RecommendationStatus::Wait,
            recommendations: Vec::new(),
            active_position: None,
            wait_reason: Some("No options qualify.".into()),
        };
        let stock = crate::models::StockInformation {
            ticker: "AAPL".into(),
            current_price: 227.65,
            quote_timestamp: Utc::now().to_rfc3339(),
            data_source: "SIMULATED_MOCK_DATA".into(),
            disclaimer: "Simulated quotes".into(),
        };
        let (_, body) = format_notification(&settings, &227.65, &response, &stock);
        assert!(body.contains("SIMULATED EMAIL"));
        assert!(body.contains("WAIT"));
    }

    #[test]
    fn toronto_schedule_handles_daylight_saving_time() {
        let summer = Utc
            .with_ymd_and_hms(2026, 7, 15, 14, 0, 0)
            .single()
            .unwrap()
            .with_timezone(&Toronto);
        let winter = Utc
            .with_ymd_and_hms(2026, 11, 18, 15, 0, 0)
            .single()
            .unwrap()
            .with_timezone(&Toronto);
        assert_eq!((summer.hour(), summer.minute()), (10, 0));
        assert_eq!((winter.hour(), winter.minute()), (10, 0));
    }
}
