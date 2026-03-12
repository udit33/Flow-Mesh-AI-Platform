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

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("missing tenant context")]
    MissingTenantContext,
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
}
