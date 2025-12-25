use axum::{
    extract::Query,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct Task {
    id: String,
    name: String,
    status: String,
    created_at: String,
    retries: u32,
    max_retries: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = Router::new()
        .route("/", get(dashboard))
        .route("/api/proxy/tasks", get(proxy_tasks))
        .route("/api/proxy/stats", get(proxy_stats));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8081").await?;
    println!("Dashboard listening on http://0.0.0.0:8081");
    
    axum::serve(listener, app).await?;
    Ok(())
}

async fn dashboard() -> Html<&'static str> {
    Html(include_str!("dashboard.html"))
}

async fn proxy_tasks(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let client = reqwest::Client::new();
    let mut url = "http://localhost:3001/api/tasks".to_string();
    
    if let Some(status) = params.get("status") {
        url.push_str(&format!("?status={}", status));
    }
    
    match client.get(&url).send().await {
        Ok(response) => {
            match response.json::<serde_json::Value>().await {
                Ok(json) => axum::Json(json).into_response(),
                Err(_) => axum::Json(serde_json::json!([])).into_response(),
            }
        }
        Err(_) => axum::Json(serde_json::json!([])).into_response(),
    }
}

async fn proxy_stats() -> impl IntoResponse {
    let client = reqwest::Client::new();
    
    match client.get("http://localhost:3001/api/stats").send().await {
        Ok(response) => {
            match response.json::<serde_json::Value>().await {
                Ok(json) => axum::Json(json).into_response(),
                Err(_) => axum::Json(serde_json::json!({"stats": {}, "total_tasks": 0})).into_response(),
            }
        }
        Err(_) => axum::Json(serde_json::json!({"stats": {}, "total_tasks": 0})).into_response(),
    }
}

