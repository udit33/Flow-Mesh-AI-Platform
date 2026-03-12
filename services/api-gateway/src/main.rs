use axum::{
    Json, Router,
    extract::{Request, State},
    http::{StatusCode, header::AUTHORIZATION, header::HeaderName},
    middleware::Next,
    response::IntoResponse,
    routing::{get, post},
};
use platform_common::{TenantContext, decode_access_token};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use std::{net::SocketAddr, sync::Arc};
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: Option<PgPool>,
    jwt_secret: Option<Arc<String>>,
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

    let state = AppState { db, jwt_secret };

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/runtime/complete", post(stub_complete))
        .route("/v1/admin/tenants", get(admin_tenant_list))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            tenant_context_middleware,
        ))
        .with_state(state);

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

async fn admin_tenant_list(
    State(state): State<AppState>,
    req: Request,
) -> Result<impl IntoResponse, StatusCode> {
    let ctx = req
        .extensions()
        .get::<TenantContext>()
        .cloned()
        .ok_or(StatusCode::UNAUTHORIZED)?;

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

async fn tenant_context_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    if req.uri().path() == "/health" {
        return Ok(next.run(req).await);
    }

    let ctx = extract_context(&req, state.jwt_secret.as_deref())?;
    req.extensions_mut().insert(ctx);

    Ok(next.run(req).await)
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
