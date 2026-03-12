-- Durable capabilities + invoke scopes

ALTER TABLE agents
ADD COLUMN IF NOT EXISTS capabilities_json JSONB NOT NULL DEFAULT '[]'::jsonb;

CREATE TABLE IF NOT EXISTS tool_invoke_scopes (
  id UUID PRIMARY KEY,
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  tool_id UUID NOT NULL REFERENCES tools(id) ON DELETE CASCADE,
  scope_name TEXT NOT NULL,
  allowed_roles TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(tenant_id, tool_id, scope_name)
);

CREATE INDEX IF NOT EXISTS idx_tool_invoke_scopes_tenant_tool
  ON tool_invoke_scopes(tenant_id, tool_id);
