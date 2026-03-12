use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: Option<PgPool>,
    registry: Arc<RwLock<HashMap<Uuid, HashMap<String, AgentProfile>>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    version: String,
    status: String,
    workspace_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
struct ExecuteRequest {
    tenant_id: Uuid,
    user_id: Option<Uuid>,
    input: String,
    preferred_agent: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ListAgentsQuery {
    tenant_id: Uuid,
    capability: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RegisterAgentRequest {
    tenant_id: Uuid,
    workspace_id: Option<Uuid>,
    name: String,
    #[serde(default)]
    agent_type: Option<AgentType>,
    #[serde(default)]
    capabilities: Option<Vec<String>>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct RegisterAgentResponse {
    updated: bool,
    source: &'static str,
    agent: AgentProfile,
}

#[derive(Debug, Clone, Serialize)]
struct ListAgentsResponse {
    source: &'static str,
    items: Vec<AgentProfile>,
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

    let db = if let Ok(database_url) = std::env::var("DATABASE_URL") {
        match PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
        {
            Ok(pool) => Some(pool),
            Err(e) => {
                warn!("failed to connect to postgres: {e}");
                None
            }
        }
    } else {
        None
    };

    let state = AppState {
        db,
        registry: Arc::new(RwLock::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/agents", get(list_agents))
        .route("/v1/agents/register", post(register_agent))
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

async fn list_agents(
    State(state): State<AppState>,
    Query(query): Query<ListAgentsQuery>,
) -> Result<Json<ListAgentsResponse>, (StatusCode, Json<serde_json::Value>)> {
    if let Some(db) = &state.db {
        let items = fetch_agents_from_db(db, query.tenant_id)
            .await
            .map_err(internal_error)?;
        return Ok(Json(ListAgentsResponse {
            source: "database",
            items: filter_by_capability(items, query.capability.as_deref()),
        }));
    }

    let items = fetch_agents_from_memory(&state.registry, query.tenant_id).await;
    Ok(Json(ListAgentsResponse {
        source: "memory",
        items: filter_by_capability(items, query.capability.as_deref()),
    }))
}

async fn register_agent(
    State(state): State<AppState>,
    Json(req): Json<RegisterAgentRequest>,
) -> Result<(StatusCode, Json<RegisterAgentResponse>), (StatusCode, Json<serde_json::Value>)> {
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "name cannot be empty"})),
        ));
    }

    let tenant_id = req.tenant_id;
    let mut agent = profile_from_registration(req);

    if let Some(db) = &state.db {
        let updated = upsert_agent_in_db(db, tenant_id, &agent)
            .await
            .map_err(internal_error)?;

        let refreshed = fetch_agents_from_db(db, tenant_id)
            .await
            .map_err(internal_error)?;
        if let Some(db_agent) = refreshed.into_iter().find(|a| a.name == agent.name) {
            agent = db_agent;
        }

        return Ok((
            StatusCode::OK,
            Json(RegisterAgentResponse {
                updated,
                source: "database",
                agent,
            }),
        ));
    }

    let mut registry = state.registry.write().await;
    let tenant_agents = registry.entry(tenant_id).or_default();
    let updated = tenant_agents
        .insert(agent.name.clone(), agent.clone())
        .is_some();

    Ok((
        StatusCode::OK,
        Json(RegisterAgentResponse {
            updated,
            source: "memory",
            agent,
        }),
    ))
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

    let registry = resolved_registry_for_tenant(&state, req.tenant_id)
        .await
        .map_err(internal_error)?;

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
        .unwrap_or_else(|| default_supervisor());

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

fn default_supervisor() -> AgentProfile {
    AgentProfile {
        name: "default-supervisor".into(),
        agent_type: AgentType::Supervisor,
        capabilities: vec!["routing".into()],
        version: "1.0.0".into(),
        status: "active".into(),
        workspace_id: None,
    }
}

fn default_registry() -> HashMap<String, AgentProfile> {
    HashMap::from([
        (
            "supervisor".into(),
            AgentProfile {
                name: "supervisor".into(),
                agent_type: AgentType::Supervisor,
                capabilities: vec!["routing".into(), "delegation".into()],
                version: "1.0.0".into(),
                status: "active".into(),
                workspace_id: None,
            },
        ),
        (
            "workflow-specialist".into(),
            AgentProfile {
                name: "workflow-specialist".into(),
                agent_type: AgentType::Specialist,
                capabilities: vec!["workflow_execution".into(), "approval_state".into()],
                version: "1.0.0".into(),
                status: "active".into(),
                workspace_id: None,
            },
        ),
        (
            "tool-specialist".into(),
            AgentProfile {
                name: "tool-specialist".into(),
                agent_type: AgentType::Specialist,
                capabilities: vec!["tool_selection".into(), "connector_calls".into()],
                version: "1.0.0".into(),
                status: "active".into(),
                workspace_id: None,
            },
        ),
    ])
}

fn infer_agent_type(name: &str) -> AgentType {
    if name == "supervisor" || name.contains("supervisor") {
        AgentType::Supervisor
    } else {
        AgentType::Specialist
    }
}

fn infer_capabilities(name: &str) -> Vec<String> {
    match name {
        "supervisor" => vec!["routing".into(), "delegation".into()],
        "workflow-specialist" => vec!["workflow_execution".into(), "approval_state".into()],
        "tool-specialist" => vec!["tool_selection".into(), "connector_calls".into()],
        _ => vec![],
    }
}

fn profile_from_registration(req: RegisterAgentRequest) -> AgentProfile {
    AgentProfile {
        name: req.name,
        agent_type: req.agent_type.unwrap_or(AgentType::Specialist),
        capabilities: req.capabilities.unwrap_or_default(),
        version: req.version.unwrap_or_else(|| "1.0.0".into()),
        status: req.status.unwrap_or_else(|| "active".into()),
        workspace_id: req.workspace_id,
    }
}

fn filter_by_capability(items: Vec<AgentProfile>, capability: Option<&str>) -> Vec<AgentProfile> {
    let Some(capability) = capability else {
        return items;
    };

    items
        .into_iter()
        .filter(|a| a.capabilities.iter().any(|c| c == capability))
        .collect()
}

async fn fetch_agents_from_memory(
    registry: &Arc<RwLock<HashMap<Uuid, HashMap<String, AgentProfile>>>>,
    tenant_id: Uuid,
) -> Vec<AgentProfile> {
    let mem = registry.read().await;
    mem.get(&tenant_id)
        .map(|m| m.values().cloned().collect())
        .unwrap_or_else(|| default_registry().values().cloned().collect())
}

async fn resolved_registry_for_tenant(
    state: &AppState,
    tenant_id: Uuid,
) -> Result<HashMap<String, AgentProfile>, sqlx::Error> {
    if let Some(db) = &state.db {
        let db_agents = fetch_agents_from_db(db, tenant_id).await?;
        if !db_agents.is_empty() {
            return Ok(db_agents
                .into_iter()
                .map(|a| (a.name.clone(), a))
                .collect::<HashMap<_, _>>());
        }
    }

    let mem = state.registry.read().await;
    Ok(mem
        .get(&tenant_id)
        .cloned()
        .unwrap_or_else(default_registry))
}

async fn fetch_agents_from_db(
    db: &PgPool,
    tenant_id: Uuid,
) -> Result<Vec<AgentProfile>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT name, version, status, workspace_id FROM agents WHERE tenant_id = $1 ORDER BY created_at ASC",
    )
    .bind(tenant_id)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let name = r.get::<String, _>("name");
            AgentProfile {
                agent_type: infer_agent_type(&name),
                capabilities: infer_capabilities(&name),
                version: r.get::<String, _>("version"),
                status: r.get::<String, _>("status"),
                workspace_id: r.get::<Option<Uuid>, _>("workspace_id"),
                name,
            }
        })
        .collect())
}

