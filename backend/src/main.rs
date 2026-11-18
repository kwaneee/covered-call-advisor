use axum::{Json, Router, routing::get};
use serde_json::{Value, json};

async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok"
    }))
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/api/health", get(health));
    let address = "127.0.0.1:3001";
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("backend address should be available");

    println!("Backend listening on http://{address}");
    axum::serve(listener, app)
        .await
        .expect("backend server should run");
}
