mod db;
mod models;
mod routes;
mod validation;

use std::env;
use std::fs;

use crate::db::connect;
use crate::routes::{AppState, router};

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
    let app = router(AppState { pool });

    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .expect("backend address should be available");

    println!("Backend listening on http://{address}");
    axum::serve(listener, app)
        .await
        .expect("backend server should run");
}
