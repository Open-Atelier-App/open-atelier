import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useChatStore } from '../../stores/chatStore';

export function EmptyState() {
  const openWorkspace = useWorkspaceStore(s => s.open);
  const setActiveWorkspace = useWorkspaceStore(s => s.setActive);
  const loadConversations = useChatStore(s => s.loadConversations);

  const handleOpen = async () => {
    const selected = await openDialog({ directory: true, multiple: false });
    if (selected && typeof selected === 'string') {
      const ws = await openWorkspace(selected);
      setActiveWorkspace(ws);
      loadConversations(ws.id);
    }
  };

  return (
    <div style={{
      flex: 1, display: 'flex', flexDirection: 'column',
      alignItems: 'center', justifyContent: 'center',
      gap: 32, padding: 40,
    }}>
      <div style={{ textAlign: 'center' }}>
        <h2 style={{ margin: '0 0 8px', fontSize: 22, fontWeight: 600, color: 'var(--text-primary)' }}>
          Open Atelier
        </h2>
        <p style={{ margin: 0, color: 'var(--text-muted)', fontSize: 14 }}>
          A local-first AI workspace. Your files, your keys, your machine.
        </p>
      </div>

      <button
        onClick={handleOpen}
        style={{
          padding: '10px 24px', background: 'var(--accent)',
          border: 'none', borderRadius: 4, color: '#fff',
          fontSize: 14, fontWeight: 500, cursor: 'pointer',
        }}
      >
        Create new project
      </button>

      {/* Keyboard reference */}
      <div style={{
        background: 'var(--bg-surface)', border: '1px solid var(--border)',
        borderRadius: 8, padding: '16px 20px', maxWidth: 320,
      }}>
        <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-muted)', marginBottom: 10, textTransform: 'uppercase', letterSpacing: '0.06em' }}>
          Keyboard shortcuts
        </div>
        {[
          ['⌘N', 'New chat'],
          ['⌘⇧N', 'Quick chat'],
          ['⌘T', 'New tab'],
          ['⌘K', 'Search'],
          ['⌘,', 'Settings'],
          ['⌘[', 'Toggle sidebar'],
          ['⌘]', 'Toggle right bar'],
          ['⌘↩', 'Send message'],
          ['⌘\\', 'File viewer'],
          ['Esc', 'Close / cancel'],
        ].map(([key, label]) => (
          <div key={key} style={{ display: 'flex', justifyContent: 'space-between', padding: '3px 0', fontSize: 13 }}>
            <code style={{
              fontFamily: 'JetBrains Mono, monospace', fontSize: 12,
              background: 'var(--overlay)', padding: '1px 6px', borderRadius: 3,
              color: 'var(--text-primary)',
            }}>{key}</code>
            <span style={{ color: 'var(--text-muted)' }}>{label}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
