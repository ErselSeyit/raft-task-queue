use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use task_queue::{TaskQueue, TaskExecutor};
use tower_http::cors::CorsLayer;
use tracing::info;
use uuid::Uuid;
use shared::types::TaskStatus;

#[derive(Clone)]
struct AppState {
    queue: Arc<TaskQueue>,
}

#[derive(Deserialize)]
struct CreateTaskRequest {
    name: String,
    payload: Option<String>,
    max_retries: Option<u32>,
}

#[derive(Serialize)]
struct TaskResponse {
    id: String,
    name: String,
    status: String,
    created_at: String,
    retries: u32,
    max_retries: u32,
}

#[derive(Serialize)]
struct StatsResponse {
    stats: HashMap<String, usize>,
    total_tasks: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting API server...");

    let queue = Arc::new(TaskQueue::new(10000));
    let executor = Arc::new(TaskExecutor::new(Arc::clone(&queue), 10));
    
    // Start executor in background
    let executor_clone: Arc<TaskExecutor> = Arc::clone(&executor);
    tokio::spawn(async move {
        executor_clone.start().await;
    });

    let app_state = AppState {
        queue: Arc::clone(&queue),
    };

    let app = Router::new()
        .route("/api/tasks", post(create_task).get(list_tasks))
        .route("/api/tasks/:id", get(get_task).delete(cancel_task))
        .route("/api/stats", get(get_stats))
        .route("/health", get(health_check))
        .layer(CorsLayer::permissive()) // TODO: Configure specific origins for production
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    info!("API server listening on http://0.0.0.0:3000");
    
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "healthy" }))
}

async fn create_task(
    State(state): State<AppState>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<Json<TaskResponse>, StatusCode> {
    let payload = req.payload
        .map(|s| s.into_bytes())
        .unwrap_or_default();
    
    let max_retries = req.max_retries.unwrap_or(3);
    
    match state.queue.submit_task(req.name, payload, max_retries).await {
        Ok(task_id) => {
            let task = state.queue.get_task(task_id).await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            Ok(Json(TaskResponse {
                id: task.id.to_string(),
                name: task.name,
                status: format!("{:?}", task.status),
                created_at: task.created_at.to_rfc3339(),
                retries: task.retries,
                max_retries: task.max_retries,
            }))
        }
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TaskResponse>, StatusCode> {
    let task_id = Uuid::parse_str(&id)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let task = state.queue.get_task(task_id).await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    
    Ok(Json(TaskResponse {
        id: task.id.to_string(),
        name: task.name,
        status: format!("{:?}", task.status),
        created_at: task.created_at.to_rfc3339(),
        retries: task.retries,
        max_retries: task.max_retries,
    }))
}

async fn list_tasks(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Vec<TaskResponse>> {
    let status_filter = params.get("status")
        .and_then(|s| match s.as_str() {
            "Pending" => Some(TaskStatus::Pending),
            "Running" => Some(TaskStatus::Running),
            "Completed" => Some(TaskStatus::Completed),
            "Failed" => Some(TaskStatus::Failed),
            "Cancelled" => Some(TaskStatus::Cancelled),
            _ => None,
        });
    
    let tasks = state.queue.list_tasks(status_filter).await;
    
    Json(tasks.into_iter().map(|task| TaskResponse {
        id: task.id.to_string(),
        name: task.name,
        status: format!("{:?}", task.status),
        created_at: task.created_at.to_rfc3339(),
        retries: task.retries,
        max_retries: task.max_retries,
    }).collect())
}

async fn cancel_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let task_id = Uuid::parse_str(&id)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    state.queue.cancel_task(task_id).await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    
    Ok(StatusCode::NO_CONTENT)
}

async fn get_stats(
    State(state): State<AppState>,
) -> Json<StatsResponse> {
    let stats = state.queue.get_stats().await;
    let total_tasks: usize = stats.values().sum();
    
    Json(StatsResponse {
        stats,
        total_tasks,
    })
}

