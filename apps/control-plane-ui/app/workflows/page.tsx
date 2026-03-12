'use client';

import Link from 'next/link';
import { useState } from 'react';
import { runWorkflow } from '../../lib/api';
import { DEFAULT_WORKFLOWS } from '../../lib/workflows';
import { useTenant } from '../providers';

export default function WorkflowsPage() {
  const { tenantId } = useTenant();
  const [launchingId, setLaunchingId] = useState<string | null>(null);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');

  return (
    <main>
      <h2 style={{ marginTop: 0 }}>Workflow List</h2>
      <p style={{ color: '#9ca7cf' }}>Select a workflow to start a run or open the run console.</p>

      {error ? <p style={{ color: '#ff9f9f' }}>{error}</p> : null}
      {message ? <p style={{ color: '#a6f3bf' }}>{message}</p> : null}

      <div style={{ display: 'grid', gap: 12 }}>
        {DEFAULT_WORKFLOWS.map((workflow) => (
          <article key={workflow.id} style={cardStyle}>
            <div>
              <h3 style={{ margin: '0 0 4px' }}>{workflow.name}</h3>
              <p style={{ margin: '0 0 8px', color: '#b3bfeb' }}>{workflow.summary}</p>
              <span style={badgeStyle}>{workflow.version}</span>
              <code style={{ marginLeft: 8, color: '#8ea0dc' }}>{workflow.id}</code>
            </div>

            <div style={{ display: 'flex', gap: 10 }}>
              <button
                disabled={launchingId === workflow.id}
                style={btnStyle}
                onClick={async () => {
                  try {
                    setError('');
                    setMessage('');
                    setLaunchingId(workflow.id);
                    const result = await runWorkflow(workflow.id, tenantId, 'manual', {
                      source: 'workflow-list'
                    });
                    setMessage(`Run started: ${result.instance_id} (${result.status})`);
                  } catch (e) {
                    setError(String(e));
                  } finally {
                    setLaunchingId(null);
                  }
                }}
              >
                {launchingId === workflow.id ? 'Starting…' : 'Start Run'}
              </button>

              <Link style={btnSecondaryStyle} href={`/runs?workflowId=${workflow.id}`}>
                Open Console
              </Link>
            </div>
          </article>
        ))}
      </div>
    </main>
  );
}

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

const badgeStyle: React.CSSProperties = {
  background: '#223064',
  border: '1px solid #324583',
  borderRadius: 999,
  padding: '3px 8px',
  fontSize: 12
};

const btnStyle: React.CSSProperties = {
  background: '#2d6cf6',
  color: 'white',
  border: 0,
  borderRadius: 8,
  padding: '10px 12px',
  cursor: 'pointer',
  textDecoration: 'none'
};

const btnSecondaryStyle: React.CSSProperties = {
  ...btnStyle,
  background: '#1c2752',
  border: '1px solid #324583'
};
