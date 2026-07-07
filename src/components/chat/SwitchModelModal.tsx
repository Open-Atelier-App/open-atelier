import { useState } from 'react';
import { Archive } from 'lucide-react';
import { ProviderBadge } from './ProviderBadge';
import type { ModelOption } from '../../lib/types';

interface Props {
  visibleModels: ModelOption[];
  currentProvider: string;
  currentModel: string;
  onClose: () => void;
  onConfirm: (provider: string, model: string) => Promise<void>;
}

// The model picker locks for the rest of a conversation once it has
// messages, to keep a thread internally consistent — this modal is the
// guided path out of that lock: pick a different model, and it compresses
// the conversation into a memory first so the switch doesn't lose context.
export function SwitchModelModal({ visibleModels, currentProvider, currentModel, onClose, onConfirm }: Props) {
  const firstOther = visibleModels.find(m => !(m.provider === currentProvider && m.id === currentModel));
  const [picked, setPicked] = useState<ModelOption | null>(firstOther ?? visibleModels[0] ?? null);
  const [switching, setSwitching] = useState(false);
  const [error, setError] = useState('');

  const handleConfirm = async () => {
    if (!picked || switching) return;
    setSwitching(true);
    setError('');
    try {
      await onConfirm(picked.provider, picked.id);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSwitching(false);
    }
  };

  return (
    <div
      onClick={onClose}
      style={{
        position: 'fixed', inset: 0, zIndex: 500,
        background: 'rgba(0,0,0,0.4)', display: 'flex',
        alignItems: 'center', justifyContent: 'center',
      }}
    >
      <div
        onClick={e => e.stopPropagation()}
        onKeyDown={e => { if (e.key === 'Escape') onClose(); }}
        style={{
          width: 380, background: 'var(--bg-surface)',
          border: '1px solid var(--border)', borderRadius: 12,
          boxShadow: '0 16px 48px rgba(0,0,0,0.2)', padding: 20,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
          <Archive size={16} color="var(--accent)" />
          <div style={{ fontSize: 15, fontWeight: 600, color: 'var(--text-primary)' }}>
            Switch model
          </div>
        </div>
        <div style={{ fontSize: 12, color: 'var(--text-muted)', lineHeight: 1.5, marginBottom: 14 }}>
          The model is locked for the rest of this session to keep it consistent. To switch, Atelier
          will first summarize everything so far into a memory, then continue from there with the new
          model — the original messages stay visible, but won't be resent on future turns.
        </div>

        {visibleModels.length === 0 ? (
          <div style={{ fontSize: 12, color: 'var(--text-muted)', marginBottom: 14 }}>
            No other connected models. Add an API key in Settings first.
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4, marginBottom: 14, maxHeight: 220, overflow: 'auto' }}>
            {visibleModels.map(m => {
              const active = picked?.provider === m.provider && picked?.id === m.id;
              const isCurrent = m.provider === currentProvider && m.id === currentModel;
              return (
                <button
                  key={`${m.provider}:${m.id}`}
                  onClick={() => setPicked(m)}
                  style={{
                    display: 'flex', alignItems: 'center', gap: 8, width: '100%',
                    padding: '7px 10px', borderRadius: 6, textAlign: 'left',
                    background: active ? 'var(--overlay)' : 'none',
                    border: active ? '1px solid var(--accent)' : '1px solid transparent',
                    cursor: 'pointer', fontSize: 12, color: 'var(--text-primary)',
                  }}
                >
                  <ProviderBadge provider={m.provider} size={13} />
                  <span style={{ flex: 1 }}>{m.name}</span>
                  {isCurrent && <span style={{ fontSize: 10, color: 'var(--text-muted)' }}>current</span>}
                </button>
              );
            })}
          </div>
        )}

        {error && (
          <div style={{ fontSize: 12, color: 'var(--error)', marginBottom: 12 }}>{error}</div>
        )}

        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
          <button
            onClick={onClose}
            disabled={switching}
            style={{
              padding: '7px 14px', background: 'none', border: '1px solid var(--border)',
              borderRadius: 4, color: 'var(--text-muted)', fontSize: 13, cursor: switching ? 'default' : 'pointer',
            }}
          >
            Cancel
          </button>
          <button
            onClick={handleConfirm}
            disabled={!picked || switching}
            style={{
              padding: '7px 14px', background: 'var(--accent)', border: 'none',
              borderRadius: 4, color: '#fff', fontSize: 13, cursor: 'pointer',
              opacity: (!picked || switching) ? 0.5 : 1,
            }}
          >
            {switching ? 'Compressing…' : 'Compress & switch'}
          </button>
        </div>
      </div>
    </div>
  );
}
