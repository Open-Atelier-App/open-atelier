import { FileText, AlertCircle } from 'lucide-react';
import type { Citation } from '../../lib/types';
import { useUIStore } from '../../stores/uiStore';

interface Props {
  citations: Citation[];
}

export function CitationList({ citations }: Props) {
  const openFileViewer = useUIStore(s => s.openFileViewer);

  if (citations.length === 0) return null;

  return (
    <div style={{ padding: '4px 24px 4px 40px', display: 'flex', flexWrap: 'wrap', gap: 6 }}>
      {citations.map((c, i) => {
        const isDeleted = !c.file_id && !c.chunk_id;
        return (
          <button
            key={i}
            onClick={() => !isDeleted && openFileViewer(c.rel_path)}
            title={isDeleted ? '(file deleted)' : c.snippet}
            style={{
              display: 'flex', alignItems: 'center', gap: 5,
              padding: '3px 8px', borderRadius: 4,
              background: isDeleted ? 'var(--overlay)' : 'var(--bg-surface)',
              border: `1px solid ${isDeleted ? 'var(--border)' : 'var(--accent)'}`,
              cursor: isDeleted ? 'default' : 'pointer',
              fontSize: 11, color: isDeleted ? 'var(--text-muted)' : 'var(--accent)',
              fontFamily: 'JetBrains Mono, monospace',
              maxWidth: 220, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
            }}
          >
            {isDeleted
              ? <AlertCircle size={11} color="var(--text-muted)" />
              : <FileText size={11} />
            }
            {isDeleted
              ? <span style={{ color: 'var(--text-muted)' }}>{c.rel_path} (deleted)</span>
              : <span>{c.rel_path}{c.page ? `:${c.page}` : ''}</span>
            }
          </button>
        );
      })}
    </div>
  );
}
