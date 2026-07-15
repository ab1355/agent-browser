use axum::{
    routing::{get, post},
    Router, Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use crate::color;

#[derive(Deserialize, Serialize)]
pub struct TaskRequest {
    pub name: String,
    pub version: String,
    pub steps: Vec<serde_json::Value>,
}

#[derive(Serialize)]
pub struct TaskResponse {
    pub task_id: String,
    pub status: String,
}

pub async fn start_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/api/v1/health", get(|| async { "OK" }))
        .route("/api/v1/tasks", post(create_task));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("{} Agent Browser REST API running on {}", color::success_indicator(), addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn create_task(Json(payload): Json<TaskRequest>) -> Result<Json<TaskResponse>, (StatusCode, String)> {
    // Scaffold implementation for task queuing.
    // In a full implementation, this would insert the task into a queue or execution pool.
    let task_id = format!("task_{}", uuid::Uuid::new_v4().simple());
    
    // Simulate webhook/deck execution setup.
    println!("{} Received task '{}' version '{}'. Enqueuing as {}", color::success_indicator(), payload.name, payload.version, task_id);

    Ok(Json(TaskResponse {
        task_id,
        status: "queued".to_string(),
    }))
}