async fn upsert_agent_in_db(
    db: &PgPool,
    tenant_id: Uuid,
    profile: &AgentProfile,
) -> Result<bool, sqlx::Error> {
    let existing_id =
        sqlx::query("SELECT id FROM agents WHERE tenant_id = $1 AND name = $2 LIMIT 1")
            .bind(tenant_id)
            .bind(&profile.name)
            .fetch_optional(db)
            .await?
            .map(|r| r.get::<Uuid, _>("id"));

    if let Some(id) = existing_id {
        sqlx::query("UPDATE agents SET workspace_id = $1, version = $2, status = $3 WHERE id = $4")
            .bind(profile.workspace_id)
            .bind(&profile.version)
            .bind(&profile.status)
            .bind(id)
            .execute(db)
            .await?;

        Ok(true)
    } else {
        sqlx::query(
            "INSERT INTO agents (id, tenant_id, workspace_id, name, version, status) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(Uuid::new_v4())
        .bind(tenant_id)
        .bind(profile.workspace_id)
        .bind(&profile.name)
        .bind(&profile.version)
        .bind(&profile.status)
        .execute(db)
        .await?;

        Ok(false)
    }
}

fn internal_error<E: std::fmt::Display>(err: E) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": err.to_string()})),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_by_capability_works() {
        let items = vec![
            AgentProfile {
                name: "a".into(),
                agent_type: AgentType::Specialist,
                capabilities: vec!["x".into()],
                version: "1.0.0".into(),
                status: "active".into(),
                workspace_id: None,
            },
            AgentProfile {
                name: "b".into(),
                agent_type: AgentType::Specialist,
                capabilities: vec!["y".into()],
                version: "1.0.0".into(),
                status: "active".into(),
                workspace_id: None,
            },
        ];

        let filtered = filter_by_capability(items, Some("x"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "a");
    }

    #[tokio::test]
    async fn execute_prefers_tenant_memory_registry_over_default() {
        let tenant_id = Uuid::new_v4();
        let mut tenant_registry = HashMap::new();
        tenant_registry.insert(
            "supervisor".to_string(),
            AgentProfile {
                name: "tenant-supervisor".into(),
                agent_type: AgentType::Supervisor,
                capabilities: vec!["routing".into()],
                version: "2.0.0".into(),
                status: "active".into(),
                workspace_id: None,
            },
        );

        let state = AppState {
            db: None,
            registry: Arc::new(RwLock::new(HashMap::from([(tenant_id, tenant_registry)]))),
        };

        let resolved = resolved_registry_for_tenant(&state, tenant_id)
            .await
            .expect("registry resolves");

        assert_eq!(
            resolved.get("supervisor").map(|a| a.name.as_str()),
            Some("tenant-supervisor")
        );
    }

    #[test]
    fn select_agent_uses_preferred_agent_when_present() {
        let mut registry = default_registry();
        registry.insert(
            "custom".into(),
            AgentProfile {
                name: "custom".into(),
                agent_type: AgentType::Specialist,
                capabilities: vec![],
                version: "1.0.0".into(),
                status: "active".into(),
                workspace_id: None,
            },
        );

        let req = ExecuteRequest {
            tenant_id: Uuid::new_v4(),
            user_id: None,
            input: "anything".into(),
            preferred_agent: Some("custom".into()),
        };

        let (selected, delegated) = select_agent(&registry, &req);
        assert_eq!(selected.name, "custom");
        assert!(delegated.is_none());
    }
}
