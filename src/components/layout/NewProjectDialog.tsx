import { useState } from 'react';
import { FolderIcon } from 'lucide-react';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { errorMessage } from '../../lib/tauri';
import type { Workspace } from '../../lib/types';

interface Props {
  profileRootPath: string;
  // Only top-level projects can be picked as a parent — one level of
  // nesting is all sub-projects support today.
  topLevelProjects: Workspace[];
  onCreate: (path: string, parentWorkspaceId: number | null) => Promise<void>;
  onClose: () => void;
}

export function NewProjectDialog({ profileRootPath, topLevelProjects, onCreate, onClose }: Props) {
  const [name, setName] = useState('');
  const [path, setPath] = useState(profileRootPath);
  const [pathTouched, setPathTouched] = useState(false);
  const [parentId, setParentId] = useState<string>('');
  const [error, setError] = useState('');
  const [creating, setCreating] = useState(false);

  const handleNameChange = (value: string) => {
    setName(value);
    if (!pathTouched) {
      setPath(value.trim() ? `${profileRootPath}/${value}` : profileRootPath);
    }
  };

  const handleBrowse = async () => {
    try {
      const selected = await openDialog({ directory: true, multiple: false, defaultPath: path });
      if (selected && typeof selected === 'string') {
        setPath(selected);
        setPathTouched(true);
      }
    } catch (e) {
      console.error('Browse failed', e);
    }
  };

  const handleConfirm = async () => {
    if (!name.trim() || !path.trim() || creating) return;
    setError('');
    setCreating(true);
    try {
      await onCreate(path.trim(), parentId ? Number(parentId) : null);
      onClose();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setCreating(false);
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
        onKeyDown={e => {
          if (e.key === 'Escape') onClose();
          if (e.key === 'Enter') handleConfirm();
        }}
        style={{
          width: 420, background: 'var(--bg-surface)',
          border: '1px solid var(--border)', borderRadius: 12,
          boxShadow: '0 16px 48px rgba(0,0,0,0.2)', padding: 20,
        }}
      >
        <div style={{ fontSize: 15, fontWeight: 600, color: 'var(--text-primary)', marginBottom: 16 }}>
          New Project
        </div>

        <label style={{ display: 'block', fontSize: 12, color: 'var(--text-muted)', marginBottom: 4 }}>
          Name
        </label>
        <input
          autoFocus
          placeholder="My project"
          value={name}
          onChange={e => handleNameChange(e.target.value)}
          style={{
            width: '100%', padding: '7px 10px', background: 'var(--bg-app)',
            border: '1px solid var(--border)', borderRadius: 4, fontSize: 13,
            color: 'var(--text-primary)', outline: 'none', boxSizing: 'border-box',
            marginBottom: 12,
          }}
        />

        <label style={{ display: 'block', fontSize: 12, color: 'var(--text-muted)', marginBottom: 4 }}>
          Path
        </label>
        <div style={{ display: 'flex', gap: 6, marginBottom: 12 }}>
          <input
            value={path}
            onChange={e => { setPath(e.target.value); setPathTouched(true); }}
            style={{
              flex: 1, padding: '7px 10px', background: 'var(--bg-app)',
              border: '1px solid var(--border)', borderRadius: 4, fontSize: 12,
              color: 'var(--text-primary)', outline: 'none', boxSizing: 'border-box',
              fontFamily: 'JetBrains Mono, monospace',
            }}
          />
          <button
            onClick={handleBrowse}
            title="Choose folder"
            style={{
              width: 32, flexShrink: 0, background: 'none', border: '1px solid var(--border)',
              borderRadius: 4, color: 'var(--text-muted)', cursor: 'pointer',
              display: 'flex', alignItems: 'center', justifyContent: 'center',
            }}
          >
            <FolderIcon size={14} />
          </button>
        </div>

        {topLevelProjects.length > 0 && (
          <>
            <label style={{ display: 'block', fontSize: 12, color: 'var(--text-muted)', marginBottom: 4 }}>
              Parent project (optional)
            </label>
            <select
              value={parentId}
              onChange={e => setParentId(e.target.value)}
              style={{
                width: '100%', padding: '7px 10px', background: 'var(--bg-app)',
                border: '1px solid var(--border)', borderRadius: 4, fontSize: 13,
                color: 'var(--text-primary)', outline: 'none', boxSizing: 'border-box',
                marginBottom: 12,
              }}
            >
              <option value="">None — top-level project</option>
              {topLevelProjects.map(p => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
          </>
        )}

        {error && (
          <div style={{ fontSize: 12, color: 'var(--error)', marginBottom: 12 }}>{error}</div>
        )}

        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
          <button
            onClick={onClose}
            style={{
              padding: '7px 14px', background: 'none', border: '1px solid var(--border)',
              borderRadius: 4, color: 'var(--text-muted)', fontSize: 13, cursor: 'pointer',
            }}
          >
            Cancel
          </button>
          <button
            onClick={handleConfirm}
            disabled={!name.trim() || !path.trim() || creating}
            style={{
              padding: '7px 14px', background: 'var(--accent)', border: 'none',
              borderRadius: 4, color: '#fff', fontSize: 13, cursor: 'pointer',
              opacity: (!name.trim() || !path.trim() || creating) ? 0.5 : 1,
            }}
          >
            {creating ? 'Creating…' : 'Create'}
          </button>
        </div>
      </div>
    </div>
  );
}
