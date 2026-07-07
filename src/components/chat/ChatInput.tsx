import { useState, useRef, useEffect } from 'react';
import { Send, Square, ChevronDown, Lock, X } from 'lucide-react';
import { useChatStore } from '../../stores/chatStore';
import { useUIStore } from '../../stores/uiStore';
import { useProfileStore } from '../../stores/profileStore';
import { MODEL_OPTIONS } from '../../lib/types';
import * as api from '../../lib/tauri';
import type { KeyStatus } from '../../lib/types';
import { ProviderBadge } from './ProviderBadge';
import { PermissionDropdown } from './PermissionDropdown';
import { SwitchModelModal } from './SwitchModelModal';
import { messagesSinceCompression } from '../../lib/conversation';

interface Props {
  conversationId?: number;
  workspaceId?: number;
  autoFocus?: boolean;
}

export function ChatInput({ workspaceId, autoFocus }: Props) {
  const [input, setInput] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const streaming = useChatStore(s => s.streaming);
  const activeConversation = useChatStore(s => s.activeConversation);
  const messages = useChatStore(s => s.messages);
  const sendMessage = useChatStore(s => s.sendMessage);
  const startConversationAndSend = useChatStore(s => s.startConversationAndSend);
  const cancelStreaming = useChatStore(s => s.cancelStreaming);
  const messageQueue = useChatStore(s => s.messageQueue);
  const queueMessage = useChatStore(s => s.queueMessage);
  const removeQueuedMessage = useChatStore(s => s.removeQueuedMessage);
  const provider = useUIStore(s => s.selectedProvider);
  const model = useUIStore(s => s.selectedModel);
  const setModel = useUIStore(s => s.setModel);
  const compressConversation = useChatStore(s => s.compressConversation);
  const [modelMenuOpen, setModelMenuOpen] = useState(false);
  const [switchModalOpen, setSwitchModalOpen] = useState(false);
  const [connectedProviders, setConnectedProviders] = useState<Set<string>>(new Set());
  const activeProfile = useProfileStore(s => s.active);
  const newChatIntent = useUIStore(s => s.newChatIntent);
  const searchFocusIntent = useUIStore(s => s.searchFocusIntent);

  // The model picker is only editable before a session's first message. Once
  // the conversation has at least one message, the model is locked for its
  // duration — always read from the conversation's own stored provider/model
  // rather than the (mutable) global picker, so it can't drift mid-session.
  //
  // Compressing a session (see ChatView's "Compress session" button) is
  // meant to let the user switch models and keep chatting in the same
  // thread, so the lock only looks at messages sent *since* the
  // compression point — right after compressing there are none yet, so
  // the picker re-opens; it locks again once the first post-compression
  // message goes out, same as an ordinary new conversation.
  const isLockedSession = !!activeConversation && messagesSinceCompression(activeConversation, messages) > 0;
  const effectiveProvider = isLockedSession ? (activeConversation!.provider ?? provider) : provider;
  const effectiveModel = isLockedSession ? (activeConversation!.model ?? model) : model;

  useEffect(() => {
    const load = activeProfile?.id != null
      ? api.keyListStatusProfile(activeProfile.id)
      : api.keyListStatus();
    load.then((statuses: KeyStatus[]) => {
      setConnectedProviders(new Set(statuses.filter(s => s.exists).map(s => s.provider)));
    }).catch(() => {});
  }, [activeProfile?.id]);

  useEffect(() => {
    textareaRef.current?.focus();
  }, [newChatIntent, searchFocusIntent]);

  const visibleModels = MODEL_OPTIONS.filter(m => connectedProviders.has(m.provider));
  // Once locked, show the conversation's own model even if its provider is no
  // longer connected — search the full option list, not just visible ones.
  const currentModel = (isLockedSession ? MODEL_OPTIONS : visibleModels)
    .find(m => m.id === effectiveModel && m.provider === effectiveProvider);
  const canSend = isLockedSession || visibleModels.length > 0;

  // Default-select a connected model before a session has started. Once
  // locked, the picker no longer drives what gets sent, so there's nothing
  // to default.
  useEffect(() => {
    if (isLockedSession) return;
    const stillConnected = visibleModels.some(m => m.id === model && m.provider === provider);
    if (!stillConnected && visibleModels.length > 0) {
      setModel(visibleModels[0].provider, visibleModels[0].id);
    }
  }, [isLockedSession, model, provider, visibleModels, setModel]);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      const h = Math.min(textareaRef.current.scrollHeight, 120);
      textareaRef.current.style.height = `${h}px`;
    }
  }, [input]);

  // Once a conversation exists, a response in progress no longer blocks
  // composing — the draft is queued and sent automatically (in order) once
  // the current exchange, including any auto-continuations, settles.
  const canQueue = workspaceId === undefined;

  const handleSend = async () => {
    if (!input.trim() || !canSend) return;
    const content = input.trim();

    if (streaming) {
      if (canQueue) {
        queueMessage(content);
        setInput('');
      }
      return;
    }

    setInput('');
    if (workspaceId !== undefined) {
      await startConversationAndSend(workspaceId, content, effectiveProvider, effectiveModel);
    } else {
      await sendMessage(content, effectiveProvider, effectiveModel);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && ((e.metaKey || e.ctrlKey) || !e.shiftKey)) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div style={{
      borderTop: '1px solid var(--border)',
      padding: '12px 24px',
      background: 'var(--bg-app)',
      flexShrink: 0,
    }}>
      {messageQueue.length > 0 && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 4, marginBottom: 8 }}>
          {messageQueue.map((queued, i) => (
            <div
              key={i}
              style={{
                display: 'flex', alignItems: 'center', gap: 8,
                padding: '6px 10px', background: 'var(--overlay)', borderRadius: 6,
                fontSize: 12, color: 'var(--text-muted)',
              }}
            >
              <span style={{ flexShrink: 0, color: 'var(--text-muted)' }}>Queued:</span>
              <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', color: 'var(--text-primary)' }}>
                {queued}
              </span>
              <button
                onClick={() => removeQueuedMessage(i)}
                title="Remove from queue"
                style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', padding: 2, flexShrink: 0 }}
              >
                <X size={12} />
              </button>
            </div>
          ))}
        </div>
      )}
      <div style={{
        display: 'flex', gap: 8, alignItems: 'flex-end',
        background: 'var(--bg-surface)', border: '1px solid var(--border)',
        borderRadius: 8, padding: '6px 8px 6px 12px',
      }}>
        {/* Model selector — editable before a session starts, locked read-only once it has messages */}
        <div style={{ position: 'relative', flexShrink: 0 }}>
          <button
            onClick={() => { if (isLockedSession) setSwitchModalOpen(true); else setModelMenuOpen(o => !o); }}
            title={isLockedSession ? 'Model is locked for this session — click to switch' : undefined}
            style={{
              background: 'var(--overlay)', border: 'none', borderRadius: 4,
              padding: '4px 8px', cursor: 'pointer', fontSize: 11,
              color: 'var(--text-muted)', display: 'flex', alignItems: 'center', gap: 4,
              whiteSpace: 'nowrap', opacity: isLockedSession ? 0.85 : 1,
            }}
          >
            {currentModel && <ProviderBadge provider={currentModel.provider} size={13} />}
            {visibleModels.length === 0 && !isLockedSession ? 'No model connected' : (currentModel?.name ?? effectiveModel)}
            {isLockedSession ? <Lock size={10} /> : <ChevronDown size={11} />}
          </button>
          {!isLockedSession && modelMenuOpen && (
            <div style={{
              position: 'absolute', bottom: '100%', left: 0,
              background: 'var(--bg-surface)', border: '1px solid var(--border)',
              borderRadius: 8, boxShadow: '0 4px 16px rgba(0,0,0,0.12)',
              zIndex: 200, minWidth: 180, overflow: 'hidden', marginBottom: 4,
            }}>
              {visibleModels.length === 0 && (
                <div style={{ padding: '10px 12px', fontSize: 11, color: 'var(--text-muted)', maxWidth: 220 }}>
                  No connected providers. Add an API key in Settings.
                </div>
              )}
              {visibleModels.map(m => (
                <button
                  key={`${m.provider}:${m.id}`}
                  onClick={() => { setModel(m.provider, m.id); setModelMenuOpen(false); }}
                  style={{
                    width: '100%', padding: '7px 12px',
                    background: m.id === model && m.provider === provider ? 'var(--overlay)' : 'none',
                    border: 'none', cursor: 'pointer', textAlign: 'left',
                    fontSize: 12, color: 'var(--text-primary)',
                    display: 'flex', alignItems: 'center', gap: 8,
                  }}
                >
                  <ProviderBadge provider={m.provider} />
                  <span style={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
                    <span>{m.name}</span>
                    <span style={{ fontSize: 10, color: 'var(--text-muted)' }}>{m.provider}</span>
                  </span>
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Permission level */}
        <PermissionDropdown />

        {/* Textarea */}
        <textarea
          ref={textareaRef}
          value={input}
          onChange={e => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={streaming && canQueue ? 'Message... (queued until the current reply finishes)' : 'Message... (Enter to send, Shift+Enter for new line)'}
          rows={1}
          disabled={(streaming && !canQueue) || !canSend}
          autoFocus={autoFocus}
          style={{
            flex: 1, background: 'none', border: 'none', outline: 'none',
            resize: 'none', fontSize: 14, color: 'var(--text-primary)',
            lineHeight: 1.5, minHeight: 24,
            fontFamily: 'inherit',
          }}
        />

        {/* Send / Stop */}
        <button
          onClick={streaming ? cancelStreaming : handleSend}
          disabled={!streaming && (!input.trim() || !canSend)}
          style={{
            width: 32, height: 32, borderRadius: 4,
            background: streaming ? 'var(--error)' : input.trim() ? 'var(--accent)' : 'var(--overlay)',
            border: 'none', cursor: streaming || input.trim() ? 'pointer' : 'default',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            flexShrink: 0, transition: 'background 120ms ease-out',
          }}
        >
          {streaming
            ? <Square size={14} color="#fff" />
            : <Send size={14} color={input.trim() ? '#fff' : 'var(--text-muted)'} />
          }
        </button>
      </div>

      {switchModalOpen && activeConversation && (
        <SwitchModelModal
          visibleModels={visibleModels}
          currentProvider={activeConversation.provider ?? ''}
          currentModel={activeConversation.model ?? ''}
          onClose={() => setSwitchModalOpen(false)}
          onConfirm={async (newProvider, newModel) => {
            await compressConversation(activeConversation.id, newProvider, newModel);
            // The session briefly unlocks right after compressing (zero
            // messages since the new compression point) — until the next
            // message locks it again, effectiveProvider/effectiveModel
            // above fall through to this global picker state, not the
            // conversation's own (now-updated) provider/model. Without
            // this, the very next send would silently go out on whatever
            // stale model the global picker still held, undoing the
            // switch the user just made.
            setModel(newProvider, newModel);
          }}
        />
      )}
    </div>
  );
}
