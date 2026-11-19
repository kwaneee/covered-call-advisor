use crate::models::{ActivePosition, UserSettings};
use chrono::NaiveDate;

pub fn validate_settings(settings: &UserSettings) -> Result<(), String> {
    validate_ticker(&settings.stock_ticker)?;

    if settings.shares_owned < 0 {
        return Err("shares_owned must be zero or greater".into());
    }
    if settings.min_preferred_dte < 0 || settings.max_preferred_dte < 0 {
        return Err("preferred DTE values must be zero or greater".into());
    }
    if settings.min_preferred_dte > settings.max_preferred_dte {
        return Err("min_preferred_dte cannot exceed max_preferred_dte".into());
    }
    if !finite_and_in_range(settings.min_preferred_delta, 0.0, 1.0)
        || !finite_and_in_range(settings.max_preferred_delta, 0.0, 1.0)
    {
        return Err("preferred delta values must be between 0 and 1".into());
    }
    if settings.min_preferred_delta > settings.max_preferred_delta {
        return Err("min_preferred_delta cannot exceed max_preferred_delta".into());
    }
    validate_non_negative_finite(
        settings.min_acceptable_premium_per_share,
        "min_acceptable_premium_per_share",
    )?;
    if let Some(cost_basis) = settings.cost_basis_per_share {
        validate_non_negative_finite(cost_basis, "cost_basis_per_share")?;
    }
    validate_email(&settings.email_address)?;
    Ok(())
}

pub fn validate_position(position: &ActivePosition) -> Result<(), String> {
    validate_ticker(&position.stock_ticker)?;
    validate_non_negative_finite(position.strike_price, "strike_price")?;
    validate_non_negative_finite(
        position.premium_received_per_share,
        "premium_received_per_share",
    )?;
    if position.contracts <= 0 {
        return Err("contracts must be greater than zero".into());
    }
    if NaiveDate::parse_from_str(&position.expiration_date, "%Y-%m-%d").is_err() {
        return Err("expiration_date must use YYYY-MM-DD format".into());
    }
    if let Some(cost_basis) = position.cost_basis_per_share {
        validate_non_negative_finite(cost_basis, "cost_basis_per_share")?;
    }
    Ok(())
}

fn validate_ticker(ticker: &str) -> Result<(), String> {
    let ticker = ticker.trim();
    if ticker.is_empty() || ticker.len() > 10 {
        return Err("stock_ticker must contain between 1 and 10 characters".into());
    }
    if !ticker
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'))
    {
        return Err("stock_ticker contains unsupported characters".into());
    }
    Ok(())
}

fn validate_email(email: &str) -> Result<(), String> {
    let email = email.trim();
    if email.is_empty() || !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
        return Err("email_address must be a valid email address".into());
    }
    Ok(())
}

fn validate_non_negative_finite(value: f64, field: &str) -> Result<(), String> {
    if !value.is_finite() || value < 0.0 {
        return Err(format!("{field} must be zero or greater"));
    }
    Ok(())
}

fn finite_and_in_range(value: f64, minimum: f64, maximum: f64) -> bool {
    value.is_finite() && (minimum..=maximum).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> UserSettings {
        UserSettings {
            stock_ticker: "AAPL".into(),
            shares_owned: 300,
            min_preferred_dte: 21,
            max_preferred_dte: 45,
            min_preferred_delta: 0.20,
            max_preferred_delta: 0.35,
            min_acceptable_premium_per_share: 1.25,
            cost_basis_per_share: Some(150.0),
            email_address: "owner@example.com".into(),
        }
    }

    #[test]
    fn validates_preferred_ranges() {
        assert!(validate_settings(&settings()).is_ok());

        let mut invalid = settings();
        invalid.min_preferred_dte = 60;
        assert!(
            validate_settings(&invalid)
                .unwrap_err()
                .contains("cannot exceed")
        );

        invalid = settings();
        invalid.max_preferred_delta = 1.5;
        assert!(
            validate_settings(&invalid)
                .unwrap_err()
                .contains("between 0 and 1")
        );
    }

    #[test]
    fn validates_email_and_position_expiration() {
        let mut invalid = settings();
        invalid.email_address = "invalid-email".into();
        assert!(validate_settings(&invalid).is_err());

        let position = ActivePosition {
            stock_ticker: "AAPL".into(),
            strike_price: 180.0,
            expiration_date: "01/15/2027".into(),
            contracts: 1,
            premium_received_per_share: 2.50,
            cost_basis_per_share: None,
        };
        assert!(
            validate_position(&position)
                .unwrap_err()
                .contains("YYYY-MM-DD")
        );
    }
}
