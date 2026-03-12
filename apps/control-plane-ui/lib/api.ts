const API_BASE = process.env.NEXT_PUBLIC_API_BASE_URL || 'http://127.0.0.1:8080';

type ApiMethod = 'GET' | 'POST';

async function apiRequest<T>(path: string, tenantId: string, method: ApiMethod = 'GET', body?: unknown, roles = 'builder,tenant_admin,approver') {
  const res = await fetch(`${API_BASE}${path}`, {
    method,
    headers: {
      'content-type': 'application/json',
      'x-tenant-id': tenantId,
      'x-roles': roles
    },
    cache: 'no-store',
    body: body ? JSON.stringify(body) : undefined
  });

  const payload = await res.json().catch(() => ({}));
  if (!res.ok) {
    throw new Error(`${method} ${path} failed: ${res.status} ${JSON.stringify(payload)}`);
  }
  return payload as T;
}

export type WorkflowRunStatus = 'running' | 'waiting_approval' | 'succeeded' | 'failed';

export type RunWorkflowResponse = {
  instance_id: string;
  status: WorkflowRunStatus;
  trace_id: string;
  trigger?: string;
  inputs?: Record<string, unknown>;
};

export type WorkflowInstanceResponse = {
  instance_id: string;
  tenant_id: string;
  workflow_id: string;
  status: WorkflowRunStatus;
  current_node?: string | null;
  trace_id: string;
  started_at: string;
  completed_at?: string | null;
};

export async function getHealth() {
  const res = await fetch(`${API_BASE}/health`, { cache: 'no-store' });
  if (!res.ok) throw new Error(`health failed: ${res.status}`);
  return res.json();
}

export async function runtimeComplete(input: string, tenantId: string) {
  return apiRequest('/v1/runtime/complete', tenantId, 'POST', { input }, 'builder,tenant_admin');
}

export async function runWorkflow(workflowId: string, tenantId: string, trigger: string, inputs: Record<string, unknown>) {
  return apiRequest<RunWorkflowResponse>(`/v1/workflows/${workflowId}/run`, tenantId, 'POST', { trigger, inputs }, 'builder,tenant_admin');
}

export async function getWorkflowInstance(instanceId: string, tenantId: string) {
  return apiRequest<WorkflowInstanceResponse>(`/v1/workflow-instances/${instanceId}`, tenantId, 'GET', undefined, 'builder,tenant_admin,approver');
}

export async function sendApprovalDecision(approvalId: string, tenantId: string, decision: 'approve' | 'reject', comment?: string) {
  return apiRequest<{ approval_id: string; status: WorkflowRunStatus; comment?: string }>(
    `/v1/approvals/${approvalId}/decision`,
    tenantId,
    'POST',
    { decision, comment },
    'approver,tenant_admin'
  );
}
