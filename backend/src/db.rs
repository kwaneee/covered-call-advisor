use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};

use crate::models::{ActivePosition, UserSettings};
use crate::validation::{validate_position, validate_settings};

pub type DbPool = Pool<Sqlite>;

pub async fn connect(database_url: &str) -> Result<DbPool, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;
    initialize_schema(&pool).await?;
    Ok(pool)
}

pub async fn initialize_schema(pool: &DbPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS user_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            stock_ticker TEXT NOT NULL,
            shares_owned INTEGER NOT NULL CHECK (shares_owned >= 0),
            min_preferred_dte INTEGER NOT NULL CHECK (min_preferred_dte >= 0),
            max_preferred_dte INTEGER NOT NULL CHECK (max_preferred_dte >= 0),
            min_preferred_delta REAL NOT NULL,
            max_preferred_delta REAL NOT NULL,
            min_acceptable_premium_per_share REAL NOT NULL CHECK (min_acceptable_premium_per_share >= 0),
            cost_basis_per_share REAL,
            email_address TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS active_position (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            stock_ticker TEXT NOT NULL,
            strike_price REAL NOT NULL CHECK (strike_price >= 0),
            expiration_date TEXT NOT NULL,
            contracts INTEGER NOT NULL CHECK (contracts > 0),
            premium_received_per_share REAL NOT NULL CHECK (premium_received_per_share >= 0),
            cost_basis_per_share REAL
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS notification_log (
            notification_date TEXT PRIMARY KEY
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_settings(pool: &DbPool) -> Result<Option<UserSettings>, sqlx::Error> {
    sqlx::query_as::<_, UserSettings>(
        r#"
        SELECT stock_ticker, shares_owned, min_preferred_dte, max_preferred_dte,
               min_preferred_delta, max_preferred_delta,
               min_acceptable_premium_per_share, cost_basis_per_share, email_address
        FROM user_settings
        WHERE id = 1
        "#,
    )
    .fetch_optional(pool)
    .await
}

pub async fn upsert_settings(
    pool: &DbPool,
    settings: &UserSettings,
) -> Result<UserSettings, String> {
    validate_settings(settings)?;

    let normalized = normalize_settings(settings);
    sqlx::query(
        r#"
        INSERT INTO user_settings (
            id, stock_ticker, shares_owned, min_preferred_dte, max_preferred_dte,
            min_preferred_delta, max_preferred_delta, min_acceptable_premium_per_share,
            cost_basis_per_share, email_address
        ) VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            stock_ticker = excluded.stock_ticker,
            shares_owned = excluded.shares_owned,
            min_preferred_dte = excluded.min_preferred_dte,
            max_preferred_dte = excluded.max_preferred_dte,
            min_preferred_delta = excluded.min_preferred_delta,
            max_preferred_delta = excluded.max_preferred_delta,
            min_acceptable_premium_per_share = excluded.min_acceptable_premium_per_share,
            cost_basis_per_share = excluded.cost_basis_per_share,
            email_address = excluded.email_address
        "#,
    )
    .bind(&normalized.stock_ticker)
    .bind(normalized.shares_owned)
    .bind(normalized.min_preferred_dte)
    .bind(normalized.max_preferred_dte)
    .bind(normalized.min_preferred_delta)
    .bind(normalized.max_preferred_delta)
    .bind(normalized.min_acceptable_premium_per_share)
    .bind(normalized.cost_basis_per_share)
    .bind(&normalized.email_address)
    .execute(pool)
    .await
    .map_err(|error| format!("database error: {error}"))?;

    Ok(normalized)
}

pub async fn get_position(pool: &DbPool) -> Result<Option<ActivePosition>, sqlx::Error> {
    sqlx::query_as::<_, ActivePosition>(
        r#"
        SELECT stock_ticker, strike_price, expiration_date, contracts,
               premium_received_per_share, cost_basis_per_share
        FROM active_position
        WHERE id = 1
        "#,
    )
    .fetch_optional(pool)
    .await
}

pub async fn upsert_position(
    pool: &DbPool,
    position: &ActivePosition,
) -> Result<ActivePosition, String> {
    validate_position(position)?;

    let settings = get_settings(pool)
        .await
        .map_err(|error| format!("database error: {error}"))?
        .ok_or_else(|| "user settings must be configured before creating a position".to_string())?;

    if position
        .stock_ticker
        .trim()
        .eq_ignore_ascii_case(&settings.stock_ticker)
        == false
    {
        return Err("position stock_ticker must match user settings stock_ticker".into());
    }
    let covered_shares = position
        .contracts
        .checked_mul(100)
        .ok_or_else(|| "contract count is too large".to_string())?;
    if covered_shares > settings.shares_owned {
        return Err(format!(
            "position requires {covered_shares} shares, but only {} shares are owned",
            settings.shares_owned
        ));
    }

    let normalized = normalize_position(position);
    sqlx::query(
        r#"
        INSERT INTO active_position (
            id, stock_ticker, strike_price, expiration_date, contracts,
            premium_received_per_share, cost_basis_per_share
        ) VALUES (1, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            stock_ticker = excluded.stock_ticker,
            strike_price = excluded.strike_price,
            expiration_date = excluded.expiration_date,
            contracts = excluded.contracts,
            premium_received_per_share = excluded.premium_received_per_share,
            cost_basis_per_share = excluded.cost_basis_per_share
        "#,
    )
    .bind(&normalized.stock_ticker)
    .bind(normalized.strike_price)
    .bind(&normalized.expiration_date)
    .bind(normalized.contracts)
    .bind(normalized.premium_received_per_share)
    .bind(normalized.cost_basis_per_share)
    .execute(pool)
    .await
    .map_err(|error| format!("database error: {error}"))?;

    Ok(normalized)
}

pub async fn close_position(pool: &DbPool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM active_position WHERE id = 1")
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn was_notification_sent(pool: &DbPool, date: &str) -> Result<bool, sqlx::Error> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM notification_log WHERE notification_date = ?")
            .bind(date)
            .fetch_one(pool)
            .await?;
    Ok(count > 0)
}

pub async fn record_notification(pool: &DbPool, date: &str) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT OR IGNORE INTO notification_log (notification_date) VALUES (?)")
        .bind(date)
        .execute(pool)
        .await?;
    Ok(())
}

fn normalize_settings(settings: &UserSettings) -> UserSettings {
    UserSettings {
        stock_ticker: settings.stock_ticker.trim().to_uppercase(),
        shares_owned: settings.shares_owned,
        min_preferred_dte: settings.min_preferred_dte,
        max_preferred_dte: settings.max_preferred_dte,
        min_preferred_delta: settings.min_preferred_delta,
        max_preferred_delta: settings.max_preferred_delta,
        min_acceptable_premium_per_share: settings.min_acceptable_premium_per_share,
        cost_basis_per_share: settings.cost_basis_per_share,
        email_address: settings.email_address.trim().to_string(),
    }
}

fn normalize_position(position: &ActivePosition) -> ActivePosition {
    ActivePosition {
        stock_ticker: position.stock_ticker.trim().to_uppercase(),
        strike_price: position.strike_price,
        expiration_date: position.expiration_date.trim().to_string(),
        contracts: position.contracts,
        premium_received_per_share: position.premium_received_per_share,
        cost_basis_per_share: position.cost_basis_per_share,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> DbPool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("test database should connect");
        initialize_schema(&pool)
            .await
            .expect("test schema should initialize");
        pool
    }

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

    fn position() -> ActivePosition {
        ActivePosition {
            stock_ticker: "AAPL".into(),
            strike_price: 180.0,
            expiration_date: "2027-01-15".into(),
            contracts: 2,
            premium_received_per_share: 2.50,
            cost_basis_per_share: Some(150.0),
        }
    }

    #[tokio::test]
    async fn settings_persist_and_upsert() {
        let pool = test_pool().await;
        assert!(get_settings(&pool).await.unwrap().is_none());

        let saved = upsert_settings(&pool, &settings()).await.unwrap();
        assert_eq!(saved.stock_ticker, "AAPL");
        assert_eq!(get_settings(&pool).await.unwrap(), Some(saved.clone()));

        let mut changed = saved;
        changed.shares_owned = 500;
        upsert_settings(&pool, &changed).await.unwrap();
        assert_eq!(
            get_settings(&pool).await.unwrap().unwrap().shares_owned,
            500
        );
    }

    #[tokio::test]
    async fn position_requires_covered_shares() {
        let pool = test_pool().await;
        upsert_settings(&pool, &settings()).await.unwrap();

        let mut too_many = position();
        too_many.contracts = 4;
        let error = upsert_position(&pool, &too_many).await.unwrap_err();
        assert!(error.contains("requires 400 shares"));

        upsert_position(&pool, &position()).await.unwrap();
        assert_eq!(get_position(&pool).await.unwrap(), Some(position()));
    }

    #[tokio::test]
    async fn position_requires_matching_ticker_and_close_removes_it() {
        let pool = test_pool().await;
        upsert_settings(&pool, &settings()).await.unwrap();

        let mut mismatched = position();
        mismatched.stock_ticker = "MSFT".into();
        assert!(upsert_position(&pool, &mismatched).await.is_err());

        upsert_position(&pool, &position()).await.unwrap();
        close_position(&pool).await.unwrap();
        assert!(get_position(&pool).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn settings_survive_pool_reconnection() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "covered-call-advisor-{}-{}.db",
            std::process::id(),
            unique
        ));
        let url = format!("sqlite:{}?mode=rwc", path.display());

        let first = connect(&url).await.unwrap();
        upsert_settings(&first, &settings()).await.unwrap();
        first.close().await;

        let second = connect(&url).await.unwrap();
        assert_eq!(get_settings(&second).await.unwrap(), Some(settings()));
        second.close().await;
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn notification_log_prevents_duplicate_date_records() {
        let pool = test_pool().await;
        assert!(!was_notification_sent(&pool, "2026-11-18").await.unwrap());

        record_notification(&pool, "2026-11-18").await.unwrap();
        record_notification(&pool, "2026-11-18").await.unwrap();
        assert!(was_notification_sent(&pool, "2026-11-18").await.unwrap());
        assert!(!was_notification_sent(&pool, "2026-11-19").await.unwrap());
    }
}
