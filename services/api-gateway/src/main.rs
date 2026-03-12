use axum::{
    Extension, Json, Router,
    extract::{Path, Request, State},
    http::{StatusCode, header::AUTHORIZATION, header::HeaderName},
    middleware::Next,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use platform_common::{TenantContext, decode_access_token};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: Option<PgPool>,
    jwt_secret: Option<Arc<String>>,
    workflow_runs: Arc<RwLock<HashMap<Uuid, WorkflowRun>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WorkflowRunStatus {
    Running,
    WaitingApproval,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkflowRun {
    instance_id: Uuid,
    tenant_id: Uuid,
    workflow_id: Uuid,
    status: WorkflowRunStatus,
    current_node: Option<String>,
    trace_id: String,
    started_at: String,
    completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RunWorkflowRequest {
    trigger: Option<String>,
    inputs: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ApprovalDecisionRequest {
    decision: String,
    comment: Option<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let jwt_secret = std::env::var("JWT_SECRET").ok().map(Arc::new);

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
        jwt_secret,
        workflow_runs: Arc::new(RwLock::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/", get(ui_shell))
        .route("/health", get(health))
        .route("/v1/runtime/complete", post(stub_complete))
        .route("/v1/admin/tenants", get(admin_tenant_list))
        .route("/v1/workflows/{workflow_id}/run", post(run_workflow))
        .route(
            "/v1/workflow-instances/{instance_id}",
            get(get_workflow_instance),
        )
        .route(
            "/v1/approvals/{approval_id}/decision",
            post(approval_decision),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            tenant_context_middleware,
        ))
        .with_state(state);

    let port = std::env::var("API_GATEWAY_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("api-gateway listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status":"ok","service":"api-gateway"}))
}

async fn ui_shell() -> Html<&'static str> {
    Html(include_str!("ui_index.html"))
}

async fn stub_complete(Extension(ctx): Extension<TenantContext>) -> impl IntoResponse {
    Json(serde_json::json!({
        "trace_id": "tr_bootstrap",
        "message": "runtime completion stub ready",
        "tenant_context": ctx
    }))
}

async fn admin_tenant_list(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    if !ctx.has_role("tenant_admin") {
        return Err(StatusCode::FORBIDDEN);
    }

    if let Some(db) = &state.db {
        let rows = sqlx::query("SELECT id, name, region, tier FROM tenants WHERE id = $1")
            .bind(ctx.tenant_id)
            .fetch_all(db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let items = rows
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.try_get::<Uuid, _>("id").ok(),
                    "name": r.try_get::<String, _>("name").ok(),
                    "region": r.try_get::<String, _>("region").ok(),
                    "tier": r.try_get::<String, _>("tier").ok(),
                })
            })
            .collect::<Vec<_>>();

        return Ok(Json(serde_json::json!({ "items": items })));
    }

    Ok(Json(serde_json::json!({
        "items": [{"tenant_id": ctx.tenant_id, "name": "example-tenant", "mode": "mock"}]
    })))
}

async fn run_workflow(
    State(state): State<AppState>,
    Path(workflow_id): Path<Uuid>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<RunWorkflowRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let instance_id = Uuid::new_v4();
    let trace_id = format!("tr_{}", Uuid::new_v4().simple());

    let run = WorkflowRun {
        instance_id,
        tenant_id: ctx.tenant_id,
        workflow_id,
        status: WorkflowRunStatus::Running,
        current_node: Some("start".to_string()),
        trace_id: trace_id.clone(),
        started_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
    };

    if let Some(db) = &state.db {
        let _ = sqlx::query(
            "INSERT INTO workflow_runs (id, tenant_id, workflow_id, status, trace_id) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(instance_id)
        .bind(ctx.tenant_id)
        .bind(workflow_id)
        .bind("running")
        .bind(&trace_id)
        .execute(db)
        .await;
    }

    state.workflow_runs.write().await.insert(instance_id, run);

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "instance_id": instance_id,
            "status": "running",
            "trace_id": trace_id,
            "trigger": payload.trigger,
            "inputs": payload.inputs
        })),
    ))
}

