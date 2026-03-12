'use client';

export type TimelineEvent = {
  id: string;
  time: string;
  label: string;
  detail?: string;
  tone?: 'info' | 'success' | 'warning' | 'danger';
};

export function EventsTimeline({ events }: { events: TimelineEvent[] }) {
  return (
    <aside style={panelStyle}>
      <h3 style={{ marginTop: 0, marginBottom: 10 }}>Events Timeline</h3>
      {events.length === 0 ? (
        <p style={{ margin: 0, color: '#98a6d4' }}>No events yet.</p>
      ) : (
        <ul style={listStyle}>
          {events.map((event) => (
            <li key={event.id} style={{ ...eventStyle, borderLeftColor: toneColor(event.tone) }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8 }}>
                <strong>{event.label}</strong>
                <span style={{ color: '#91a0d6', fontSize: 12 }}>{event.time}</span>
              </div>
              {event.detail ? <p style={{ margin: '6px 0 0', color: '#cad3f7', fontSize: 13 }}>{event.detail}</p> : null}
            </li>
          ))}
        </ul>
      )}
    </aside>
  );
}

function toneColor(tone: TimelineEvent['tone']) {
  if (tone === 'success') return '#2fa56f';
  if (tone === 'warning') return '#c8962f';
  if (tone === 'danger') return '#c74f4f';
  return '#3f77ff';
}

const panelStyle: React.CSSProperties = {
  background: '#0f1530',
  border: '1px solid #273056',
  borderRadius: 10,
  padding: 12,
  height: 'fit-content'
};

const listStyle: React.CSSProperties = {
  margin: 0,
  padding: 0,
  listStyle: 'none',
  display: 'grid',
  gap: 8
};

const eventStyle: React.CSSProperties = {
  borderLeft: '4px solid',
  background: '#141d40',
  borderRadius: 8,
  padding: '8px 10px'
};
