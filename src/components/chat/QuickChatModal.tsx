import { useEffect, useRef, useState } from 'react';
import { FolderIcon } from 'lucide-react';
import { useUIStore } from '../../stores/uiStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useRecentsStore } from '../../stores/recentsStore';
import { useActiveChatsStore } from '../../stores/activeChatsStore';
import * as api from '../../lib/tauri';

/**
 * Starts a new conversation without leaving whatever the user is currently
 * looking at: closing this modal doesn't touch chatStore's
 * activeConversation/messages/streaming state at all — those are exactly
 * what's driving the *visible* chat, and mutating them here would yank the
 * user over to this new conversation instead of leaving them where they
 * were. The create+send calls go straight to the backend; the exchange
 * streams and completes in the database like any other, just with nothing
 * subscribed to render it live. It shows up under Recent once sent, ready
 * to open whenever.
 */
export function QuickChatModal() {
  const quickChatOpen = useUIStore(s => s.quickChatOpen);
  const setQuickChatOpen = useUIStore(s => s.setQuickChatOpen);
  const workspaces = useWorkspaceStore(s => s.workspaces);
  const activeWorkspace = useWorkspaceStore(s => s.active);
  const selectedProvider = useUIStore(s => s.selectedProvider);
  const selectedModel = useUIStore(s => s.selectedModel);

  const [workspaceId, setWorkspaceId] = useState<number | null>(null);
  const [content, setContent] = useState('');
  const [sending, setSending] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const [wasOpen, setWasOpen] = useState(quickChatOpen);
  if (quickChatOpen !== wasOpen) {
    setWasOpen(quickChatOpen);
    if (quickChatOpen) {
      setContent('');
      setWorkspaceId(activeWorkspace?.id ?? workspaces[0]?.id ?? null);
    }
  }

  useEffect(() => {
    if (quickChatOpen) {
      const id = setTimeout(() => textareaRef.current?.focus(), 0);
      return () => clearTimeout(id);
    }
  }, [quickChatOpen]);

  if (!quickChatOpen) return null;

  const handleSend = async () => {
    const trimmed = content.trim();
    if (!trimmed || workspaceId == null || sending) return;
    setSending(true);
    // Close immediately — per the design, the modal disappears on send and
    // the exchange keeps going in the background rather than blocking here.
    setQuickChatOpen(false);
    try {
      const conv = await api.conversationCreate(workspaceId);
      useRecentsStore.getState().recordOpened({
        conversationId: conv.id,
        workspaceId,
        title: conv.title,
      });
      const assistantMsg = await api.ask(conv.id, trimmed, selectedProvider, selectedModel);
      useActiveChatsStore.getState().startStreaming(assistantMsg.id, {
        conversationId: conv.id,
        workspaceId,
        title: conv.title,
      });
    } catch (e) {
      console.error('Quick chat failed to send', e);
    } finally {
      setSending(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      setQuickChatOpen(false);
    }
  };

  return (
    <div
      onClick={() => setQuickChatOpen(false)}
      style={{
        position: 'fixed', inset: 0, zIndex: 500,
        background: 'rgba(0,0,0,0.4)', display: 'flex',
        justifyContent: 'center', alignItems: 'flex-start', paddingTop: 100,
      }}
    >
      <div
        onClick={e => e.stopPropagation()}
        style={{
          width: 520, background: 'var(--bg-surface)',
          border: '1px solid var(--border)', borderRadius: 12,
          boxShadow: '0 16px 48px rgba(0,0,0,0.2)',
          display: 'flex', flexDirection: 'column', overflow: 'hidden',
          flexShrink: 0,
        }}
      >
        <div style={{
          display: 'flex', alignItems: 'center', gap: 8,
          padding: '10px 16px', borderBottom: '1px solid var(--border)',
        }}>
          <FolderIcon size={13} color="var(--text-muted)" />
          <select
            value={workspaceId ?? ''}
            onChange={e => setWorkspaceId(Number(e.target.value))}
            style={{
              flex: 1, background: 'none', border: 'none', outline: 'none',
              fontSize: 13, color: 'var(--text-primary)', fontFamily: 'inherit', cursor: 'pointer',
            }}
          >
            {workspaces.map(ws => (
              <option key={ws.id} value={ws.id}>{ws.name}</option>
            ))}
          </select>
          <kbd style={{
            padding: '2px 6px', borderRadius: 4, fontSize: 10,
            background: 'var(--overlay)', color: 'var(--text-muted)',
            border: '1px solid var(--border)',
          }}>
            ESC
          </kbd>
        </div>

        <textarea
          ref={textareaRef}
          value={content}
          onChange={e => setContent(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Quick message... (Enter to send in the background, Shift+Enter for new line)"
          rows={3}
          style={{
            padding: '14px 16px', background: 'none', border: 'none', outline: 'none',
            resize: 'none', fontSize: 14, color: 'var(--text-primary)',
            lineHeight: 1.5, fontFamily: 'inherit',
          }}
        />
      </div>
    </div>
  );
}