async fn get_workflow_instance(
    State(state): State<AppState>,
    Path(instance_id): Path<Uuid>,
    Extension(ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    let runs = state.workflow_runs.read().await;
    let run = runs
        .get(&instance_id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;

    if run.tenant_id != ctx.tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(Json(run))
}

async fn approval_decision(
    State(state): State<AppState>,
    Path(approval_id): Path<Uuid>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<ApprovalDecisionRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    if !ctx.has_role("tenant_admin") && !ctx.has_role("approver") {
        return Err(StatusCode::FORBIDDEN);
    }

    let mut runs = state.workflow_runs.write().await;
    let run = runs.get_mut(&approval_id).ok_or(StatusCode::NOT_FOUND)?;

    if run.tenant_id != ctx.tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    match payload.decision.as_str() {
        "approve" => {
            run.status = WorkflowRunStatus::Succeeded;
            run.current_node = None;
            run.completed_at = Some(chrono::Utc::now().to_rfc3339());
        }
        "reject" => {
            run.status = WorkflowRunStatus::Failed;
            run.current_node = None;
            run.completed_at = Some(chrono::Utc::now().to_rfc3339());
        }
        _ => return Err(StatusCode::BAD_REQUEST),
    }

    Ok(Json(serde_json::json!({
        "approval_id": approval_id,
        "status": run.status,
        "comment": payload.comment
    })))
}

async fn tenant_context_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    if is_public_path(req.uri().path()) {
        return Ok(next.run(req).await);
    }

    let ctx = extract_context(&req, state.jwt_secret.as_deref())?;
    req.extensions_mut().insert(ctx);

    Ok(next.run(req).await)
}

fn is_public_path(path: &str) -> bool {
    matches!(path, "/" | "/health")
}

fn extract_context(
    req: &Request,
    jwt_secret: Option<&String>,
) -> Result<TenantContext, StatusCode> {
    if let Some(secret) = jwt_secret
        && let Some(auth) = req
            .headers()
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
        && let Some(token) = auth.strip_prefix("Bearer ")
    {
        let claims =
            decode_access_token(token.trim(), secret).map_err(|_| StatusCode::UNAUTHORIZED)?;
        return claims
            .into_tenant_context()
            .map_err(|_| StatusCode::UNAUTHORIZED);
    }

    // Development fallback headers (kept for bootstrap only)
    let tenant_header = HeaderName::from_static("x-tenant-id");
    let tenant_id = req
        .headers()
        .get(tenant_header)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| Uuid::parse_str(v).ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let roles_header = HeaderName::from_static("x-roles");
    let roles = req
        .headers()
        .get(roles_header)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(TenantContext {
        tenant_id,
        workspace_id: None,
        user_id: None,
        roles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request as HttpRequest};

    #[test]
    fn public_path_predicate_works() {
        assert!(is_public_path("/"));
        assert!(is_public_path("/health"));
        assert!(!is_public_path("/v1/runtime/complete"));
    }

    #[test]
    fn extract_context_from_headers() {
        let tenant = Uuid::new_v4();
        let req = HttpRequest::builder()
            .uri("/v1/runtime/complete")
            .header("x-tenant-id", tenant.to_string())
            .header("x-roles", "tenant_admin,builder")
            .body(Body::empty())
            .unwrap();

        let ctx = extract_context(&req, None).expect("context should parse");
        assert_eq!(ctx.tenant_id, tenant);
        assert!(ctx.has_role("tenant_admin"));
        assert!(ctx.has_role("builder"));
    }

    #[test]
    fn extract_context_requires_tenant() {
        let req = HttpRequest::builder()
            .uri("/v1/runtime/complete")
            .body(Body::empty())
            .unwrap();

        let result = extract_context(&req, None);
        assert!(result.is_err());
    }
}
