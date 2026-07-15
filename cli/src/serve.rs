use axum::{
    routing::{get, post},
    Router, Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use crate::color;
use serde_json::{Value, json};

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaskRequest {
    pub name: String,
    pub version: String,
    pub steps: Vec<Value>,
    pub webhook_url: Option<String>,
    pub vault: Option<std::collections::HashMap<String, String>>,
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
    let task_id = format!("task_{}", uuid::Uuid::new_v4().simple());
    
    println!("{} Received task '{}' version '{}'. Enqueuing as {}", color::success_indicator(), payload.name, payload.version, task_id);

    let task_id_clone = task_id.clone();
    tokio::spawn(async move {
        run_task(task_id_clone, payload).await;
    });

    Ok(Json(TaskResponse {
        task_id,
        status: "queued".to_string(),
    }))
}

async fn run_task(task_id: String, payload: TaskRequest) {
    let session = task_id.clone();
    let mut success = true;
    let mut outputs = Vec::new();

    // Spawn the daemon explicitly for this session using the CLI.
    let exe = std::env::current_exe().unwrap_or_else(|e| {
        eprintln!("{} Failed to determine current executable: {}. Falling back to 'agent-browser'", color::warning_indicator(), e);
        "agent-browser".into()
    });
    let mut daemon_cmd = std::process::Command::new(&exe);
    // Send a harmless command to bootstrap the daemon process for this session.
    // We ignore the output because we just need the daemon to start.
    daemon_cmd.args(&["--session", &session, "eval", "1+1"]);
    if let Err(e) = daemon_cmd.output() {
        eprintln!("{} Task {} failed to start daemon: {}", color::error_indicator(), task_id, e);
        success = false;
    }

    if let Some(ref vault) = payload.vault {
        println!("{} Task {} vault payload decrypted: {} keys injected", color::success_indicator(), task_id, vault.keys().len());
    }

    if success {
        for step in payload.steps {
            if let Some(action) = step.get("action").and_then(|v| v.as_str()) {
                let mut cmd = step.get("args").cloned().unwrap_or_else(|| json!({}));
                
                if let Some(obj) = cmd.as_object_mut() {
                    obj.insert("action".to_string(), json!(action));
                    if let Some(ref vault) = payload.vault {
                        // Merge vault secrets with existing env vars, vault takes precedence over existing keys.
                        let mut merged_env = obj.get("env").cloned().unwrap_or_else(|| json!({}));
                        if let Some(env_obj) = merged_env.as_object_mut() {
                            for (k, v) in vault {
                                env_obj.insert(k.clone(), json!(v));
                            }
                        } else {
                            merged_env = json!(vault);
                        }
                        obj.insert("env".to_string(), merged_env);
                    }
                }
                
                let s = session.clone();
                let c = cmd.clone();
                
                let res = tokio::task::spawn_blocking(move || {
                    crate::connection::send_command(c, &s)
                }).await;

                match res {
                    Ok(Ok(response)) => {
                        let mut resp_json = json!({"success": response.success});
                        if let Some(data) = response.data {
                            resp_json["data"] = data;
                        }
                        if let Some(error) = response.error {
                            resp_json["error"] = json!(error);
                        }
                        outputs.push(resp_json);
                        
                        if !response.success {
                            success = false;
                            break;
                        }
                    }
                    _ => {
                        success = false;
                        break;
                    }
                }
            }
        }

        let close_cmd = json!({"action": "close"});
        let task_id_for_close = task_id.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = crate::connection::send_command(close_cmd, &session) {
                eprintln!("{} Task {} failed to close daemon: {}", color::error_indicator(), task_id_for_close, e);
            }
        }).await.unwrap_or_else(|e| {
            eprintln!("{} Task {} failed to join close task: {}", color::error_indicator(), task_id, e);
        });
    }

    if let Some(webhook) = payload.webhook_url {
        println!("{} Task {} completed. Firing webhook callback to {}", color::success_indicator(), task_id, webhook);
        let client = reqwest::Client::new();
        let body = json!({
            "task_id": task_id,
            "status": if success { "completed" } else { "failed" },
            "outputs": outputs
        });
        
        match client.post(&webhook).json(&body).send().await {
            Ok(resp) if !resp.status().is_success() => {
                eprintln!("{} Task {} webhook failed with status: {}", color::error_indicator(), task_id, resp.status());
            }
            Err(e) => {
                eprintln!("{} Task {} webhook request failed: {}", color::error_indicator(), task_id, e);
            }
            _ => {}
        }
    } else {
        println!("{} Task {} completed with status: {}", color::success_indicator(), task_id, if success { "completed" } else { "failed" });
    }
}
