'use client';

import { useSearchParams } from 'next/navigation';
import { useEffect, useMemo, useState } from 'react';
import { EventsTimeline, TimelineEvent } from '../../components/events-timeline';
import { getWorkflowInstance, runWorkflow, WorkflowInstanceResponse } from '../../lib/api';
import { DEFAULT_WORKFLOWS } from '../../lib/workflows';
import { useTenant } from '../providers';

function nowLabel() {
  return new Date().toISOString().slice(11, 19) + ' UTC';
}

export default function RunsPage() {
  const search = useSearchParams();
  const workflowFromQuery = search.get('workflowId') || DEFAULT_WORKFLOWS[0].id;
  const { tenantId } = useTenant();

  const [workflowId, setWorkflowId] = useState(workflowFromQuery);
  const [trigger, setTrigger] = useState('manual');
  const [inputsText, setInputsText] = useState('{"priority":"normal"}');
  const [currentRun, setCurrentRun] = useState<WorkflowInstanceResponse | null>(null);
  const [instanceId, setInstanceId] = useState('');
  const [events, setEvents] = useState<TimelineEvent[]>([]);
  const [loading, setLoading] = useState(false);
  const [polling, setPolling] = useState(false);
  const [error, setError] = useState('');

  const workflowChoices = useMemo(() => DEFAULT_WORKFLOWS, []);

  useEffect(() => {
    if (!polling || !instanceId) return;

    const timer = setInterval(async () => {
      try {
        const latest = await getWorkflowInstance(instanceId, tenantId);
        setCurrentRun(latest);
        setEvents((prev) => [
          {
            id: `${Date.now()}-${Math.random()}`,
            time: nowLabel(),
            label: `Polled status: ${latest.status}`,
            detail: latest.current_node ? `Current node: ${latest.current_node}` : undefined,
            tone: latest.status === 'failed' ? 'danger' : latest.status === 'succeeded' ? 'success' : 'info'
          },
          ...prev
        ].slice(0, 25));
        if (latest.status === 'succeeded' || latest.status === 'failed') {
          setPolling(false);
        }
      } catch (e) {
        setError(String(e));
        setPolling(false);
      }
    }, 3000);

    return () => clearInterval(timer);
  }, [polling, instanceId, tenantId]);

  return (
    <main style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: 16 }}>
      <section style={panelStyle}>
        <h2 style={{ marginTop: 0 }}>Run Console</h2>

        <label style={labelStyle}>
          Workflow
          <select value={workflowId} onChange={(e) => setWorkflowId(e.target.value)} style={inputStyle}>
            {workflowChoices.map((w) => (
              <option key={w.id} value={w.id}>
                {w.name} ({w.version})
              </option>
            ))}
          </select>
        </label>

        <label style={labelStyle}>
          Trigger
          <input value={trigger} onChange={(e) => setTrigger(e.target.value)} style={inputStyle} />
        </label>

        <label style={labelStyle}>
          Inputs (JSON)
          <textarea rows={4} value={inputsText} onChange={(e) => setInputsText(e.target.value)} style={inputStyle} />
        </label>

        <div style={{ display: 'flex', gap: 10, marginTop: 10 }}>
          <button
            style={btnStyle}
            disabled={loading}
            onClick={async () => {
              try {
                setError('');
                setLoading(true);
                const parsedInputs = JSON.parse(inputsText || '{}');
                const started = await runWorkflow(workflowId, tenantId, trigger, parsedInputs);
                setInstanceId(started.instance_id);
                setCurrentRun({
                  instance_id: started.instance_id,
                  tenant_id: tenantId,
                  workflow_id: workflowId,
                  status: started.status,
                  current_node: 'start',
                  trace_id: started.trace_id,
                  started_at: new Date().toISOString(),
                  completed_at: null
                });
                setEvents((prev) => [
                  {
                    id: `${Date.now()}`,
                    time: nowLabel(),
                    label: `Run started (${started.status})`,
                    detail: `Instance: ${started.instance_id}`,
                    tone: 'info'
                  },
                  ...prev
                ]);
                setPolling(true);
              } catch (e) {
                setError(String(e));
              } finally {
                setLoading(false);
              }
            }}
          >
            {loading ? 'Starting…' : 'Start Run'}
          </button>

          <button
            style={btnSecondaryStyle}
            disabled={!instanceId || loading}
            onClick={async () => {
              if (!instanceId) return;
              try {
                setError('');
                setLoading(true);
                const latest = await getWorkflowInstance(instanceId, tenantId);
                setCurrentRun(latest);
              } catch (e) {
                setError(String(e));
              } finally {
                setLoading(false);
              }
            }}
          >
            Refresh Status
          </button>
        </div>

        {error ? <p style={{ color: '#ff9f9f' }}>{error}</p> : null}

        <article style={{ ...panelStyle, marginTop: 14 }}>
          <h3 style={{ marginTop: 0, marginBottom: 10 }}>Run Status</h3>
          {!currentRun ? (
            <p style={{ color: '#9ca7cf' }}>No run selected.</p>
          ) : (
            <dl style={statusGridStyle}>
              <dt>Instance</dt>
              <dd>{currentRun.instance_id}</dd>
              <dt>Status</dt>
              <dd>{currentRun.status}</dd>
              <dt>Current node</dt>
              <dd>{currentRun.current_node || '-'}</dd>
              <dt>Trace</dt>
              <dd>{currentRun.trace_id}</dd>
              <dt>Started</dt>
              <dd>{currentRun.started_at}</dd>
              <dt>Completed</dt>
              <dd>{currentRun.completed_at || '-'}</dd>
            </dl>
          )}
          {polling ? <p style={{ color: '#9cd0ff', marginBottom: 0 }}>Polling status every 3s…</p> : null}
        </article>
      </section>

      <EventsTimeline events={events} />
    </main>
  );
}

const panelStyle: React.CSSProperties = {
  background: '#0f1530',
  border: '1px solid #273056',
  borderRadius: 10,
  padding: 12
};

const labelStyle: React.CSSProperties = {
  display: 'grid',
  gap: 6,
  marginBottom: 10
};

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

const btnSecondaryStyle: React.CSSProperties = {
  ...btnStyle,
  background: '#1c2752',
  border: '1px solid #324583'
};

const statusGridStyle: React.CSSProperties = {
  margin: 0,
  display: 'grid',
  gridTemplateColumns: '140px 1fr',
  gap: 6,
  fontSize: 14
};
