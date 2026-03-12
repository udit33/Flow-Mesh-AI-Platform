'use client';

import { useEffect, useState } from 'react';
import { getWorkflowInstance, sendApprovalDecision, WorkflowInstanceResponse } from '../../lib/api';
import { useTenant } from '../providers';

export default function ApprovalsPage() {
  const { tenantId } = useTenant();
  const [idsInput, setIdsInput] = useState('');
  const [items, setItems] = useState<WorkflowInstanceResponse[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');

  async function loadInbox() {
    const ids = idsInput
      .split(/\s|,|\n/)
      .map((v) => v.trim())
      .filter(Boolean);

    if (ids.length === 0) {
      setItems([]);
      return;
    }

    try {
      setError('');
      setLoading(true);
      const fetched = await Promise.all(ids.map((id) => getWorkflowInstance(id, tenantId).catch(() => null)));
      const pending = fetched.filter((run): run is WorkflowInstanceResponse => Boolean(run) && run.status === 'waiting_approval');
      setItems(pending);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    setItems([]);
    setMessage('');
  }, [tenantId]);

  return (
    <main>
      <h2 style={{ marginTop: 0 }}>Approval Inbox</h2>
      <p style={{ color: '#9ca7cf' }}>
        Paste run/approval instance IDs (comma or newline separated), then load pending approvals.
      </p>

      <textarea rows={4} value={idsInput} onChange={(e) => setIdsInput(e.target.value)} style={inputStyle} />

      <div style={{ marginTop: 10, display: 'flex', gap: 10 }}>
        <button style={btnStyle} onClick={loadInbox} disabled={loading}>
          {loading ? 'Loading…' : 'Load Inbox'}
        </button>
      </div>

      {error ? <p style={{ color: '#ff9f9f' }}>{error}</p> : null}
      {message ? <p style={{ color: '#a6f3bf' }}>{message}</p> : null}

      <div style={{ display: 'grid', gap: 10, marginTop: 14 }}>
        {items.length === 0 ? (
          <p style={{ color: '#9ca7cf' }}>No approvals waiting.</p>
        ) : (
          items.map((item) => (
            <article key={item.instance_id} style={cardStyle}>
              <div>
                <h3 style={{ margin: '0 0 6px' }}>{item.instance_id}</h3>
                <p style={{ margin: '0 0 6px', color: '#b3bfeb' }}>
                  Workflow: <code>{item.workflow_id}</code>
                </p>
                <p style={{ margin: 0, color: '#b3bfeb' }}>Status: {item.status}</p>
              </div>

              <div style={{ display: 'flex', gap: 8 }}>
                <button
                  style={approveStyle}
                  onClick={async () => {
                    try {
                      setError('');
                      setMessage('');
                      const result = await sendApprovalDecision(item.instance_id, tenantId, 'approve', 'Approved from inbox');
                      setMessage(`Approved ${result.approval_id}`);
                      await loadInbox();
                    } catch (e) {
                      setError(String(e));
                    }
                  }}
                >
                  Approve
                </button>
                <button
                  style={rejectStyle}
                  onClick={async () => {
                    try {
                      setError('');
                      setMessage('');
                      const result = await sendApprovalDecision(item.instance_id, tenantId, 'reject', 'Rejected from inbox');
                      setMessage(`Rejected ${result.approval_id}`);
                      await loadInbox();
                    } catch (e) {
                      setError(String(e));
                    }
                  }}
                >
                  Reject
                </button>
              </div>
            </article>
          ))
        )}
      </div>
    </main>
  );
}

const inputStyle: React.CSSProperties = {
  width: '100%',
  background: '#121936',
  color: '#eaf0ff',
  border: '1px solid #2a3565',
  borderRadius: 8,
  padding: '10px 12px',
  boxSizing: 'border-box'
};

const btnStyle: React.CSSProperties = {
  background: '#2d6cf6',
  color: 'white',
  border: 0,
  borderRadius: 8,
  padding: '10px 12px',
  cursor: 'pointer'
};

const cardStyle: React.CSSProperties = {
  display: 'flex',
  justifyContent: 'space-between',
  gap: 16,
  background: '#101834',
  border: '1px solid #27315c',
  borderRadius: 10,
  padding: 14,
  alignItems: 'center'
};

const approveStyle: React.CSSProperties = {
  ...btnStyle,
  background: '#2e8b57'
};

const rejectStyle: React.CSSProperties = {
  ...btnStyle,
  background: '#a13a44'
};
