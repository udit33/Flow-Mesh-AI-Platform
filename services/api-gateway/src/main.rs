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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WorkflowRunStatus {
    Running,
    WaitingApproval,
    Succeeded,
    Failed,
}

impl WorkflowRunStatus {
    fn as_db_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::WaitingApproval => "waiting_approval",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "running" => Some(Self::Running),
            "waiting_approval" => Some(Self::WaitingApproval),
            "succeeded" => Some(Self::Succeeded),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
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

    let state = init_state().await;
    let app = build_app(state);

    let port = std::env::var("API_GATEWAY_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("api-gateway listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}

async fn init_state() -> AppState {
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

    AppState {
        db,
        jwt_secret,
        workflow_runs: Arc::new(RwLock::new(HashMap::new())),
    }
}

fn build_app(state: AppState) -> Router {
    Router::new()
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
        .with_state(state)
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
    if !ctx.has_role("tenant_admin") && !ctx.has_role("builder") {
        return Err(StatusCode::FORBIDDEN);
    }

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

    persist_run(&state, &run).await?;

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
    let run = fetch_workflow_run(&state, instance_id)
        .await?
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

    let desired_status = match payload.decision.as_str() {
        "approve" => WorkflowRunStatus::Succeeded,
        "reject" => WorkflowRunStatus::Failed,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let updated =
        apply_approval_decision(&state, approval_id, ctx.tenant_id, desired_status).await?;

    Ok(Json(serde_json::json!({
        "approval_id": approval_id,
        "status": updated.status,
        "comment": payload.comment
    })))
}

async fn persist_run(state: &AppState, run: &WorkflowRun) -> Result<(), StatusCode> {
    if let Some(db) = &state.db {
        sqlx::query(
            "INSERT INTO workflow_runs (id, tenant_id, workflow_id, status, trace_id) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(run.instance_id)
        .bind(run.tenant_id)
        .bind(run.workflow_id)
        .bind(run.status.as_db_str())
        .bind(&run.trace_id)
        .execute(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        return Ok(());
    }

    // Bootstrap fallback only when DB is unavailable.
    state
        .workflow_runs
        .write()
        .await
        .insert(run.instance_id, run.clone());
    Ok(())
}

async fn fetch_workflow_run(
    state: &AppState,
    instance_id: Uuid,
) -> Result<Option<WorkflowRun>, StatusCode> {
    if let Some(db) = &state.db {
        let row = sqlx::query(
            "SELECT id, tenant_id, workflow_id, status, trace_id, started_at, completed_at FROM workflow_runs WHERE id = $1",
        )
        .bind(instance_id)
        .fetch_optional(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        return row
            .map(|r| {
                let status_raw = r
                    .try_get::<String, _>("status")
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                let status = WorkflowRunStatus::from_db_str(&status_raw)
                    .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
                let started_at = r
                    .try_get::<chrono::DateTime<chrono::Utc>, _>("started_at")
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                    .to_rfc3339();
                let completed_at = r
                    .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("completed_at")
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                    .map(|v| v.to_rfc3339());

                Ok(WorkflowRun {
                    instance_id: r
                        .try_get("id")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    tenant_id: r
                        .try_get("tenant_id")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    workflow_id: r
                        .try_get("workflow_id")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    status,
                    current_node: None,
                    trace_id: r
                        .try_get("trace_id")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    started_at,
                    completed_at,
                })
            })
            .transpose();
    }

    Ok(state.workflow_runs.read().await.get(&instance_id).cloned())
}

async fn apply_approval_decision(
    state: &AppState,
    approval_id: Uuid,
    tenant_id: Uuid,
    desired_status: WorkflowRunStatus,
) -> Result<WorkflowRun, StatusCode> {
    if let Some(db) = &state.db {
        let row = sqlx::query(
            "UPDATE workflow_runs
             SET status = $3,
                 completed_at = COALESCE(completed_at, NOW())
             WHERE id = $1
               AND tenant_id = $2
               AND (status IN ('running', 'waiting_approval') OR status = $3)
             RETURNING id, tenant_id, workflow_id, status, trace_id, started_at, completed_at",
        )
        .bind(approval_id)
        .bind(tenant_id)
        .bind(desired_status.as_db_str())
        .fetch_optional(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if let Some(r) = row {
            let status_raw = r
                .try_get::<String, _>("status")
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let status = WorkflowRunStatus::from_db_str(&status_raw)
                .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

            return Ok(WorkflowRun {
                instance_id: r
                    .try_get("id")
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                tenant_id: r
                    .try_get("tenant_id")
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                workflow_id: r
                    .try_get("workflow_id")
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                status,
                current_node: None,
                trace_id: r
                    .try_get("trace_id")
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                started_at: r
                    .try_get::<chrono::DateTime<chrono::Utc>, _>("started_at")
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                    .to_rfc3339(),
                completed_at: r
                    .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("completed_at")
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                    .map(|v| v.to_rfc3339()),
            });
        }

        let existing =
            sqlx::query("SELECT status FROM workflow_runs WHERE id = $1 AND tenant_id = $2")
                .bind(approval_id)
                .bind(tenant_id)
                .fetch_optional(db)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        return match existing {
            Some(row) => {
                let existing_status = row
                    .try_get::<String, _>("status")
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                if existing_status == desired_status.as_db_str() {
                    fetch_workflow_run(state, approval_id)
                        .await?
                        .ok_or(StatusCode::NOT_FOUND)
                } else {
                    Err(StatusCode::CONFLICT)
                }
            }
            None => Err(StatusCode::NOT_FOUND),
        };
    }

    // Bootstrap fallback only when DB is unavailable.
    let mut runs = state.workflow_runs.write().await;
    let run = runs.get_mut(&approval_id).ok_or(StatusCode::NOT_FOUND)?;

    if run.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    if run.status == desired_status {
        return Ok(run.clone());
    }

    if matches!(
        run.status,
        WorkflowRunStatus::Succeeded | WorkflowRunStatus::Failed
    ) {
        return Err(StatusCode::CONFLICT);
    }

    run.status = desired_status;
    run.current_node = None;
    run.completed_at = Some(chrono::Utc::now().to_rfc3339());

    Ok(run.clone())
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
    use axum::{
        body::{Body, to_bytes},
        http::Request as HttpRequest,
    };
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState {
            db: None,
            jwt_secret: None,
            workflow_runs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

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

    #[tokio::test]
    async fn run_workflow_requires_builder_or_admin_role() {
        let tenant = Uuid::new_v4();
        let workflow_id = Uuid::new_v4();
        let app = build_app(test_state());

        let req = HttpRequest::builder()
            .method("POST")
            .uri(format!("/v1/workflows/{workflow_id}/run"))
            .header("content-type", "application/json")
            .header("x-tenant-id", tenant.to_string())
            .body(Body::from(r#"{"trigger":"manual"}"#))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn approval_requires_approver_or_admin_role() {
        let tenant = Uuid::new_v4();
        let approval_id = Uuid::new_v4();
        let state = test_state();
        state.workflow_runs.write().await.insert(
            approval_id,
            WorkflowRun {
                instance_id: approval_id,
                tenant_id: tenant,
                workflow_id: Uuid::new_v4(),
                status: WorkflowRunStatus::WaitingApproval,
                current_node: Some("approve_step".to_string()),
                trace_id: "tr_test".to_string(),
                started_at: chrono::Utc::now().to_rfc3339(),
                completed_at: None,
            },
        );
        let app = build_app(state);

        let req = HttpRequest::builder()
            .method("POST")
            .uri(format!("/v1/approvals/{approval_id}/decision"))
            .header("content-type", "application/json")
            .header("x-tenant-id", tenant.to_string())
            .header("x-roles", "builder")
            .body(Body::from(r#"{"decision":"approve"}"#))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn workflow_instance_is_tenant_isolated() {
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let workflow_id = Uuid::new_v4();
        let app = build_app(test_state());

        let run_req = HttpRequest::builder()
            .method("POST")
            .uri(format!("/v1/workflows/{workflow_id}/run"))
            .header("content-type", "application/json")
            .header("x-tenant-id", tenant_a.to_string())
            .header("x-roles", "builder")
            .body(Body::from(r#"{"trigger":"manual"}"#))
            .unwrap();

        let run_res = app.clone().oneshot(run_req).await.unwrap();
        assert_eq!(run_res.status(), StatusCode::ACCEPTED);
        let run_body = to_bytes(run_res.into_body(), usize::MAX).await.unwrap();
        let run_json: serde_json::Value = serde_json::from_slice(&run_body).unwrap();
        let instance_id = run_json
            .get("instance_id")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();

        let get_req = HttpRequest::builder()
            .method("GET")
            .uri(format!("/v1/workflow-instances/{instance_id}"))
            .header("x-tenant-id", tenant_b.to_string())
            .header("x-roles", "builder")
            .body(Body::empty())
            .unwrap();

        let get_res = app.oneshot(get_req).await.unwrap();
        assert_eq!(get_res.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn approval_decision_is_idempotent_for_same_choice() {
        let tenant = Uuid::new_v4();
        let approval_id = Uuid::new_v4();
        let state = test_state();
        state.workflow_runs.write().await.insert(
            approval_id,
            WorkflowRun {
                instance_id: approval_id,
                tenant_id: tenant,
                workflow_id: Uuid::new_v4(),
                status: WorkflowRunStatus::WaitingApproval,
                current_node: Some("approve_step".to_string()),
                trace_id: "tr_test".to_string(),
                started_at: chrono::Utc::now().to_rfc3339(),
                completed_at: None,
            },
        );
        let app = build_app(state);

        for _ in 0..2 {
            let req = HttpRequest::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}/decision"))
                .header("content-type", "application/json")
                .header("x-tenant-id", tenant.to_string())
                .header("x-roles", "approver")
                .body(Body::from(r#"{"decision":"approve"}"#))
                .unwrap();

            let res = app.clone().oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::OK);
        }
    }

    #[tokio::test]
    async fn approval_reject_after_approve_conflicts() {
        let tenant = Uuid::new_v4();
        let approval_id = Uuid::new_v4();
        let state = test_state();
        state.workflow_runs.write().await.insert(
            approval_id,
            WorkflowRun {
                instance_id: approval_id,
                tenant_id: tenant,
                workflow_id: Uuid::new_v4(),
                status: WorkflowRunStatus::Succeeded,
                current_node: None,
                trace_id: "tr_test".to_string(),
                started_at: chrono::Utc::now().to_rfc3339(),
                completed_at: Some(chrono::Utc::now().to_rfc3339()),
            },
        );
        let app = build_app(state);

        let req = HttpRequest::builder()
            .method("POST")
            .uri(format!("/v1/approvals/{approval_id}/decision"))
            .header("content-type", "application/json")
            .header("x-tenant-id", tenant.to_string())
            .header("x-roles", "approver")
            .body(Body::from(r#"{"decision":"reject"}"#))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CONFLICT);
    }
}
