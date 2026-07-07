import { useRef, useEffect, useState } from 'react';
import { ArrowLeft, Archive, Check, ChevronDown, ChevronRight } from 'lucide-react';
import { confirm as confirmDialog } from '@tauri-apps/plugin-dialog';
import { useChatStore } from '../../stores/chatStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useUIStore } from '../../stores/uiStore';
import { MessageBubble } from './MessageBubble';
import { ChatInput } from './ChatInput';
import { ToolCallCard } from './ToolCallCard';
import { PermissionBadge } from './PermissionBadge';
import { TriggerErrorBlock } from './TriggerErrorBlock';
import { usePermissionStore } from '../../stores/permissionStore';
import { messagesSinceCompression } from '../../lib/conversation';

export function ChatView() {
  const activeConversation = useChatStore(s => s.activeConversation);
  const messages = useChatStore(s => s.messages);
  const messageCitations = useChatStore(s => s.messageCitations);
  const pendingToolCalls = useChatStore(s => s.pendingToolCalls);
  const closeConversation = useChatStore(s => s.closeConversation);
  const renameConversation = useChatStore(s => s.renameConversation);
  const compressConversation = useChatStore(s => s.compressConversation);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState('');
  const [compressing, setCompressing] = useState(false);
  const [memoryExpanded, setMemoryExpanded] = useState(false);
  const activeWorkspace = useWorkspaceStore(s => s.active);
  const provider = useUIStore(s => s.selectedProvider);
  const model = useUIStore(s => s.selectedModel);
  const triggerResults = usePermissionStore(s => s.triggerResults);
  const triggerErrors = usePermissionStore(s => s.triggerErrors);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  if (!activeConversation) return null;

  const handleTitleSave = async () => {
    if (titleDraft && titleDraft !== activeConversation.title) {
      await renameConversation(activeConversation.id, titleDraft);
    }
    setEditingTitle(false);
  };

  // Summarizes everything so far into a memory block and marks the
  // compression point, so future turns send that memory plus only the
  // messages sent after it — letting the user keep chatting in this same
  // thread, optionally switching to a different provider/model first via
  // the composer's own selector below.
  const handleCompress = async () => {
    if (compressing) return;
    const confirmed = await confirmDialog(
      'This summarizes the conversation so far into a memory and continues from there, keeping the same model — useful to shrink a long thread. To switch models instead, use the locked model picker in the composer below.',
      { title: 'Compress this session?' },
    );
    if (!confirmed) return;
    setCompressing(true);
    try {
      // Compress in place with whatever model this conversation is already
      // using — switching models is a separate, guided flow (see
      // SwitchModelModal, opened from the composer's locked picker), not
      // this button's job. Falls back to the global new-chat picker only
      // for the edge case of a conversation with no provider/model set yet.
      await compressConversation(activeConversation.id, activeConversation.provider ?? provider, activeConversation.model ?? model);
    } catch (e) {
      console.error('Failed to compress session', e);
    } finally {
      setCompressing(false);
    }
  };

  const justCompressed = !!activeConversation.compressed_at && messagesSinceCompression(activeConversation, messages) === 0;

  return (
    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
      {/* Breadcrumb */}
      <div style={{
        padding: '0 24px', height: 48,
        borderBottom: '1px solid var(--border)',
        display: 'flex', alignItems: 'center', gap: 8, flexShrink: 0,
      }}>
        <button
          onClick={closeConversation}
          style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', padding: '4px 0', display: 'flex', alignItems: 'center', gap: 4, fontSize: 13 }}
        >
          <ArrowLeft size={14} />
          {activeWorkspace?.name}
        </button>
        <span style={{ color: 'var(--border)', fontSize: 13 }}>/</span>
        {editingTitle ? (
          <input
            autoFocus
            value={titleDraft}
            onChange={e => setTitleDraft(e.target.value)}
            onBlur={handleTitleSave}
            onKeyDown={e => {
              if (e.key === 'Enter') handleTitleSave();
              if (e.key === 'Escape') setEditingTitle(false);
            }}
            style={{
              background: 'none', border: 'none', outline: '1px solid var(--accent)',
              borderRadius: 2, padding: '2px 4px', fontSize: 14, fontWeight: 500,
              color: 'var(--text-primary)', minWidth: 120,
            }}
          />
        ) : (
          <>
            <span
              onClick={() => { setTitleDraft(activeConversation.title); setEditingTitle(true); }}
              style={{ fontSize: 14, fontWeight: 500, color: 'var(--text-primary)', cursor: 'text' }}
            >
              {activeConversation.title}
            </span>
            <PermissionBadge />
          </>
        )}
        <div style={{ flex: 1 }} />
        <button
          onClick={handleCompress}
          disabled={compressing || justCompressed}
          title={justCompressed ? 'Already compressed — nothing new to fold in yet' : 'Summarize this session into a memory and continue from there'}
          style={{
            display: 'flex', alignItems: 'center', gap: 4, flexShrink: 0,
            padding: '4px 8px', borderRadius: 4, fontSize: 12, border: 'none',
            cursor: (compressing || justCompressed) ? 'default' : 'pointer',
            background: justCompressed ? 'rgba(61, 122, 90, 0.15)' : 'var(--overlay)',
            color: justCompressed ? 'var(--success)' : compressing ? 'var(--text-muted)' : 'var(--text-primary)',
          }}
        >
          {justCompressed ? <Check size={12} /> : <Archive size={12} />}
          {compressing ? 'Compressing…' : justCompressed ? 'Compressed' : 'Compress session'}
        </button>
      </div>

      {/* Messages */}
      <div
        ref={scrollRef}
        style={{ flex: 1, overflow: 'auto', padding: '24px 0' }}
      >
        {activeConversation.compressed_at && (
          <div style={{ margin: '0 24px 12px' }}>
            <button
              onClick={() => setMemoryExpanded(v => !v)}
              style={{
                display: 'flex', alignItems: 'center', gap: 6, width: '100%',
                background: 'none', border: 'none', cursor: 'pointer', padding: '6px 0',
                color: 'var(--text-muted)', fontSize: 12, borderTop: '1px dashed var(--border)',
                borderBottom: '1px dashed var(--border)',
              }}
            >
              {memoryExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
              <Archive size={12} />
              Session compressed — memory carried forward
            </button>
            {memoryExpanded && (
              <div style={{
                marginTop: 6, padding: '8px 10px', borderRadius: 4, background: 'var(--overlay)',
                fontSize: 12, color: 'var(--text-muted)', whiteSpace: 'pre-wrap',
              }}>
                {activeConversation.compressed_memory}
              </div>
            )}
          </div>
        )}
        {messages.filter(m => m.role !== 'system').map((msg, i) => (
          <div key={msg.id}>
            <MessageBubble message={msg} citations={messageCitations[msg.id]} />
            {/* Inline tool calls after assistant messages */}
            {msg.role === 'assistant' && pendingToolCalls
              .filter(tc => tc.message_id === msg.id || (i === messages.length - 2 && tc.status === 'pending'))
              .map(tc => <ToolCallCard key={tc.id} toolCall={tc} />)
            }
          </div>
        ))}
      </div>

      {/* Trigger feedback */}
      {(triggerResults.length > 0 || triggerErrors.length > 0) && (
        <TriggerErrorBlock results={triggerResults} errors={triggerErrors} />
      )}

      {/* Input */}
      <ChatInput conversationId={activeConversation.id} />
    </div>
  );
}
