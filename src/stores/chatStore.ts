import { create } from 'zustand';
import type { Conversation, Message, Citation, ToolCall } from '../lib/types';
import * as api from '../lib/tauri';
import { usePermissionStore } from './permissionStore';
import { usePlanStore } from './planStore';
import { useRecentsStore } from './recentsStore';
import { useActiveChatsStore } from './activeChatsStore';

interface ChatState {
  conversations: Conversation[];
  activeConversation: Conversation | null;
  messages: Message[];
  messageCitations: Record<number, Citation[]>;
  pendingToolCalls: ToolCall[];
  streaming: boolean;
  streamingMessageId: number | null;
  // Drafted while a response (including any auto-continuations) is still
  // streaming — sent automatically, in order, once it finishes.
  messageQueue: string[];
  error: string | null;

  loadConversations: (workspace_id: number) => Promise<void>;
  createConversation: (workspace_id: number, title?: string) => Promise<Conversation>;
  openConversation: (id: number) => Promise<void>;
  closeConversation: () => void;
  renameConversation: (id: number, title: string) => Promise<void>;
  compressConversation: (id: number, provider: string, model: string) => Promise<void>;
  forkConversation: (upToMessageId: number) => Promise<void>;
  deleteConversation: (id: number) => Promise<void>;
  archiveConversation: (id: number) => Promise<void>;
  setConversationGroup: (id: number, groupId: number | null) => Promise<void>;
  updateConversationTitle: (id: number, title: string) => void;
  updateConversationSummary: (id: number, summary: string) => void;

  sendMessage: (content: string, provider: string, model: string) => Promise<void>;
  startConversationAndSend: (workspace_id: number, content: string, provider: string, model: string) => Promise<void>;
  queueMessage: (content: string) => void;
  removeQueuedMessage: (index: number) => void;
  appendToken: (message_id: number, delta: string) => void;
  completeMessage: (message_id: number, citations?: Citation[], hasMore?: boolean, displayOverride?: string | null) => void;
  addMessage: (message: Message) => void;
  errorMessage: (message_id: number, error: string) => void;
  cancelStreaming: () => void;
  sendNextQueued: () => void;

  addToolCall: (tc: ToolCall) => void;
  approveToolCall: (id: number) => Promise<void>;
  rejectToolCall: (id: number) => Promise<void>;
}

