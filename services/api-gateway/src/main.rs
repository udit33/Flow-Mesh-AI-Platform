use axum::{
    Extension, Json, Router,
    extract::{Path, Query, Request, State},
    http::{Method, StatusCode, header::AUTHORIZATION, header::HeaderName},
    middleware::Next,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use platform_common::{TenantContext, decode_access_token};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use tower_http::cors::{AllowHeaders, AllowOrigin, CorsLayer};
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: Option<PgPool>,
    jwt_secret: Option<Arc<String>>,
    agent_runtime_url: Arc<String>,
    http_client: reqwest::Client,
    workflows: Arc<RwLock<HashMap<Uuid, WorkflowRecord>>>,
    workflow_runs: Arc<RwLock<HashMap<Uuid, WorkflowRun>>>,
    approvals: Arc<RwLock<HashMap<Uuid, ApprovalRecord>>>,
    tools: Arc<RwLock<HashMap<Uuid, ToolRecord>>>,
    audit_events: Arc<RwLock<Vec<AuditEventRecord>>>,
    workflow_events: Arc<RwLock<Vec<WorkflowEventRecord>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WorkflowRunStatus {
    Running,
    WaitingApproval,
    Succeeded,
    Failed,
    Cancelled,
}

impl WorkflowRunStatus {
    fn as_db_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::WaitingApproval => "waiting_approval",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "running" => Some(Self::Running),
            "waiting_approval" => Some(Self::WaitingApproval),
            "succeeded" => Some(Self::Succeeded),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WorkflowEventType {
    RunCreated,
    NodeStarted,
    NodeCompleted,
    ApprovalWaiting,
    ApprovalDecided,
    RunCompleted,
    RunFailed,
    RunCancelled,
}

impl WorkflowEventType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::RunCreated => "run_created",
            Self::NodeStarted => "node_started",
            Self::NodeCompleted => "node_completed",
            Self::ApprovalWaiting => "approval_waiting",
            Self::ApprovalDecided => "approval_decided",
            Self::RunCompleted => "run_completed",
            Self::RunFailed => "run_failed",
            Self::RunCancelled => "run_cancelled",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "run_created" => Some(Self::RunCreated),
            "node_started" => Some(Self::NodeStarted),
            "node_completed" => Some(Self::NodeCompleted),
            "approval_waiting" => Some(Self::ApprovalWaiting),
            "approval_decided" => Some(Self::ApprovalDecided),
            "run_completed" => Some(Self::RunCompleted),
            "run_failed" => Some(Self::RunFailed),
            "run_cancelled" => Some(Self::RunCancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkflowEvent {
    id: Uuid,
    instance_id: Uuid,
    tenant_id: Uuid,
    event_type: WorkflowEventType,
    node_id: Option<String>,
    status: Option<WorkflowRunStatus>,
    created_at: String,
    data: serde_json::Value,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkflowRecord {
    id: Uuid,
    tenant_id: Uuid,
    name: String,
    version: String,
    definition_json: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct CreateWorkflowRequest {
    name: String,
    version: String,
    definition_json: serde_json::Value,
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

#[derive(Debug, Deserialize)]
struct ApprovalListQuery {
    status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
}

impl ApprovalStatus {
    fn as_db_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }

    fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApprovalRecord {
    id: Uuid,
    tenant_id: Uuid,
    run_id: Uuid,
    requester: String,
    reason: String,
    due_at: String,
    status: ApprovalStatus,
    created_at: String,
    decided_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WorkflowInstancesQuery {
    workflow_id: Option<Uuid>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WorkflowActionRequest {
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolRecord {
    id: Uuid,
    tenant_id: Uuid,
    workspace_id: Option<Uuid>,
    name: String,
    kind: String,
    version: String,
    schema_json: serde_json::Value,
    created_at: String,
}

#[derive(Debug, Clone, Serialize)]
struct AuditEventRecord {
    id: Uuid,
    tenant_id: Uuid,
    actor_user_id: Option<Uuid>,
    event_type: String,
    trace_id: Option<String>,
    payload_json: serde_json::Value,
    created_at: String,
}

#[derive(Debug, Clone, Serialize)]
struct WorkflowEventRecord {
    id: Uuid,
    tenant_id: Uuid,
    run_id: Uuid,
    event_type: String,
    payload_json: serde_json::Value,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct PolicyDoc {
    #[serde(default)]
    allow_roles: Vec<String>,
    #[serde(default)]
    deny_roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RegisterToolRequest {
    name: String,
    kind: String,
    version: String,
    schema_json: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct InvokeToolRequest {
    input: Option<serde_json::Value>,
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RuntimeCompleteRequest {
    input: String,
    preferred_agent: Option<String>,
}

#[derive(Debug, Serialize)]
struct AgentExecuteRequest {
    tenant_id: Uuid,
    user_id: Option<Uuid>,
    input: String,
    preferred_agent: Option<String>,
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
    let agent_runtime_url =
        std::env::var("AGENT_RUNTIME_URL").unwrap_or_else(|_| "http://127.0.0.1:8081".to_string());
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .expect("http client");

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
        agent_runtime_url: Arc::new(agent_runtime_url),
        http_client,
        workflows: Arc::new(RwLock::new(HashMap::new())),
        workflow_runs: Arc::new(RwLock::new(HashMap::new())),
        approvals: Arc::new(RwLock::new(HashMap::new())),
        tools: Arc::new(RwLock::new(HashMap::new())),
        audit_events: Arc::new(RwLock::new(Vec::new())),
        workflow_events: Arc::new(RwLock::new(Vec::new())),
    }
}

fn build_app(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(AllowHeaders::any())
        .allow_origin(AllowOrigin::list([
            "http://localhost:3000".parse().unwrap(),
            "http://127.0.0.1:3000".parse().unwrap(),
        ]));

    Router::new()
        .route("/", get(ui_shell))
        .route("/health", get(health))
        .route("/v1/runtime/complete", post(stub_complete))
        .route("/v1/admin/tenants", get(admin_tenant_list))
        .route("/v1/workflows", post(create_workflow).get(list_workflows))
        .route("/v1/workflows/{workflow_id}", get(get_workflow))
        .route(
            "/v1/workflows/{workflow_id}/publish",
            post(publish_workflow),
        )
        .route("/v1/workflows/{workflow_id}/run", post(run_workflow))
        .route("/v1/workflow-instances", get(list_workflow_instances))
        .route(
            "/v1/workflow-instances/{instance_id}",
            get(get_workflow_instance),
        )
        .route(
            "/v1/workflow-instances/{instance_id}/cancel",
            post(cancel_workflow_instance),
        )
        .route(
            "/v1/workflow-instances/{instance_id}/retry",
            post(retry_workflow_instance),
        )
        .route(
            "/v1/workflow-instances/{instance_id}/events",
            get(get_workflow_instance_events),
        )
        .route("/v1/approvals", get(list_approvals))
        .route(
            "/v1/approvals/{approval_id}/decision",
            post(approval_decision),
        )
        .route("/v1/tools", post(register_tool).get(list_tools))
        .route("/v1/tools/{tool_id}/invoke", post(invoke_tool))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            tenant_context_middleware,
        ))
        .layer(cors)
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status":"ok","service":"api-gateway"}))
}

async fn ui_shell() -> Html<&'static str> {
    Html(include_str!("ui_index.html"))
}

async fn stub_complete(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<RuntimeCompleteRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    if payload.input.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    enforce_policy(&state, &ctx, "runtime.complete").await?;

    let request = AgentExecuteRequest {
        tenant_id: ctx.tenant_id,
        user_id: ctx.user_id,
        input: payload.input,
        preferred_agent: payload.preferred_agent,
    };

    let url = format!("{}/v1/agent/execute", state.agent_runtime_url);
    let response = state
        .http_client
        .post(url)
        .json(&request)
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let status = response.status();
    let body: serde_json::Value = response.json().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    if !status.is_success() {
        return Err(StatusCode::BAD_GATEWAY);
    }

    Ok(Json(serde_json::json!({
        "trace_id": body.get("trace_id"),
        "output": body.get("output"),
        "selected_agent": body.get("selected_agent"),
        "delegated_to": body.get("delegated_to"),
        "steps": body.get("steps"),
        "tenant_context": ctx
    })))
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

async fn create_workflow(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<CreateWorkflowRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    if !ctx.has_role("tenant_admin") && !ctx.has_role("builder") {
        return Err(StatusCode::FORBIDDEN);
    }
    let wf = WorkflowRecord {
        id: Uuid::new_v4(),
        tenant_id: ctx.tenant_id,
        name: payload.name,
        version: payload.version,
        definition_json: payload.definition_json,
    };
    state.workflows.write().await.insert(wf.id, wf.clone());
    Ok((StatusCode::CREATED, Json(wf)))
}

async fn list_workflows(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    let items: Vec<WorkflowRecord> = state
        .workflows
        .read()
        .await
        .values()
        .filter(|w| w.tenant_id == ctx.tenant_id)
        .cloned()
        .collect();
    Ok(Json(serde_json::json!({"items": items})))
}

async fn get_workflow(
    State(state): State<AppState>,
    Path(workflow_id): Path<Uuid>,
    Extension(ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    let wf = state
        .workflows
        .read()
        .await
        .get(&workflow_id)
        .filter(|w| w.tenant_id == ctx.tenant_id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(wf))
}

async fn publish_workflow(
    State(state): State<AppState>,
    Path(workflow_id): Path<Uuid>,
    Extension(ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    let wf = state
        .workflows
        .read()
        .await
        .get(&workflow_id)
        .filter(|w| w.tenant_id == ctx.tenant_id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(
        serde_json::json!({"workflow_id": wf.id, "published": true}),
    ))
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
    enforce_policy(&state, &ctx, "workflow.run").await?;

    if let Some(existing) = state.workflows.read().await.get(&workflow_id).cloned() {
        if existing.tenant_id != ctx.tenant_id {
            return Err(StatusCode::NOT_FOUND);
        }
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

    let approval = ApprovalRecord {
        id: Uuid::new_v4(),
        tenant_id: ctx.tenant_id,
        run_id: instance_id,
        requester: ctx
            .user_id
            .map(|u| u.to_string())
            .unwrap_or_else(|| "system".to_string()),
        reason: "Workflow execution requires approval".to_string(),
        due_at: (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339(),
        status: ApprovalStatus::Pending,
        created_at: chrono::Utc::now().to_rfc3339(),
        decided_at: None,
    };
    persist_approval(&state, &approval).await?;

    record_workflow_event(
        &state,
        instance_id,
        ctx.tenant_id,
        WorkflowEventType::RunCreated,
        None,
        Some(WorkflowRunStatus::WaitingApproval),
        serde_json::json!({}),
    )
    .await;
    record_workflow_event(
        &state,
        instance_id,
        ctx.tenant_id,
        WorkflowEventType::ApprovalWaiting,
        Some("approval".to_string()),
        Some(WorkflowRunStatus::Running),
        serde_json::json!({"approval_id": approval.id}),
    )
    .await;

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

async fn list_workflow_instances(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Query(query): Query<WorkflowInstancesQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let status_filter = query
        .status
        .as_deref()
        .map(|s| WorkflowRunStatus::from_db_str(s).ok_or(StatusCode::BAD_REQUEST))
        .transpose()?;

    let items =
        fetch_workflow_runs_for_tenant(&state, ctx.tenant_id, query.workflow_id, status_filter)
            .await?;
    Ok(Json(serde_json::json!({"items": items})))
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

async fn cancel_workflow_instance(
    State(state): State<AppState>,
    Path(instance_id): Path<Uuid>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<WorkflowActionRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    if !ctx.has_role("tenant_admin") && !ctx.has_role("builder") {
        return Err(StatusCode::FORBIDDEN);
    }

    let run = set_workflow_run_status(
        &state,
        instance_id,
        ctx.tenant_id,
        WorkflowRunStatus::Cancelled,
    )
    .await?;
    record_workflow_event(
        &state,
        instance_id,
        ctx.tenant_id,
        WorkflowEventType::RunCancelled,
        run.current_node.clone(),
        Some(run.status.clone()),
        serde_json::json!({"reason": payload.reason}),
    )
    .await;

    Ok(Json(
        serde_json::json!({"instance_id": instance_id, "status": run.status}),
    ))
}

async fn retry_workflow_instance(
    State(state): State<AppState>,
    Path(instance_id): Path<Uuid>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<WorkflowActionRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    if !ctx.has_role("tenant_admin") && !ctx.has_role("builder") {
        return Err(StatusCode::FORBIDDEN);
    }

    let run = retry_workflow_run(&state, instance_id, ctx.tenant_id).await?;
    record_workflow_event(
        &state,
        instance_id,
        ctx.tenant_id,
        WorkflowEventType::NodeStarted,
        run.current_node.clone(),
        Some(run.status.clone()),
        serde_json::json!({"reason": payload.reason}),
    )
    .await;

    Ok(Json(
        serde_json::json!({"instance_id": instance_id, "status": run.status}),
    ))
}

async fn get_workflow_instance_events(
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

    let items = fetch_workflow_events(&state, ctx.tenant_id, instance_id).await?;
    Ok(Json(serde_json::json!({"items": items})))
}

async fn list_approvals(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Query(query): Query<ApprovalListQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    if !ctx.has_role("tenant_admin") && !ctx.has_role("approver") {
        return Err(StatusCode::FORBIDDEN);
    }

    let status_filter = match query.status.as_deref() {
        None => None,
        Some("pending") => Some(ApprovalStatus::Pending),
        Some("approved") => Some(ApprovalStatus::Approved),
        Some("rejected") => Some(ApprovalStatus::Rejected),
        Some(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let items = fetch_approvals_for_tenant(&state, ctx.tenant_id, status_filter).await?;
    Ok(Json(serde_json::json!({"items": items})))
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
        "approve" => ApprovalStatus::Approved,
        "reject" => ApprovalStatus::Rejected,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let (approval, run) =
        apply_approval_decision(&state, approval_id, ctx.tenant_id, desired_status.clone()).await?;

    record_audit_event(
        &state,
        &ctx,
        "approval.decision",
        serde_json::json!({
            "approval_id": approval.id,
            "run_id": approval.run_id,
            "decision": desired_status,
            "comment": payload.comment,
        }),
        Some(run.trace_id.clone()),
    )
    .await;

    let run_status = run.status.clone();
    record_workflow_event(
        &state,
        approval.run_id,
        approval.tenant_id,
        WorkflowEventType::ApprovalDecided,
        None,
        Some(run_status.clone()),
        serde_json::json!({
            "approval_id": approval.id,
            "status": approval.status,
            "comment": payload.comment,
        }),
    )
    .await;
    record_workflow_event(
        &state,
        approval.run_id,
        approval.tenant_id,
        WorkflowEventType::NodeCompleted,
        Some("approval".to_string()),
        Some(run_status.clone()),
        serde_json::json!({"approval_id": approval.id}),
    )
    .await;
    record_workflow_event(
        &state,
        approval.run_id,
        approval.tenant_id,
        if run_status == WorkflowRunStatus::Succeeded {
            WorkflowEventType::RunCompleted
        } else {
            WorkflowEventType::RunFailed
        },
        None,
        Some(run_status.clone()),
        serde_json::json!({}),
    )
    .await;

    Ok(Json(serde_json::json!({
        "approval_id": approval.id,
        "status": approval.status,
        "run_id": approval.run_id,
        "run_status": run.status,
        "comment": payload.comment
    })))
}

async fn register_tool(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<RegisterToolRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    if !can_register_tools(&ctx) {
        return Err(StatusCode::FORBIDDEN);
    }
    enforce_policy(&state, &ctx, "tool.register").await?;

    validate_tool_request(&payload).map_err(|_| StatusCode::BAD_REQUEST)?;

    let tool = ToolRecord {
        id: Uuid::new_v4(),
        tenant_id: ctx.tenant_id,
        workspace_id: ctx.workspace_id,
        name: payload.name,
        kind: payload.kind,
        version: payload.version,
        schema_json: payload.schema_json,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    persist_tool(&state, &tool).await?;

    record_audit_event(
        &state,
        &ctx,
        "tool.registered",
        serde_json::json!({
            "tool_id": tool.id,
            "name": tool.name,
            "kind": tool.kind,
            "version": tool.version,
            "workspace_id": tool.workspace_id,
        }),
        None,
    )
    .await;

    Ok((StatusCode::CREATED, Json(tool)))
}

async fn list_tools(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, StatusCode> {
    let items = fetch_tools_for_tenant(&state, ctx.tenant_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

async fn invoke_tool(
    State(state): State<AppState>,
    Path(tool_id): Path<Uuid>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<InvokeToolRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let tool = fetch_tool(&state, ctx.tenant_id, tool_id)
        .await?
        .ok_or(StatusCode::NOT_FOUND)?;

    let scope = payload.scope.unwrap_or_else(|| "default".to_string());
    if !can_invoke_tool(&ctx, tool_id, &scope) {
        return Err(StatusCode::FORBIDDEN);
    }
    enforce_policy(&state, &ctx, "tool.invoke").await?;

    let trace_id = format!("tr_{}", Uuid::new_v4().simple());
    let input = payload.input.unwrap_or(serde_json::json!({}));
    let result = serde_json::json!({
        "ok": true,
        "tool_id": tool_id,
        "scope": scope.clone(),
        "echo": input.clone(),
    });

    record_audit_event(
        &state,
        &ctx,
        "tool.invoked",
        serde_json::json!({
            "tool_id": tool_id,
            "tool_name": tool.name,
            "scope": scope,
            "input": input,
            "result": result,
        }),
        Some(trace_id.clone()),
    )
    .await;

    Ok(Json(serde_json::json!({
        "trace_id": trace_id,
        "tool_id": tool_id,
        "result": result,
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

async fn fetch_approvals_for_tenant(
    state: &AppState,
    tenant_id: Uuid,
    status_filter: Option<ApprovalStatus>,
) -> Result<Vec<ApprovalRecord>, StatusCode> {
    if let Some(db) = &state.db {
        let rows = match status_filter {
            Some(ref status) => sqlx::query("SELECT id, tenant_id, run_id, requester, reason, due_at, status, created_at, decided_at FROM approvals WHERE tenant_id = $1 AND status = $2 ORDER BY due_at ASC")
                .bind(tenant_id).bind(status.as_db_str()).fetch_all(db).await,
            None => sqlx::query("SELECT id, tenant_id, run_id, requester, reason, due_at, status, created_at, decided_at FROM approvals WHERE tenant_id = $1 ORDER BY due_at ASC")
                .bind(tenant_id).fetch_all(db).await,
        }.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        return rows.into_iter().map(row_to_approval).collect();
    }
    let approvals = state.approvals.read().await;
    let mut items = approvals
        .values()
        .filter(|a| a.tenant_id == tenant_id)
        .filter(|a| match status_filter.as_ref() {
            Some(s) => &a.status == s,
            None => true,
        })
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.due_at.cmp(&b.due_at));
    Ok(items)
}

async fn fetch_approval(
    state: &AppState,
    tenant_id: Uuid,
    approval_id: Uuid,
) -> Result<Option<ApprovalRecord>, StatusCode> {
    if let Some(db) = &state.db {
        let row = sqlx::query("SELECT id, tenant_id, run_id, requester, reason, due_at, status, created_at, decided_at FROM approvals WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id).bind(approval_id).fetch_optional(db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        return row.map(row_to_approval).transpose();
    }
    Ok(state
        .approvals
        .read()
        .await
        .get(&approval_id)
        .filter(|a| a.tenant_id == tenant_id)
        .cloned())
}

fn row_to_approval(r: sqlx::postgres::PgRow) -> Result<ApprovalRecord, StatusCode> {
    let status_raw = r
        .try_get::<String, _>("status")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let status =
        ApprovalStatus::from_db_str(&status_raw).ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(ApprovalRecord {
        id: r
            .try_get("id")
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        tenant_id: r
            .try_get("tenant_id")
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        run_id: r
            .try_get("run_id")
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        requester: r
            .try_get("requester")
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        reason: r
            .try_get("reason")
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        due_at: r
            .try_get::<chrono::DateTime<chrono::Utc>, _>("due_at")
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .to_rfc3339(),
        status,
        created_at: r
            .try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .to_rfc3339(),
        decided_at: r
            .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("decided_at")
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .map(|v| v.to_rfc3339()),
    })
}

async fn apply_approval_decision(
    state: &AppState,
    approval_id: Uuid,
    tenant_id: Uuid,
    desired_status: ApprovalStatus,
) -> Result<(ApprovalRecord, WorkflowRun), StatusCode> {
    let approval = fetch_approval(state, tenant_id, approval_id)
        .await?
        .ok_or(StatusCode::NOT_FOUND)?;
    if approval.status == desired_status {
        let run = fetch_workflow_run(state, approval.run_id)
            .await?
            .ok_or(StatusCode::NOT_FOUND)?;
        return Ok((approval, run));
    }
    if approval.status != ApprovalStatus::Pending {
        return Err(StatusCode::CONFLICT);
    }
    let target_run_status = match desired_status {
        ApprovalStatus::Approved => WorkflowRunStatus::Succeeded,
        ApprovalStatus::Rejected => WorkflowRunStatus::Failed,
        ApprovalStatus::Pending => return Err(StatusCode::BAD_REQUEST),
    };
    let now = chrono::Utc::now().to_rfc3339();
    if let Some(db) = &state.db {
        let mut tx = db
            .begin()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        sqlx::query("UPDATE approvals SET status = $3, decided_at = NOW() WHERE id = $1 AND tenant_id = $2 AND status = 'pending'")
            .bind(approval_id).bind(tenant_id).bind(desired_status.as_db_str()).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        sqlx::query("UPDATE workflow_runs SET status = $3, completed_at = COALESCE(completed_at, NOW()) WHERE id = $1 AND tenant_id = $2")
            .bind(approval.run_id).bind(tenant_id).bind(target_run_status.as_db_str()).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        tx.commit()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    } else {
        let mut approvals = state.approvals.write().await;
        let appr = approvals
            .get_mut(&approval_id)
            .ok_or(StatusCode::NOT_FOUND)?;
        if appr.tenant_id != tenant_id {
            return Err(StatusCode::FORBIDDEN);
        }
        appr.status = desired_status.clone();
        appr.decided_at = Some(now.clone());
        let mut runs = state.workflow_runs.write().await;
        let run = runs
            .get_mut(&approval.run_id)
            .ok_or(StatusCode::NOT_FOUND)?;
        if run.tenant_id != tenant_id {
            return Err(StatusCode::FORBIDDEN);
        }
        run.status = target_run_status;
        run.current_node = None;
        run.completed_at = Some(now.clone());
    }
    let approval = fetch_approval(state, tenant_id, approval_id)
        .await?
        .ok_or(StatusCode::NOT_FOUND)?;
    let run = fetch_workflow_run(state, approval.run_id)
        .await?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok((approval, run))
}

async fn persist_approval(state: &AppState, approval: &ApprovalRecord) -> Result<(), StatusCode> {
    if let Some(db) = &state.db {
        let due_at = chrono::DateTime::parse_from_rfc3339(&approval.due_at)
            .map_err(|_| StatusCode::BAD_REQUEST)?
            .with_timezone(&chrono::Utc);
        sqlx::query(
            "INSERT INTO approvals (id, tenant_id, run_id, requester, reason, due_at, status) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(approval.id)
        .bind(approval.tenant_id)
        .bind(approval.run_id)
        .bind(&approval.requester)
        .bind(&approval.reason)
        .bind(due_at)
        .bind(approval.status.as_db_str())
        .execute(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        return Ok(());
    }

    state
        .approvals
        .write()
        .await
        .insert(approval.id, approval.clone());
    Ok(())
}

async fn fetch_workflow_runs_for_tenant(
    state: &AppState,
    tenant_id: Uuid,
    workflow_id: Option<Uuid>,
    status: Option<WorkflowRunStatus>,
) -> Result<Vec<WorkflowRun>, StatusCode> {
    let runs: Vec<WorkflowRun> = state
        .workflow_runs
        .read()
        .await
        .values()
        .cloned()
        .filter(|r| r.tenant_id == tenant_id)
        .filter(|r| workflow_id.as_ref().is_none_or(|wf| wf == &r.workflow_id))
        .filter(|r| status.as_ref().is_none_or(|s| s == &r.status))
        .collect();
    Ok(runs)
}

async fn set_workflow_run_status(
    state: &AppState,
    instance_id: Uuid,
    tenant_id: Uuid,
    target_status: WorkflowRunStatus,
) -> Result<WorkflowRun, StatusCode> {
    let mut runs = state.workflow_runs.write().await;
    let run = runs.get_mut(&instance_id).ok_or(StatusCode::NOT_FOUND)?;
    if run.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }
    run.status = target_status;
    if matches!(
        run.status,
        WorkflowRunStatus::Succeeded | WorkflowRunStatus::Failed | WorkflowRunStatus::Cancelled
    ) {
        run.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }
    Ok(run.clone())
}

async fn retry_workflow_run(
    state: &AppState,
    instance_id: Uuid,
    tenant_id: Uuid,
) -> Result<WorkflowRun, StatusCode> {
    let mut run =
        set_workflow_run_status(state, instance_id, tenant_id, WorkflowRunStatus::Running).await?;
    run.current_node = Some("start".to_string());
    run.completed_at = None;
    state
        .workflow_runs
        .write()
        .await
        .insert(instance_id, run.clone());
    Ok(run)
}

async fn fetch_workflow_events(
    state: &AppState,
    tenant_id: Uuid,
    instance_id: Uuid,
) -> Result<Vec<WorkflowEvent>, StatusCode> {
    let items = state
        .workflow_events
        .read()
        .await
        .iter()
        .filter(|e| e.tenant_id == tenant_id && e.run_id == instance_id)
        .filter_map(|e| {
            let event_type = WorkflowEventType::from_str(&e.event_type)?;
            Some(WorkflowEvent {
                id: e.id,
                instance_id: e.run_id,
                tenant_id: e.tenant_id,
                event_type,
                node_id: e
                    .payload_json
                    .get("node_id")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string()),
                status: e
                    .payload_json
                    .get("status")
                    .and_then(|v| v.as_str())
                    .and_then(WorkflowRunStatus::from_db_str),
                created_at: e.created_at.clone(),
                data: e.payload_json.clone(),
            })
        })
        .collect();
    Ok(items)
}

async fn record_workflow_event(
    state: &AppState,
    instance_id: Uuid,
    tenant_id: Uuid,
    event_type: WorkflowEventType,
    node_id: Option<String>,
    status: Option<WorkflowRunStatus>,
    data: serde_json::Value,
) {
    let payload = serde_json::json!({
        "instance_id": instance_id,
        "node_id": node_id,
        "status": status.as_ref().map(|s| s.as_db_str()),
        "data": data,
    });
    state
        .workflow_events
        .write()
        .await
        .push(WorkflowEventRecord {
            id: Uuid::new_v4(),
            tenant_id,
            run_id: instance_id,
            event_type: event_type.as_str().to_string(),
            payload_json: payload,
            created_at: chrono::Utc::now().to_rfc3339(),
        });
}

async fn persist_tool(state: &AppState, tool: &ToolRecord) -> Result<(), StatusCode> {
    if let Some(db) = &state.db {
        sqlx::query(
            "INSERT INTO tools (id, tenant_id, workspace_id, name, kind, version, schema_json) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(tool.id)
        .bind(tool.tenant_id)
        .bind(tool.workspace_id)
        .bind(&tool.name)
        .bind(&tool.kind)
        .bind(&tool.version)
        .bind(&tool.schema_json)
        .execute(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        return Ok(());
    }

    state.tools.write().await.insert(tool.id, tool.clone());
    Ok(())
}

async fn fetch_tools_for_tenant(
    state: &AppState,
    tenant_id: Uuid,
) -> Result<Vec<ToolRecord>, StatusCode> {
    if let Some(db) = &state.db {
        let rows = sqlx::query(
            "SELECT id, tenant_id, workspace_id, name, kind, version, schema_json, created_at FROM tools WHERE tenant_id = $1 ORDER BY created_at DESC",
        )
        .bind(tenant_id)
        .fetch_all(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let items = rows
            .into_iter()
            .map(|r| {
                Ok(ToolRecord {
                    id: r
                        .try_get("id")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    tenant_id: r
                        .try_get("tenant_id")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    workspace_id: r
                        .try_get("workspace_id")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    name: r
                        .try_get("name")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    kind: r
                        .try_get("kind")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    version: r
                        .try_get("version")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    schema_json: r
                        .try_get("schema_json")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    created_at: r
                        .try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                        .to_rfc3339(),
                })
            })
            .collect::<Result<Vec<ToolRecord>, StatusCode>>()?;
        return Ok(items);
    }

    let tools = state.tools.read().await;
    Ok(tools
        .values()
        .filter(|t| t.tenant_id == tenant_id)
        .cloned()
        .collect())
}

async fn fetch_tool(
    state: &AppState,
    tenant_id: Uuid,
    tool_id: Uuid,
) -> Result<Option<ToolRecord>, StatusCode> {
    if let Some(db) = &state.db {
        let row = sqlx::query(
            "SELECT id, tenant_id, workspace_id, name, kind, version, schema_json, created_at FROM tools WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(tool_id)
        .fetch_optional(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        return row
            .map(|r| {
                Ok(ToolRecord {
                    id: r
                        .try_get("id")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    tenant_id: r
                        .try_get("tenant_id")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    workspace_id: r
                        .try_get("workspace_id")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    name: r
                        .try_get("name")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    kind: r
                        .try_get("kind")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    version: r
                        .try_get("version")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    schema_json: r
                        .try_get("schema_json")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                    created_at: r
                        .try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                        .to_rfc3339(),
                })
            })
            .transpose();
    }

    Ok(state
        .tools
        .read()
        .await
        .get(&tool_id)
        .filter(|t| t.tenant_id == tenant_id)
        .cloned())
}

async fn enforce_policy(
    state: &AppState,
    ctx: &TenantContext,
    action: &str,
) -> Result<(), StatusCode> {
    let Some(db) = &state.db else {
        // bootstrap mode: no policy DB, allow by default
        return Ok(());
    };

    let rows = sqlx::query(
        "SELECT policy_json FROM policies WHERE tenant_id = $1 AND scope IN ($2, 'global')",
    )
    .bind(ctx.tenant_id)
    .bind(action)
    .fetch_all(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for row in rows {
        let policy_json = row
            .try_get::<serde_json::Value, _>("policy_json")
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if !evaluate_policy_doc(&policy_json, &ctx.roles) {
            return Err(StatusCode::FORBIDDEN);
        }
    }

    Ok(())
}

fn evaluate_policy_doc(policy_json: &serde_json::Value, roles: &[String]) -> bool {
    let Ok(doc) = serde_json::from_value::<PolicyDoc>(policy_json.clone()) else {
        return true;
    };

    if doc.deny_roles.iter().any(|r| roles.iter().any(|x| x == r)) {
        return false;
    }

    if !doc.allow_roles.is_empty() && !doc.allow_roles.iter().any(|r| roles.iter().any(|x| x == r))
    {
        return false;
    }

    true
}

fn can_register_tools(ctx: &TenantContext) -> bool {
    ctx.has_role("builder") || ctx.has_role("tenant_admin")
}

fn can_invoke_tool(ctx: &TenantContext, tool_id: Uuid, scope: &str) -> bool {
    if can_register_tools(ctx) {
        return true;
    }

    if ctx.user_id.is_none() || !ctx.has_role("user") {
        return false;
    }

    let tool_scope = format!("tool:invoke:{tool_id}");
    let named_scope = format!("tool:invoke:{scope}");
    ctx.has_role("tool:invoke:*") || ctx.has_role(&tool_scope) || ctx.has_role(&named_scope)
}

fn validate_tool_request(payload: &RegisterToolRequest) -> Result<(), &'static str> {
    if payload.name.trim().is_empty()
        || payload.kind.trim().is_empty()
        || payload.version.trim().is_empty()
    {
        return Err("name/kind/version required");
    }

    let schema = payload
        .schema_json
        .as_object()
        .ok_or("schema_json must be an object")?;

    if let Some(t) = schema.get("type")
        && !t.is_string()
    {
        return Err("schema_json.type must be string");
    }

    if let Some(props) = schema.get("properties")
        && !props.is_object()
    {
        return Err("schema_json.properties must be object");
    }

    if let Some(required) = schema.get("required") {
        let arr = required
            .as_array()
            .ok_or("schema_json.required must be array")?;
        if !arr.iter().all(|v| v.is_string()) {
            return Err("schema_json.required entries must be strings");
        }
    }

    Ok(())
}

async fn record_audit_event(
    state: &AppState,
    ctx: &TenantContext,
    event_type: &str,
    payload_json: serde_json::Value,
    trace_id: Option<String>,
) {
    let event = AuditEventRecord {
        id: Uuid::new_v4(),
        tenant_id: ctx.tenant_id,
        actor_user_id: ctx.user_id,
        event_type: event_type.to_string(),
        trace_id: trace_id.clone(),
        payload_json,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    if let Some(db) = &state.db {
        let _ = sqlx::query(
            "INSERT INTO audit_events (id, tenant_id, actor_user_id, event_type, trace_id, payload_json) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(event.id)
        .bind(event.tenant_id)
        .bind(event.actor_user_id)
        .bind(&event.event_type)
        .bind(trace_id)
        .bind(&event.payload_json)
        .execute(db)
        .await;
        return;
    }

    state.audit_events.write().await.push(event);
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

    let user_header = HeaderName::from_static("x-user-id");
    let user_id = req
        .headers()
        .get(user_header)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| Uuid::parse_str(v).ok());

    Ok(TenantContext {
        tenant_id,
        workspace_id: None,
        user_id,
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
            agent_runtime_url: Arc::new("http://127.0.0.1:65535".to_string()),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(200))
                .build()
                .unwrap(),
            workflows: Arc::new(RwLock::new(HashMap::new())),
            workflow_runs: Arc::new(RwLock::new(HashMap::new())),
            approvals: Arc::new(RwLock::new(HashMap::new())),
            tools: Arc::new(RwLock::new(HashMap::new())),
            audit_events: Arc::new(RwLock::new(Vec::new())),
            workflow_events: Arc::new(RwLock::new(Vec::new())),
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
    async fn register_tool_requires_builder_or_admin() {
        let tenant = Uuid::new_v4();
        let app = build_app(test_state());

        let req = HttpRequest::builder()
            .method("POST")
            .uri("/v1/tools")
            .header("content-type", "application/json")
            .header("x-tenant-id", tenant.to_string())
            .header("x-roles", "user,tool:invoke:*")
            .header("x-user-id", Uuid::new_v4().to_string())
            .body(Body::from(
                r#"{"name":"jira","kind":"http","version":"1","schema_json":{"type":"object"}}"#,
            ))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn invoke_tool_requires_scoped_user_permission() {
        let tenant = Uuid::new_v4();
        let state = test_state();

        let tool_id = Uuid::new_v4();
        state.tools.write().await.insert(
            tool_id,
            ToolRecord {
                id: tool_id,
                tenant_id: tenant,
                workspace_id: None,
                name: "jira".into(),
                kind: "http".into(),
                version: "1".into(),
                schema_json: serde_json::json!({"type":"object"}),
                created_at: chrono::Utc::now().to_rfc3339(),
            },
        );

        let app = build_app(state.clone());
        let denied_req = HttpRequest::builder()
            .method("POST")
            .uri(format!("/v1/tools/{tool_id}/invoke"))
            .header("content-type", "application/json")
            .header("x-tenant-id", tenant.to_string())
            .header("x-roles", "user")
            .header("x-user-id", Uuid::new_v4().to_string())
            .body(Body::from(r#"{"scope":"ops","input":{"q":"x"}}"#))
            .unwrap();
        let denied_res = app.clone().oneshot(denied_req).await.unwrap();
        assert_eq!(denied_res.status(), StatusCode::FORBIDDEN);

        let allowed_req = HttpRequest::builder()
            .method("POST")
            .uri(format!("/v1/tools/{tool_id}/invoke"))
            .header("content-type", "application/json")
            .header("x-tenant-id", tenant.to_string())
            .header("x-roles", "user,tool:invoke:ops")
            .header("x-user-id", Uuid::new_v4().to_string())
            .body(Body::from(r#"{"scope":"ops","input":{"q":"x"}}"#))
            .unwrap();
        let allowed_res = app.oneshot(allowed_req).await.unwrap();
        assert_eq!(allowed_res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn workflow_instance_is_tenant_isolated() {
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let app = build_app(test_state());

        let create_req = HttpRequest::builder()
            .method("POST")
            .uri("/v1/workflows")
            .header("content-type", "application/json")
            .header("x-tenant-id", tenant_a.to_string())
            .header("x-roles", "builder")
            .body(Body::from(
                r#"{"name":"wf1","version":"1","definition_json":{"nodes":[{"id":"start"}],"edges":[]}}"#,
            ))
            .unwrap();
        let create_res = app.clone().oneshot(create_req).await.unwrap();
        let create_body = to_bytes(create_res.into_body(), usize::MAX).await.unwrap();
        let create_json: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
        let workflow_id = create_json.get("id").and_then(|v| v.as_str()).unwrap();

        let publish_req = HttpRequest::builder()
            .method("POST")
            .uri(format!("/v1/workflows/{workflow_id}/publish"))
            .header("x-tenant-id", tenant_a.to_string())
            .header("x-roles", "builder")
            .body(Body::empty())
            .unwrap();
        let publish_res = app.clone().oneshot(publish_req).await.unwrap();
        assert_eq!(publish_res.status(), StatusCode::OK);

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
        state.approvals.write().await.insert(
            approval_id,
            ApprovalRecord {
                id: approval_id,
                tenant_id: tenant,
                run_id: approval_id,
                requester: "alice@example.com".to_string(),
                reason: "Need approval".to_string(),
                due_at: chrono::Utc::now().to_rfc3339(),
                status: ApprovalStatus::Pending,
                created_at: chrono::Utc::now().to_rfc3339(),
                decided_at: None,
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

    #[test]
    fn policy_doc_allow_and_deny_roles() {
        let policy = serde_json::json!({"allow_roles":["builder"],"deny_roles":["suspended"]});

        let roles_ok = vec!["builder".to_string()];
        assert!(evaluate_policy_doc(&policy, &roles_ok));

        let roles_denied = vec!["builder".to_string(), "suspended".to_string()];
        assert!(!evaluate_policy_doc(&policy, &roles_denied));

        let roles_missing = vec!["user".to_string()];
        assert!(!evaluate_policy_doc(&policy, &roles_missing));
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
        state.approvals.write().await.insert(
            approval_id,
            ApprovalRecord {
                id: approval_id,
                tenant_id: tenant,
                run_id: approval_id,
                requester: "alice@example.com".to_string(),
                reason: "Need approval".to_string(),
                due_at: chrono::Utc::now().to_rfc3339(),
                status: ApprovalStatus::Approved,
                created_at: chrono::Utc::now().to_rfc3339(),
                decided_at: Some(chrono::Utc::now().to_rfc3339()),
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

    #[tokio::test]
    async fn workflow_e2e_create_publish_run_approval_complete() {
        let tenant = Uuid::new_v4();
        let app = build_app(test_state());

        let create_res = app.clone().oneshot(
            HttpRequest::builder()
                .method("POST")
                .uri("/v1/workflows")
                .header("content-type", "application/json")
                .header("x-tenant-id", tenant.to_string())
                .header("x-roles", "builder")
                .body(Body::from(r#"{"name":"wf-e2e","version":"1","definition_json":{"nodes":[{"id":"start"}],"edges":[]}}"#))
                .unwrap(),
        ).await.unwrap();
        assert_eq!(create_res.status(), StatusCode::CREATED);
        let create_body = to_bytes(create_res.into_body(), usize::MAX).await.unwrap();
        let create_json: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
        let workflow_id = create_json.get("id").and_then(|v| v.as_str()).unwrap();

        let publish_res = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{workflow_id}/publish"))
                    .header("x-tenant-id", tenant.to_string())
                    .header("x-roles", "builder")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(publish_res.status(), StatusCode::OK);

        let run_res = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{workflow_id}/run"))
                    .header("content-type", "application/json")
                    .header("x-tenant-id", tenant.to_string())
                    .header("x-roles", "builder")
                    .body(Body::from(
                        r#"{"trigger":"manual","inputs":{"case":"e2e"}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(run_res.status(), StatusCode::ACCEPTED);
        let run_body = to_bytes(run_res.into_body(), usize::MAX).await.unwrap();
        let run_json: serde_json::Value = serde_json::from_slice(&run_body).unwrap();
        let run_id = run_json
            .get("instance_id")
            .and_then(|v| v.as_str())
            .unwrap();

        let approvals_res = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("GET")
                    .uri("/v1/approvals?status=pending")
                    .header("x-tenant-id", tenant.to_string())
                    .header("x-roles", "approver")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(approvals_res.status(), StatusCode::OK);
        let approvals_body = to_bytes(approvals_res.into_body(), usize::MAX)
            .await
            .unwrap();
        let approvals_json: serde_json::Value = serde_json::from_slice(&approvals_body).unwrap();
        let approval_id = approvals_json["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item.get("run_id").and_then(|v| v.as_str()) == Some(run_id))
            .and_then(|item| item.get("id"))
            .and_then(|v| v.as_str())
            .unwrap();

        let decide_res = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri(format!("/v1/approvals/{approval_id}/decision"))
                    .header("content-type", "application/json")
                    .header("x-tenant-id", tenant.to_string())
                    .header("x-roles", "approver")
                    .body(Body::from(r#"{"decision":"approve","comment":"ship it"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(decide_res.status(), StatusCode::OK);

        let instance_res = app
            .oneshot(
                HttpRequest::builder()
                    .method("GET")
                    .uri(format!("/v1/workflow-instances/{run_id}"))
                    .header("x-tenant-id", tenant.to_string())
                    .header("x-roles", "builder")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(instance_res.status(), StatusCode::OK);
        let instance_body = to_bytes(instance_res.into_body(), usize::MAX)
            .await
            .unwrap();
        let instance_json: serde_json::Value = serde_json::from_slice(&instance_body).unwrap();
        assert_eq!(
            instance_json.get("status").and_then(|v| v.as_str()),
            Some("succeeded")
        );
    }

    #[tokio::test]
    async fn tenant_isolation_negative_for_workflow_list_get_run_and_approvals() {
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let app = build_app(test_state());

        let create_res = app.clone().oneshot(
            HttpRequest::builder()
                .method("POST")
                .uri("/v1/workflows")
                .header("content-type", "application/json")
                .header("x-tenant-id", tenant_a.to_string())
                .header("x-roles", "builder")
                .body(Body::from(r#"{"name":"wf-iso","version":"1","definition_json":{"nodes":[{"id":"start"}],"edges":[]}}"#))
                .unwrap(),
        ).await.unwrap();
        let create_body = to_bytes(create_res.into_body(), usize::MAX).await.unwrap();
        let create_json: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
        let workflow_id = create_json.get("id").and_then(|v| v.as_str()).unwrap();

        let _ = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{workflow_id}/publish"))
                    .header("x-tenant-id", tenant_a.to_string())
                    .header("x-roles", "builder")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let _ = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{workflow_id}/run"))
                    .header("content-type", "application/json")
                    .header("x-tenant-id", tenant_a.to_string())
                    .header("x-roles", "builder")
                    .body(Body::from(r#"{"trigger":"manual"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        let list_res = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("GET")
                    .uri("/v1/workflows")
                    .header("x-tenant-id", tenant_b.to_string())
                    .header("x-roles", "builder")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_res.status(), StatusCode::OK);
        let list_body = to_bytes(list_res.into_body(), usize::MAX).await.unwrap();
        let list_json: serde_json::Value = serde_json::from_slice(&list_body).unwrap();
        assert_eq!(list_json["items"].as_array().unwrap().len(), 0);

        let get_res = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("GET")
                    .uri(format!("/v1/workflows/{workflow_id}"))
                    .header("x-tenant-id", tenant_b.to_string())
                    .header("x-roles", "builder")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get_res.status(), StatusCode::NOT_FOUND);

        let run_res = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{workflow_id}/run"))
                    .header("content-type", "application/json")
                    .header("x-tenant-id", tenant_b.to_string())
                    .header("x-roles", "builder")
                    .body(Body::from(r#"{"trigger":"manual"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(run_res.status(), StatusCode::NOT_FOUND);

        let approvals_res = app
            .oneshot(
                HttpRequest::builder()
                    .method("GET")
                    .uri("/v1/approvals?status=pending")
                    .header("x-tenant-id", tenant_b.to_string())
                    .header("x-roles", "approver")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(approvals_res.status(), StatusCode::OK);
        let approvals_body = to_bytes(approvals_res.into_body(), usize::MAX)
            .await
            .unwrap();
        let approvals_json: serde_json::Value = serde_json::from_slice(&approvals_body).unwrap();
        assert_eq!(approvals_json["items"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn workflow_instances_list_supports_filters() {
        let tenant = Uuid::new_v4();
        let workflow_a = Uuid::new_v4();
        let workflow_b = Uuid::new_v4();
        let app = build_app(test_state());

        let _ = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{workflow_a}/run"))
                    .header("content-type", "application/json")
                    .header("x-tenant-id", tenant.to_string())
                    .header("x-roles", "builder")
                    .body(Body::from(r#"{"trigger":"manual"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let _ = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{workflow_b}/run"))
                    .header("content-type", "application/json")
                    .header("x-tenant-id", tenant.to_string())
                    .header("x-roles", "builder")
                    .body(Body::from(r#"{"trigger":"manual"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        let res = app
            .oneshot(
                HttpRequest::builder()
                    .method("GET")
                    .uri(format!(
                        "/v1/workflow-instances?workflow_id={workflow_a}&status=running"
                    ))
                    .header("x-tenant-id", tenant.to_string())
                    .header("x-roles", "builder")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["items"].as_array().unwrap().len(), 1);
    }
}
