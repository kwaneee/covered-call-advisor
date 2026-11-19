mod db;
mod market_data;
mod models;
mod notifications;
mod routes;
mod validation;

use std::env;
use std::fs;

use crate::db::connect;
use crate::market_data::{MockMarketDataProvider, RecommendationEngine};
use crate::routes::{AppState, router};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    fs::create_dir_all("data").expect("database data directory should be creatable");

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:data/covered-call-advisor.db?mode=rwc".to_string());
    let address = env::var("BACKEND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3001".to_string());

    let pool = connect(&database_url)
        .await
        .unwrap_or_else(|error| panic!("database should be available: {error}"));
    let recommendation_engine = RecommendationEngine::new(Arc::new(MockMarketDataProvider));
    let scheduler_pool = pool.clone();
    let scheduler_engine = recommendation_engine.clone();
    tokio::spawn(async move {
        loop {
            notifications::scheduled_tick(&scheduler_pool, &scheduler_engine).await;
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });
    let app = router(AppState {
        pool,
        recommendation_engine,
    });

    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .expect("backend address should be available");

    println!("Backend listening on http://{address}");
    axum::serve(listener, app)
        .await
        .expect("backend server should run");
}