export const useChatStore = create<ChatState>((set, get) => ({
  conversations: [],
  activeConversation: null,
  messages: [],
  messageCitations: {},
  pendingToolCalls: [],
  streaming: false,
  streamingMessageId: null,
  messageQueue: [],
  error: null,

  loadConversations: async (workspace_id) => {
    try {
      const conversations = await api.conversationList(workspace_id);
      set({ conversations });
    } catch (e) {
      set({ error: api.errorMessage(e) });
    }
  },

  createConversation: async (workspace_id, title) => {
    const conv = await api.conversationCreate(workspace_id, title);
    set(s => ({ conversations: [conv, ...s.conversations] }));
    return conv;
  },

  openConversation: async (id) => {
    try {
      const { conversation, messages } = await api.conversationGet(id);
      set({ activeConversation: conversation, messages, pendingToolCalls: [], messageCitations: {}, messageQueue: [] });
      useRecentsStore.getState().recordOpened({
        conversationId: conversation.id,
        workspaceId: conversation.workspace_id,
        title: conversation.title,
      });
      useActiveChatsStore.getState().markRead(conversation.id);
      // The action log (trigger results/parse errors) was only ever cleared
      // right before sending a *new* message — switching to a different
      // conversation without sending anything left the previous
      // conversation's (or even a different project's) actions on screen,
      // accumulating for the entire app session.
      usePermissionStore.getState().clearTriggerFeedback();
      usePlanStore.getState().loadForConversation(id);
    } catch (e) {
      set({ error: api.errorMessage(e) });
    }
  },

  closeConversation: () => {
    set({ activeConversation: null, messages: [], pendingToolCalls: [], streaming: false, streamingMessageId: null, messageQueue: [] });
    usePermissionStore.getState().clearTriggerFeedback();
    usePlanStore.getState().clear();
  },

  renameConversation: async (id, title) => {
    const conv = await api.conversationRename(id, title);
    set(s => ({
      conversations: s.conversations.map(c => c.id === id ? conv : c),
      activeConversation: s.activeConversation?.id === id ? conv : s.activeConversation,
    }));
    useRecentsStore.getState().renameEntry(id, title);
  },

  compressConversation: async (id, provider, model) => {
    const conv = await api.conversationCompress(id, provider, model);
    set(s => ({
      conversations: s.conversations.map(c => c.id === id ? conv : c),
      activeConversation: s.activeConversation?.id === id ? conv : s.activeConversation,
    }));
  },

  forkConversation: async (upToMessageId) => {
    const source = get().activeConversation;
    if (!source) return;
    const forked = await api.conversationFork(source.id, upToMessageId);
    set(s => ({ conversations: [forked, ...s.conversations] }));
    await get().openConversation(forked.id);
  },

  deleteConversation: async (id) => {
    await api.conversationDelete(id);
    const { activeConversation } = get();
    set(s => ({
      conversations: s.conversations.filter(c => c.id !== id),
      activeConversation: activeConversation?.id === id ? null : activeConversation,
      messages: activeConversation?.id === id ? [] : s.messages,
    }));
    useRecentsStore.getState().removeEntry(id);
  },

  archiveConversation: async (id) => {
    await api.conversationArchive(id);
    const { activeConversation } = get();
    set(s => ({
      conversations: s.conversations.filter(c => c.id !== id),
      activeConversation: activeConversation?.id === id ? null : activeConversation,
      messages: activeConversation?.id === id ? [] : s.messages,
    }));
    useRecentsStore.getState().removeEntry(id);
  },

  setConversationGroup: async (id, groupId) => {
    const conv = await api.conversationSetGroup(id, groupId);
    set(s => ({
      conversations: s.conversations.map(c => c.id === id ? conv : c),
      activeConversation: s.activeConversation?.id === id ? conv : s.activeConversation,
    }));
  },

  updateConversationTitle: (id, title) => {
    set(s => ({
      conversations: s.conversations.map(c => c.id === id ? { ...c, title } : c),
      activeConversation: s.activeConversation?.id === id
        ? { ...s.activeConversation, title }
        : s.activeConversation,
    }));
    useRecentsStore.getState().renameEntry(id, title);
  },

  updateConversationSummary: (id, summary) => {
    set(s => ({
      conversations: s.conversations.map(c => c.id === id ? { ...c, summary } : c),
      activeConversation: s.activeConversation?.id === id
        ? { ...s.activeConversation, summary }
        : s.activeConversation,
    }));
  },

  // Used by the new-chat composer: there's no conversation yet, so create one
  // (using the message itself, not a separate title field) and immediately
  // send the first message into it.
  startConversationAndSend: async (workspace_id, content, provider, model) => {
    const conv = await get().createConversation(workspace_id);
    set({ activeConversation: conv, messages: [], pendingToolCalls: [], messageCitations: {} });
    useRecentsStore.getState().recordOpened({
      conversationId: conv.id,
      workspaceId: conv.workspace_id,
      title: conv.title,
    });
    await get().sendMessage(content, provider, model);
  },

  sendMessage: async (content, provider, model) => {
    const { activeConversation } = get();
    if (!activeConversation) return;

    // Optimistic user message (temp id)
    const tempUserId = -Date.now();
    const tempAssistantId = -(Date.now() + 1);

    const userMsg: Message = {
      id: tempUserId,
      conversation_id: activeConversation.id,
      role: 'user',
      content,
      created_at: Date.now(),
      token_count: null,
      input_tokens: null,
      output_tokens: null,
      error: null,
      status: 'complete',
      provider: null,
      model: null,
      display_override: null,
    };

    const assistantMsg: Message = {
      id: tempAssistantId,
      conversation_id: activeConversation.id,
      role: 'assistant',
      content: '',
      created_at: Date.now() + 1,
      token_count: null,
      input_tokens: null,
      output_tokens: null,
      error: null,
      status: 'streaming',
      provider,
      model,
      display_override: null,
    };

    usePermissionStore.getState().clearTriggerFeedback();
    set(s => ({
      messages: [...s.messages, userMsg, assistantMsg],
      streaming: true,
      streamingMessageId: tempAssistantId,
      error: null,
    }));

    try {
      // ask() returns the real assistant message (with real id) immediately
      // The actual content streams via chat://token events using the real id
      const realMsg = await api.ask(activeConversation.id, content, provider, model);
      useActiveChatsStore.getState().startStreaming(realMsg.id, {
        conversationId: activeConversation.id,
        workspaceId: activeConversation.workspace_id,
        title: activeConversation.title,
      });

      // Replace temp assistant message with real one (now streaming from backend)
      set(s => ({
        messages: s.messages.map(m =>
          m.id === tempAssistantId ? { ...realMsg, content: '', status: 'streaming' } :
          m.id === tempUserId ? { ...m, id: realMsg.id - 1 } :
          m
        ),
        streamingMessageId: realMsg.id,
      }));
    } catch (e) {
      set(s => ({
        messages: s.messages.map(m =>
          m.id === tempAssistantId ? { ...m, status: 'error', error: api.errorMessage(e) } : m
        ),
        streaming: false,
        streamingMessageId: null,
        error: api.errorMessage(e),
      }));
    }
  },

  queueMessage: (content) => {
    set(s => ({ messageQueue: [...s.messageQueue, content] }));
  },

  removeQueuedMessage: (index) => {
    set(s => ({ messageQueue: s.messageQueue.filter((_, i) => i !== index) }));
  },

  appendToken: (message_id, delta) => {
    set(s => ({
      messages: s.messages.map(m =>
        m.id === message_id ? { ...m, content: m.content + delta } : m
      ),
    }));
  },

  completeMessage: (message_id, citations, hasMore, displayOverride) => {
    const wasStreamingThis = get().streamingMessageId === message_id;
    set(s => ({
      messages: s.messages.map(m =>
        m.id === message_id ? { ...m, status: 'complete', display_override: displayOverride ?? null } : m
      ),
      messageCitations: citations?.length
        ? { ...s.messageCitations, [message_id]: citations }
        : s.messageCitations,
      // When a READ/LIST triggers an automatic continuation, a new
      // assistant message is already on its way (see addMessage) — don't
      // clear the streaming indicator, that next message is still coming.
      streaming: (wasStreamingThis && !hasMore) ? false : s.streaming,
      streamingMessageId: (wasStreamingThis && !hasMore) ? null : s.streamingMessageId,
    }));
    if (wasStreamingThis && !hasMore) {
      get().sendNextQueued();
    }
  },

  addMessage: (message) => {
    set(s => ({
      messages: [...s.messages, message],
      streaming: true,
      streamingMessageId: message.id,
    }));
  },

  errorMessage: (message_id, error) => {
    set(s => ({
      messages: s.messages.map(m =>
        m.id === message_id ? { ...m, status: 'error', error } : m
      ),
      streaming: false,
      streamingMessageId: null,
    }));
    get().sendNextQueued();
  },

  cancelStreaming: () => {
    const { streamingMessageId } = get();
    if (streamingMessageId) {
      set(s => ({
        messages: s.messages.map(m =>
          m.id === streamingMessageId ? { ...m, status: 'cancelled' } : m
        ),
        streaming: false,
        streamingMessageId: null,
      }));
      // A cancel never gets a matching chat://done or chat://error event
      // (the backend keeps streaming in the background; the frontend just
      // stops listening), so without this the conversation stayed marked
      // "active" in the sidebar forever — it looked resolved everywhere
      // except there.
      useActiveChatsStore.getState().finishStreaming(streamingMessageId);
      get().sendNextQueued();
    }
  },

  // Dequeues and sends the next drafted message once the previous
  // exchange has fully settled.
  sendNextQueued: () => {
    const { messageQueue, activeConversation } = get();
    if (messageQueue.length === 0 || !activeConversation) return;
    const [next, ...rest] = messageQueue;
    set({ messageQueue: rest });
    get().sendMessage(next, activeConversation.provider ?? '', activeConversation.model ?? '');
  },

  addToolCall: (tc) => {
    set(s => ({ pendingToolCalls: [...s.pendingToolCalls, tc] }));
  },

  approveToolCall: async (id) => {
    const tc = await api.toolApprove(id);
    set(s => ({
      pendingToolCalls: s.pendingToolCalls.map(t => t.id === id ? tc : t),
    }));
  },

  rejectToolCall: async (id) => {
    const tc = await api.toolReject(id);
    set(s => ({
      pendingToolCalls: s.pendingToolCalls.map(t => t.id === id ? tc : t),
    }));
  },
}));
