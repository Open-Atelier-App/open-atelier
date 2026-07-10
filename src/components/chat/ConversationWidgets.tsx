import { useEffect, useState } from 'react';
import { Timer, X, Play, Pause, RotateCcw, XCircle } from 'lucide-react';
import { useWidgetStore, type ConversationWidget } from '../../stores/widgetStore';

function formatDuration(ms: number): string {
  const totalSeconds = Math.max(0, Math.round(ms / 1000));
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  const s = totalSeconds % 60;
  const pad = (n: number) => String(n).padStart(2, '0');
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`;
}

function WidgetCard({ children, onDismiss }: { children: React.ReactNode; onDismiss: () => void }) {
  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 10,
      background: 'var(--bg-surface)', border: '1px solid var(--border)',
      borderRadius: 8, padding: '8px 12px', width: 'fit-content',
    }}>
      {children}
      <button
        onClick={onDismiss}
        title="Dismiss"
        style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', padding: 2, display: 'flex' }}
      >
        <X size={13} />
      </button>
    </div>
  );
}

function StopwatchWidget({ onDismiss }: { onDismiss: () => void }) {
  // Lazy initializers (not a mount effect that would setState synchronously)
  // — the one-time "read the clock at creation" React itself sanctions for
  // computing initial state. Render itself only ever reads `now`, never the
  // clock directly.
  const [startedAt, setStartedAt] = useState<number | null>(() => Date.now());
  const [accumulatedMs, setAccumulatedMs] = useState(0);
  const [running, setRunning] = useState(true);
  const [now, setNow] = useState<number | null>(() => Date.now());

  useEffect(() => {
    if (!running) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [running]);

  const elapsed = accumulatedMs + (running && startedAt && now ? now - startedAt : 0);

  const handlePauseResume = () => {
    const t = Date.now();
    if (running) {
      setAccumulatedMs(accumulatedMs + (startedAt ? t - startedAt : 0));
      setStartedAt(null);
      setRunning(false);
    } else {
      setStartedAt(t);
      setNow(t);
      setRunning(true);
    }
  };

  const handleReset = () => {
    const t = Date.now();
    setAccumulatedMs(0);
    setStartedAt(running ? t : null);
    setNow(t);
  };

  return (
    <WidgetCard onDismiss={onDismiss}>
      <Timer size={14} color="var(--accent)" style={{ flexShrink: 0 }} />
      <span style={{ fontSize: 15, fontWeight: 600, color: 'var(--text-primary)', fontVariantNumeric: 'tabular-nums', minWidth: 56 }}>
        {formatDuration(elapsed)}
      </span>
      <WidgetIconButton title={running ? 'Pause' : 'Resume'} onClick={handlePauseResume}>
        {running ? <Pause size={13} /> : <Play size={13} />}
      </WidgetIconButton>
      <WidgetIconButton title="Reset" onClick={handleReset}>
        <RotateCcw size={13} />
      </WidgetIconButton>
    </WidgetCard>
  );
}

const PRESETS_MIN = [1, 5, 10, 25];

function CountdownWidget({ onDismiss }: { onDismiss: () => void }) {
  const [durationMs, setDurationMs] = useState<number | null>(null);
  const [startedAt, setStartedAt] = useState<number | null>(null);
  const [remainingAtPause, setRemainingAtPause] = useState<number | null>(null);
  const [running, setRunning] = useState(false);
  const [customMinutes, setCustomMinutes] = useState('');
  // Same rule as the stopwatch above: render only ever reads `now`, never
  // the clock directly — `now` is set from effects/handlers instead.
  const [now, setNow] = useState<number | null>(null);

  useEffect(() => {
    if (!running) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [running]);

  const start = (ms: number) => {
    const t = Date.now();
    setDurationMs(ms);
    setStartedAt(t);
    setNow(t);
    setRemainingAtPause(null);
    setRunning(true);
  };

  const remaining = durationMs == null ? null : !running
    ? (remainingAtPause ?? durationMs)
    : Math.max(0, durationMs - ((now ?? startedAt ?? 0) - (startedAt ?? 0)));

  const done = remaining !== null && remaining <= 0;

  const handlePause = () => {
    if (remaining === null) return;
    setRemainingAtPause(remaining);
    setRunning(false);
  };

  const handleResume = () => {
    if (remaining === null) return;
    const t = Date.now();
    setDurationMs(remaining);
    setStartedAt(t);
    setNow(t);
    setRemainingAtPause(null);
    setRunning(true);
  };

  if (durationMs == null) {
    return (
      <WidgetCard onDismiss={onDismiss}>
        <Timer size={14} color="var(--accent)" style={{ flexShrink: 0 }} />
        <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>Countdown:</span>
        {PRESETS_MIN.map(min => (
          <button
            key={min}
            onClick={() => start(min * 60_000)}
            style={{
              padding: '3px 9px', borderRadius: 4, fontSize: 12, border: '1px solid var(--border)',
              background: 'var(--overlay)', color: 'var(--text-primary)', cursor: 'pointer',
            }}
          >
            {min}m
          </button>
        ))}
        <input
          type="number"
          min={1}
          placeholder="min"
          value={customMinutes}
          onChange={e => setCustomMinutes(e.target.value)}
          onKeyDown={e => {
            if (e.key === 'Enter') {
              const n = Number(customMinutes);
              if (n > 0) start(n * 60_000);
            }
          }}
          style={{
            width: 48, padding: '3px 6px', borderRadius: 4, border: '1px solid var(--border)',
            background: 'var(--bg-app)', color: 'var(--text-primary)', fontSize: 12,
          }}
        />
      </WidgetCard>
    );
  }

  return (
    <WidgetCard onDismiss={onDismiss}>
      <Timer size={14} color={done ? 'var(--error)' : 'var(--accent)'} style={{ flexShrink: 0 }} />
      <span style={{
        fontSize: 15, fontWeight: 600, fontVariantNumeric: 'tabular-nums', minWidth: 56,
        color: done ? 'var(--error)' : 'var(--text-primary)',
      }}>
        {done ? "Time's up" : formatDuration(remaining ?? 0)}
      </span>
      {!done && (
        <WidgetIconButton title={running ? 'Pause' : 'Resume'} onClick={running ? handlePause : handleResume}>
          {running ? <Pause size={13} /> : <Play size={13} />}
        </WidgetIconButton>
      )}
      <WidgetIconButton title="Cancel" onClick={() => { setDurationMs(null); setRunning(false); }}>
        <XCircle size={13} />
      </WidgetIconButton>
    </WidgetCard>
  );
}

function WidgetIconButton({ title, onClick, children }: { title: string; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      title={title}
      style={{
        background: 'var(--overlay)', border: 'none', borderRadius: 4, padding: 5,
        cursor: 'pointer', color: 'var(--text-muted)', display: 'flex', alignItems: 'center',
      }}
    >
      {children}
    </button>
  );
}

export function ConversationWidgets({ conversationId }: { conversationId: number }) {
  const widgets = useWidgetStore(s => s.widgets[conversationId] ?? []);
  const removeWidget = useWidgetStore(s => s.removeWidget);

  if (widgets.length === 0) return null;

  return (
    <div style={{ padding: '0 24px 12px', display: 'flex', flexDirection: 'column', gap: 8, flexShrink: 0 }}>
      {widgets.map((w: ConversationWidget) => (
        w.kind === 'timer'
          ? <StopwatchWidget key={w.id} onDismiss={() => removeWidget(conversationId, w.id)} />
          : <CountdownWidget key={w.id} onDismiss={() => removeWidget(conversationId, w.id)} />
      ))}
    </div>
  );
}
