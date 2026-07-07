import { useState, useRef, useEffect, useMemo } from 'react';
import { Search, MessageSquare, FolderIcon } from 'lucide-react';
import { useUIStore } from '../../stores/uiStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useChatStore } from '../../stores/chatStore';

interface SearchResult {
  type: 'workspace' | 'conversation';
  id: number;
  label: string;
  detail?: string;
  workspaceId?: number;
}

export function SearchOverlay() {
  const searchOpen = useUIStore(s => s.searchOpen);
  const setSearchOpen = useUIStore(s => s.setSearchOpen);
  const workspaces = useWorkspaceStore(s => s.workspaces);
  const setActiveWorkspace = useWorkspaceStore(s => s.setActive);
  const conversations = useChatStore(s => s.conversations);
  const openConversation = useChatStore(s => s.openConversation);
  const closeConversation = useChatStore(s => s.closeConversation);
  const loadConversations = useChatStore(s => s.loadConversations);

  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  // Reset the query/selection whenever the overlay transitions to open.
  // Adjusted during render (rather than in an effect) since this component
  // never unmounts — App.tsx always renders it — so there's no mount point
  // to reset state at otherwise.
  const [wasOpen, setWasOpen] = useState(searchOpen);
  if (searchOpen !== wasOpen) {
    setWasOpen(searchOpen);
    if (searchOpen) {
      setQuery('');
      setSelectedIndex(0);
    }
  }

  useEffect(() => {
    if (searchOpen) {
      const id = setTimeout(() => inputRef.current?.focus(), 0);
      return () => clearTimeout(id);
    }
  }, [searchOpen]);

  const results = useMemo(() => {
    const q = query.toLowerCase().trim();
    const items: SearchResult[] = [];
    const wsIds = new Set(workspaces.map(w => w.id));

    for (const ws of workspaces) {
      if (!q || ws.name.toLowerCase().includes(q) || ws.path.toLowerCase().includes(q)) {
        items.push({ type: 'workspace', id: ws.id, label: ws.name, detail: ws.path });
      }
    }

    for (const conv of conversations) {
      if (!wsIds.has(conv.workspace_id)) continue;
      if (!q || conv.title.toLowerCase().includes(q)) {
        items.push({
          type: 'conversation', id: conv.id, label: conv.title,
          detail: new Date(conv.updated_at).toLocaleDateString(),
          workspaceId: conv.workspace_id,
        });
      }
    }

    return items.slice(0, 20);
  }, [query, workspaces, conversations]);

  const handleSelect = (result: SearchResult) => {
    setSearchOpen(false);
    if (result.type === 'workspace') {
      const ws = workspaces.find(w => w.id === result.id);
      if (ws) {
        closeConversation();
        setActiveWorkspace(ws);
        loadConversations(ws.id);
      }
    } else {
      openConversation(result.id);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIndex(i => Math.min(i + 1, results.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIndex(i => Math.max(i - 1, 0));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (results[selectedIndex]) handleSelect(results[selectedIndex]);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      setSearchOpen(false);
    }
  };

  if (!searchOpen) return null;

  return (
    <div
      onClick={() => setSearchOpen(false)}
      style={{
        position: 'fixed', inset: 0, zIndex: 500,
        background: 'rgba(0,0,0,0.4)', display: 'flex',
        justifyContent: 'center', paddingTop: 100,
      }}
    >
      <div
        onClick={e => e.stopPropagation()}
        style={{
          width: 520, maxHeight: 420, background: 'var(--bg-surface)',
          border: '1px solid var(--border)', borderRadius: 12,
          boxShadow: '0 16px 48px rgba(0,0,0,0.2)',
          display: 'flex', flexDirection: 'column', overflow: 'hidden',
        }}
      >
        <div style={{
          display: 'flex', alignItems: 'center', gap: 10,
          padding: '12px 16px', borderBottom: '1px solid var(--border)',
        }}>
          <Search size={16} color="var(--text-muted)" />
          <input
            ref={inputRef}
            value={query}
            onChange={e => { setQuery(e.target.value); setSelectedIndex(0); }}
            onKeyDown={handleKeyDown}
            placeholder="Search projects and conversations..."
            style={{
              flex: 1, background: 'none', border: 'none', outline: 'none',
              fontSize: 15, color: 'var(--text-primary)', fontFamily: 'inherit',
            }}
          />
          <kbd style={{
            padding: '2px 6px', borderRadius: 4, fontSize: 10,
            background: 'var(--overlay)', color: 'var(--text-muted)',
            border: '1px solid var(--border)',
          }}>
            ESC
          </kbd>
        </div>

        <div style={{ flex: 1, overflow: 'auto' }}>
          {results.length === 0 && (
            <div style={{ padding: '24px 16px', textAlign: 'center', color: 'var(--text-muted)', fontSize: 13 }}>
              No results found
            </div>
          )}
          {results.map((r, i) => (
            <button
              key={`${r.type}-${r.id}`}
              onClick={() => handleSelect(r)}
              onMouseEnter={() => setSelectedIndex(i)}
              style={{
                width: '100%', padding: '10px 16px',
                background: i === selectedIndex ? 'var(--overlay)' : 'none',
                border: 'none', cursor: 'pointer', textAlign: 'left',
                display: 'flex', alignItems: 'center', gap: 10,
              }}
            >
              {r.type === 'workspace'
                ? <FolderIcon size={14} color="var(--accent)" />
                : <MessageSquare size={14} color="var(--text-muted)" />
              }
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{
                  fontSize: 13, color: 'var(--text-primary)', fontWeight: 500,
                  overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                }}>
                  {r.label}
                </div>
                {r.detail && (
                  <div style={{
                    fontSize: 11, color: 'var(--text-muted)',
                    overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                  }}>
                    {r.detail}
                  </div>
                )}
              </div>
              <span style={{
                fontSize: 10, color: 'var(--text-muted)',
                padding: '2px 6px', background: 'var(--overlay)',
                borderRadius: 4, flexShrink: 0,
              }}>
                {r.type === 'workspace' ? 'Project' : 'Chat'}
              </span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
