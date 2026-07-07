import { useState, useEffect } from 'react';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { homeDir, join } from '@tauri-apps/api/path';
import { useProfileStore } from '../../stores/profileStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { errorMessage } from '../../lib/tauri';

interface Props {
  onDone: () => void;
}

function deriveDir(name: string): string {
  // "Arthur" → "Arthurs Atelier" (no apostrophe, filesystem safe)
  const safe = name.trim().replace(/[^a-zA-Z0-9 ]/g, '').trim();
  return `${safe}s Atelier`;
}

export function ProfileSetup({ onDone }: Props) {
  const [step, setStep] = useState<'name' | 'location' | 'confirm'>('name');
  const [name, setName] = useState('');
  const [dirName, setDirName] = useState('');
  const [rootPath, setRootPath] = useState('');
  const [locationMode, setLocationMode] = useState<'new' | 'existing'>('new');
  const [folderName, setFolderName] = useState('');
  const [parentPath, setParentPath] = useState('');
  const [existingPath, setExistingPath] = useState('');
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState('');
  const createProfile = useProfileStore(s => s.create);
  const switchProfile = useProfileStore(s => s.switch);
  const loadWorkspaces = useWorkspaceStore(s => s.load);

  useEffect(() => {
    homeDir()
      .then(h => join(h, 'Documents'))
      .then(setParentPath)
      .catch(() => setParentPath('/'));
  }, []);

  const handleNameNext = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    const dir = deriveDir(name);
    setDirName(dir);
    setFolderName(dir);
    setStep('location');
  };

  const handleBrowseParent = async () => {
    try {
      const selected = await openDialog({ directory: true, multiple: false });
      if (selected && typeof selected === 'string') {
        setParentPath(selected);
      }
    } catch (e) {
      console.error('Browse failed', e);
    }
  };

  const handleBrowseExisting = async () => {
    try {
      const selected = await openDialog({ directory: true, multiple: false });
      if (selected && typeof selected === 'string') {
        setExistingPath(selected);
      }
    } catch (e) {
      console.error('Browse failed', e);
    }
  };

  const locationContinueDisabled =
    locationMode === 'new'
      ? !folderName || !parentPath
      : !existingPath;

  const handleLocationNext = async () => {
    if (locationContinueDisabled) return;
    let resolved: string;
    if (locationMode === 'new') {
      let resolvedParent = parentPath.trim();
      if (resolvedParent.startsWith('~')) {
        try {
          const home = await homeDir();
          resolvedParent = await join(home, resolvedParent.slice(1).replace(/^\/+/, ''));
        } catch {
          // fall back to literal value if home dir resolution fails
        }
      }
      resolved = await join(resolvedParent, folderName.trim());
    } else {
      resolved = existingPath;
    }
    setRootPath(resolved);
    setStep('confirm');
  };

  const handleConfirm = async (e: React.FormEvent) => {
    e.preventDefault();
    setCreating(true);
    setError('');
    try {
      const profile = await createProfile(name.trim(), dirName, rootPath);
      await switchProfile(profile.id);
      await loadWorkspaces(profile.id);
      onDone();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setCreating(false);
    }
  };

  const cardStyle = (selected: boolean): React.CSSProperties => ({
    flex: 1,
    padding: '14px 16px',
    background: 'var(--bg-app)',
    border: `1px solid ${selected ? 'var(--accent)' : 'var(--border)'}`,
    borderRadius: 6,
    cursor: 'pointer',
    textAlign: 'left',
  });

  return (
    <div style={{
      position: 'fixed', inset: 0, background: 'var(--bg-app)',
      display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000,
    }}>
      <div style={{
        width: 480, background: 'var(--bg-surface)',
        border: '1px solid var(--border)', borderRadius: 8,
        padding: 32, boxShadow: '0 8px 32px rgba(0,0,0,0.12)',
      }}>
        <h2 style={{ margin: '0 0 8px', fontSize: 20, fontWeight: 600, color: 'var(--text-primary)' }}>
          Welcome to Open Atelier
        </h2>
        <p style={{ margin: '0 0 24px', color: 'var(--text-muted)', fontSize: 14, lineHeight: 1.5 }}>
          {step === 'name'
            ? 'Create your profile to get started. Everything stays local on your machine.'
            : step === 'location'
            ? 'Choose where to store your workspace folder.'
            : 'Review your profile details before creating.'}
        </p>

        {step === 'name' && (
          <form onSubmit={handleNameNext}>
            <label style={{ display: 'block', marginBottom: 6, fontSize: 13, fontWeight: 500, color: 'var(--text-primary)' }}>
              Your name
            </label>
            <input
              autoFocus
              value={name}
              onChange={e => setName(e.target.value)}
              placeholder="e.g. Arthur"
              style={{
                width: '100%', padding: '9px 12px',
                background: 'var(--bg-app)', border: '1px solid var(--border)',
                borderRadius: 4, fontSize: 14, color: 'var(--text-primary)',
                outline: 'none', boxSizing: 'border-box',
              }}
              onFocus={e => (e.target.style.borderColor = 'var(--accent)')}
              onBlur={e => (e.target.style.borderColor = 'var(--border)')}
            />
            <button
              type="submit"
              disabled={!name.trim()}
              style={{
                marginTop: 16, width: '100%', padding: '10px',
                background: name.trim() ? 'var(--accent)' : 'var(--overlay)',
                border: 'none', borderRadius: 4,
                color: name.trim() ? '#fff' : 'var(--text-muted)',
                fontSize: 14, fontWeight: 500, cursor: name.trim() ? 'pointer' : 'default',
              }}
            >
              Continue
            </button>
          </form>
        )}

        {step === 'location' && (
          <div>
            <div style={{ display: 'flex', gap: 12, marginBottom: 16 }}>
              {/* Card A: Create new folder */}
              <div style={cardStyle(locationMode === 'new')} onClick={() => setLocationMode('new')}>
                <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-primary)', marginBottom: 10 }}>
                  Create new folder
                </div>
                <label style={{ fontSize: 11, color: 'var(--text-muted)', display: 'block', marginBottom: 4 }}>
                  Folder name
                </label>
                <input
                  value={folderName}
                  onChange={e => setFolderName(e.target.value)}
                  onClick={e => { e.stopPropagation(); setLocationMode('new'); }}
                  style={{
                    width: '100%', padding: '7px 10px', marginBottom: 8,
                    background: 'var(--bg-surface)', border: '1px solid var(--border)',
                    borderRadius: 4, fontSize: 12, color: 'var(--text-primary)',
                    outline: 'none', boxSizing: 'border-box',
                  }}
                />
                <label style={{ fontSize: 11, color: 'var(--text-muted)', display: 'block', marginBottom: 4 }}>
                  Parent folder
                </label>
                <div style={{ display: 'flex', gap: 6 }}>
                  <input
                    value={parentPath}
                    onChange={e => setParentPath(e.target.value)}
                    onClick={e => { e.stopPropagation(); setLocationMode('new'); }}
                    style={{
                      flex: 1, padding: '7px 10px',
                      background: 'var(--bg-surface)', border: '1px solid var(--border)',
                      borderRadius: 4, fontSize: 11, color: 'var(--text-primary)',
                      outline: 'none', boxSizing: 'border-box',
                      fontFamily: 'JetBrains Mono, monospace',
                    }}
                  />
                  <button
                    type="button"
                    onClick={e => { e.stopPropagation(); setLocationMode('new'); handleBrowseParent(); }}
                    style={{
                      padding: '6px 10px', background: 'none', border: '1px solid var(--border)',
                      borderRadius: 4, color: 'var(--text-muted)', fontSize: 11, cursor: 'pointer', flexShrink: 0,
                    }}
                  >
                    Browse
                  </button>
                </div>
              </div>

              {/* Card B: Use existing folder */}
              <div style={cardStyle(locationMode === 'existing')} onClick={() => setLocationMode('existing')}>
                <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-primary)', marginBottom: 10 }}>
                  Use existing folder
                </div>
                <button
                  type="button"
                  onClick={e => { e.stopPropagation(); setLocationMode('existing'); handleBrowseExisting(); }}
                  style={{
                    padding: '7px 12px', background: 'none', border: '1px solid var(--border)',
                    borderRadius: 4, color: 'var(--text-muted)', fontSize: 12, cursor: 'pointer', marginBottom: 8,
                  }}
                >
                  Browse…
                </button>
                {existingPath && (
                  <div style={{
                    fontSize: 11, color: 'var(--text-primary)', marginTop: 4,
                    fontFamily: 'JetBrains Mono, monospace',
                    wordBreak: 'break-all',
                  }}>
                    {existingPath}
                  </div>
                )}
              </div>
            </div>

            <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
              <button
                type="button"
                onClick={() => setStep('name')}
                style={{
                  flex: 1, padding: '10px',
                  background: 'none', border: '1px solid var(--border)',
                  borderRadius: 4, color: 'var(--text-muted)', fontSize: 14, cursor: 'pointer',
                }}
              >
                Back
              </button>
              <button
                type="button"
                onClick={handleLocationNext}
                disabled={locationContinueDisabled}
                style={{
                  flex: 2, padding: '10px',
                  background: 'var(--accent)', border: 'none',
                  borderRadius: 4, color: '#fff', fontSize: 14, fontWeight: 500,
                  cursor: locationContinueDisabled ? 'default' : 'pointer',
                  opacity: locationContinueDisabled ? 0.5 : 1,
                }}
              >
                Continue
              </button>
            </div>
          </div>
        )}

        {step === 'confirm' && (
          <form onSubmit={handleConfirm}>
            <div style={{ marginBottom: 16, fontSize: 13, color: 'var(--text-primary)', lineHeight: 1.8 }}>
              <div><span style={{ color: 'var(--text-muted)' }}>Name: </span>{name}</div>
              <div style={{ fontFamily: 'JetBrains Mono, monospace', fontSize: 12 }}>
                <span style={{ color: 'var(--text-muted)', fontFamily: 'inherit' }}>Folder: </span>{rootPath}
              </div>
            </div>
            {error && (
              <div style={{ marginBottom: 8, fontSize: 12, color: 'var(--error)' }}>{error}</div>
            )}
            <div style={{ display: 'flex', gap: 8 }}>
              <button
                type="button"
                onClick={() => setStep('location')}
                style={{
                  flex: 1, padding: '10px',
                  background: 'none', border: '1px solid var(--border)',
                  borderRadius: 4, color: 'var(--text-muted)', fontSize: 14, cursor: 'pointer',
                }}
              >
                Back
              </button>
              <button
                type="submit"
                disabled={creating}
                style={{
                  flex: 2, padding: '10px',
                  background: 'var(--accent)', border: 'none',
                  borderRadius: 4, color: '#fff', fontSize: 14, fontWeight: 500,
                  cursor: creating ? 'wait' : 'pointer',
                  opacity: creating ? 0.7 : 1,
                }}
              >
                {creating ? 'Creating…' : 'Create profile'}
              </button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}
