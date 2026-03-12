use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    registry: Arc<RwLock<HashMap<String, AgentProfile>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AgentType {
    Supervisor,
    Specialist,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentProfile {
    name: String,
    agent_type: AgentType,
    capabilities: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ExecuteRequest {
    tenant_id: Uuid,
    user_id: Option<Uuid>,
    input: String,
    preferred_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ExecutionStep {
    step: usize,
    actor: String,
    action: String,
    result: String,
}

#[derive(Debug, Clone, Serialize)]
struct ExecuteResponse {
    trace_id: String,
    selected_agent: String,
    delegated_to: Option<String>,
    steps: Vec<ExecutionStep>,
    output: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let state = AppState {
        registry: Arc::new(RwLock::new(default_registry())),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/agent/execute", post(execute))
        .with_state(state);

    let port = std::env::var("AGENT_RUNTIME_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(8081);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("agent-runtime listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok","service":"agent-runtime"}))
}

async fn execute(
    State(state): State<AppState>,
    Json(req): Json<ExecuteRequest>,
) -> Result<(StatusCode, Json<ExecuteResponse>), (StatusCode, Json<serde_json::Value>)> {
    if req.input.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "input cannot be empty"})),
        ));
    }

    let registry = state.registry.read().await;
    let (selected, delegated) = select_agent(&registry, &req);

    let mut steps = vec![ExecutionStep {
        step: 1,
        actor: selected.name.clone(),
        action: "intent_classification".into(),
        result: format!(
            "tenant={} user={} input_class=general",
            req.tenant_id,
            req.user_id
                .map(|u| u.to_string())
                .unwrap_or_else(|| "anonymous".into())
        ),
    }];

    if let Some(sub) = delegated.clone() {
        steps.push(ExecutionStep {
            step: 2,
            actor: selected.name.clone(),
            action: "delegate".into(),
            result: format!("delegated_to={}", sub.name),
        });
        steps.push(ExecutionStep {
            step: 3,
            actor: sub.name.clone(),
            action: "specialist_execution".into(),
            result: "specialist handled request".into(),
        });
    } else {
        steps.push(ExecutionStep {
            step: 2,
            actor: selected.name.clone(),
            action: "direct_execution".into(),
            result: "handled without delegation".into(),
        });
    }

    let trace_id = format!("tr_{}", Uuid::new_v4().simple());
    let output = if let Some(sub) = delegated.as_ref() {
        format!(
            "Request processed by supervisor '{}' with specialist '{}' for tenant {}.",
            selected.name, sub.name, req.tenant_id
        )
    } else {
        format!(
            "Request processed by agent '{}' for tenant {}.",
            selected.name, req.tenant_id
        )
    };

    Ok((
        StatusCode::OK,
        Json(ExecuteResponse {
            trace_id,
            selected_agent: selected.name,
            delegated_to: delegated.map(|d| d.name),
            steps,
            output,
        }),
    ))
}

fn select_agent(
    registry: &HashMap<String, AgentProfile>,
    req: &ExecuteRequest,
) -> (AgentProfile, Option<AgentProfile>) {
    if let Some(pref) = &req.preferred_agent
        && let Some(agent) = registry.get(pref)
    {
        return (agent.clone(), None);
    }

    let supervisor = registry
        .get("supervisor")
        .cloned()
        .unwrap_or_else(|| AgentProfile {
            name: "default-supervisor".into(),
            agent_type: AgentType::Supervisor,
            capabilities: vec!["routing".into()],
        });

    let lower = req.input.to_lowercase();
    let specialist_key = if lower.contains("workflow") || lower.contains("approval") {
        Some("workflow-specialist")
    } else if lower.contains("tool") || lower.contains("connector") {
        Some("tool-specialist")
    } else {
        None
    };

    let delegated = specialist_key.and_then(|k| registry.get(k).cloned());
    (supervisor, delegated)
}

fn default_registry() -> HashMap<String, AgentProfile> {
    HashMap::from([
        (
            "supervisor".into(),
            AgentProfile {
                name: "supervisor".into(),
                agent_type: AgentType::Supervisor,
                capabilities: vec!["routing".into(), "delegation".into()],
            },
        ),
        (
            "workflow-specialist".into(),
            AgentProfile {
                name: "workflow-specialist".into(),
                agent_type: AgentType::Specialist,
                capabilities: vec!["workflow_execution".into(), "approval_state".into()],
            },
        ),
        (
            "tool-specialist".into(),
            AgentProfile {
                name: "tool-specialist".into(),
                agent_type: AgentType::Specialist,
                capabilities: vec!["tool_selection".into(), "connector_calls".into()],
            },
        ),
    ])
}
