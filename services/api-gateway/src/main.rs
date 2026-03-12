use axum::{
    Json, Router,
    extract::Request,
    http::{StatusCode, header::HeaderName},
    middleware::Next,
    response::IntoResponse,
    routing::{get, post},
};
use platform_common::TenantContext;
use std::net::SocketAddr;
use tracing::info;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/runtime/complete", post(stub_complete))
        .route("/v1/admin/tenants", get(admin_tenant_list))
        .route_layer(axum::middleware::from_fn(tenant_context_middleware));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("api-gateway listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status":"ok","service":"api-gateway"}))
}

async fn stub_complete(req: Request) -> impl IntoResponse {
    let ctx = req.extensions().get::<TenantContext>().cloned();

    Json(serde_json::json!({
        "trace_id": "tr_bootstrap",
        "message": "runtime completion stub ready",
        "tenant_context": ctx
    }))
}

async fn admin_tenant_list(req: Request) -> Result<impl IntoResponse, StatusCode> {
    let ctx = req
        .extensions()
        .get::<TenantContext>()
        .cloned()
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !ctx.has_role("tenant_admin") {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(Json(serde_json::json!({
        "items": [{"tenant_id": ctx.tenant_id, "name": "example-tenant"}]
    })))
}

async fn tenant_context_middleware(
    mut req: Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    if req.uri().path() == "/health" {
        return Ok(next.run(req).await);
    }

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

    let ctx = TenantContext {
        tenant_id,
        workspace_id: None,
        user_id: None,
        roles,
    };
    req.extensions_mut().insert(ctx);
    Ok(next.run(req).await)
}
