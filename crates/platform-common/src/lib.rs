use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantContext {
    pub tenant_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub roles: Vec<String>,
}

impl TenantContext {
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessClaims {
    pub sub: String,
    pub tenant_id: String,
    pub workspace_id: Option<String>,
    pub roles: Vec<String>,
    pub exp: usize,
    pub iat: Option<usize>,
}

impl AccessClaims {
    pub fn into_tenant_context(self) -> Result<TenantContext, PlatformError> {
        let tenant_id =
            Uuid::parse_str(&self.tenant_id).map_err(|_| PlatformError::Unauthorized)?;
        let user_id = Uuid::parse_str(&self.sub).ok();
        let workspace_id = self
            .workspace_id
            .as_deref()
            .and_then(|v| Uuid::parse_str(v).ok());

        Ok(TenantContext {
            tenant_id,
            workspace_id,
            user_id,
            roles: self.roles,
        })
    }
}

pub fn decode_access_token(token: &str, jwt_secret: &str) -> Result<AccessClaims, PlatformError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;

    let data = decode::<AccessClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(|_| PlatformError::Unauthorized)?;

    if data.claims.exp < Utc::now().timestamp() as usize {
        return Err(PlatformError::Unauthorized);
    }

    Ok(data.claims)
}

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("missing tenant context")]
    MissingTenantContext,
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
}
