import { useWorkspaceStore } from '../../stores/workspaceStore';

export function IndexProgressBar() {
  const progress = useWorkspaceStore(s => s.indexProgress);
  const active = useWorkspaceStore(s => s.active);

  if (!active || active.index_status === 'idle' || active.index_status === 'complete') {
    return null;
  }

  const pct = progress && progress.total > 0
    ? Math.round((progress.done / progress.total) * 100)
    : null;

  return (
    <div style={{
      position: 'relative',
      height: 28, background: 'var(--bg-surface)',
      borderTop: '1px solid var(--border)',
      display: 'flex', alignItems: 'center', gap: 10, padding: '0 16px',
      fontSize: 11, color: 'var(--text-muted)',
    }}>
      {/* Progress bar track */}
      <div style={{
        width: 120, height: 3, background: 'var(--overlay)',
        borderRadius: 2, overflow: 'hidden', flexShrink: 0,
      }}>
        <div style={{
          height: '100%', background: 'var(--accent)',
          width: pct != null ? `${pct}%` : '30%',
          transition: 'width 200ms ease-out',
          animation: pct == null ? 'pulse 1.5s ease-in-out infinite' : 'none',
        }} />
      </div>

      {pct != null ? (
        <span>Indexing {pct}% ({progress!.done}/{progress!.total})</span>
      ) : (
        <span>Indexing…</span>
      )}

      {progress?.current_file && (
        <span style={{
          fontFamily: 'JetBrains Mono, monospace', fontSize: 10,
          overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
          maxWidth: 300, color: 'var(--text-muted)',
        }}>
          {progress.current_file}
        </span>
      )}
    </div>
  );
}
