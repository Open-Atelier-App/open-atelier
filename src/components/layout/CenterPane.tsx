import { useState } from 'react';
import { Sparkles } from 'lucide-react';
import { confirm as confirmDialog, message as messageDialog } from '@tauri-apps/plugin-dialog';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useChatStore } from '../../stores/chatStore';
import { ChatView } from '../chat/ChatView';
import { ChatInput } from '../chat/ChatInput';
import { EmptyState } from '../onboarding/EmptyState';
import { ConversationList } from '../workspace/ConversationList';
import * as api from '../../lib/tauri';

export function CenterPane() {
  const activeWorkspace = useWorkspaceStore(s => s.active);
  const renameWorkspace = useWorkspaceStore(s => s.rename);
  const setWorkspaceDescription = useWorkspaceStore(s => s.setDescription);
  const conversations = useChatStore(s => s.conversations);
  const activeConversation = useChatStore(s => s.activeConversation);
  const openConversation = useChatStore(s => s.openConversation);
  const [suggesting, setSuggesting] = useState(false);
  const [editingDescription, setEditingDescription] = useState(false);
  const [descriptionDraft, setDescriptionDraft] = useState('');

  if (!activeWorkspace) {
    return <EmptyState />;
  }

  if (activeConversation) {
    return <ChatView />;
  }

  const handleMagicRename = async () => {
    if (suggesting) return;
    setSuggesting(true);
    try {
      const suggested = await api.workspaceSuggestName(activeWorkspace.id);
      if (suggested && await confirmDialog(`Rename project to "${suggested}"?`)) {
        await renameWorkspace(activeWorkspace.id, suggested);
      }
    } catch (e) {
      console.error('Magic rename failed', e);
      await messageDialog(api.errorMessage(e), { title: 'Could not suggest a name', kind: 'error' });
    } finally {
      setSuggesting(false);
    }
  };

  const startEditingDescription = () => {
    setDescriptionDraft(activeWorkspace.description ?? '');
    setEditingDescription(true);
  };

  const saveDescription = async () => {
    setEditingDescription(false);
    if (descriptionDraft.trim() === (activeWorkspace.description ?? '')) return;
    try {
      await setWorkspaceDescription(activeWorkspace.id, descriptionDraft);
    } catch (e) {
      console.error('Failed to save project description', e);
    }
  };

  return (
    <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
      {/* Header */}
      <div style={{
        padding: '20px 24px 16px',
        borderBottom: '1px solid var(--border)',
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <h1 style={{ margin: 0, fontSize: 18, fontWeight: 600, color: 'var(--text-primary)' }}>
            {activeWorkspace.name}
          </h1>
          <button
            onClick={handleMagicRename}
            disabled={suggesting}
            title="Suggest a name using AI"
            style={{
              background: 'none', border: 'none', cursor: suggesting ? 'wait' : 'pointer',
              color: suggesting ? 'var(--accent)' : 'var(--text-muted)',
              padding: 4, borderRadius: 4, display: 'flex', alignItems: 'center',
              opacity: suggesting ? 0.6 : 1,
              transition: 'color 150ms',
            }}
          >
            <Sparkles size={16} style={suggesting ? { animation: 'spin 1s linear infinite' } : undefined} />
          </button>
        </div>

        {editingDescription ? (
          <input
            autoFocus
            value={descriptionDraft}
            onChange={e => setDescriptionDraft(e.target.value)}
            onBlur={saveDescription}
            onKeyDown={e => {
              if (e.key === 'Enter') saveDescription();
              if (e.key === 'Escape') setEditingDescription(false);
            }}
            placeholder="One-sentence description…"
            style={{
              marginTop: 4, width: '100%', maxWidth: 480, padding: '2px 0',
              background: 'none', border: 'none', borderBottom: '1px solid var(--border)',
              fontSize: 13, color: 'var(--text-primary)', outline: 'none',
            }}
          />
        ) : (
          <p
            onClick={startEditingDescription}
            title="Click to edit"
            style={{
              margin: '4px 0 0', fontSize: 13, color: 'var(--text-muted)',
              cursor: 'pointer', maxWidth: 480,
              overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
            }}
          >
            {activeWorkspace.description || 'Add a description…'}
          </p>
        )}
      </div>

      {/* Conversation list */}
      <ConversationList
        workspaceId={activeWorkspace.id}
        conversations={conversations}
        onSelect={conv => openConversation(conv.id)}
      />

      {/* New chat composer — sending the first message creates the conversation */}
      <ChatInput workspaceId={activeWorkspace.id} autoFocus />
    </div>
  );
}
