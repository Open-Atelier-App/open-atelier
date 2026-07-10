import { create } from 'zustand';
import { useChatStore } from './chatStore';

export interface ActiveChatEntry {
  conversationId: number;
  workspaceId: number;
  title: string;
}

interface ActiveChatsState {
  // Conversations currently streaming a response, across every project —
  // not just the one being looked at — so a background exchange (e.g.
  // Quick Chat, or a queued follow-up in a chat that isn't the active one)
  // still shows up somewhere.
  streaming: Map<number, ActiveChatEntry>;
  // Conversations that finished streaming while they weren't the one being
  // viewed — cleared once the user actually opens them.
  unread: Map<number, ActiveChatEntry>;
  // Internal: resolves a message id back to the conversation it belongs
  // to, since the chat://done and chat://error events only carry a
  // message_id, not a conversation_id.
  messageConversation: Map<number, number>;

  startStreaming: (messageId: number, entry: ActiveChatEntry) => void;
  // Maps an additional message id (e.g. a READ/LIST auto-continuation's
  // new message) to a conversation that's already registered in
  // `streaming`, so finishStreaming can resolve it later without needing
  // the full entry again.
  registerContinuationMessage: (messageId: number, conversationId: number) => void;
  finishStreaming: (messageId: number) => void;
  markRead: (conversationId: number) => void;
}

export const useActiveChatsStore = create<ActiveChatsState>((set, get) => ({
  streaming: new Map(),
  unread: new Map(),
  messageConversation: new Map(),

  startStreaming: (messageId, entry) => {
    set(s => {
      const messageConversation = new Map(s.messageConversation).set(messageId, entry.conversationId);
      const streaming = new Map(s.streaming).set(entry.conversationId, entry);
      return { messageConversation, streaming };
    });
  },

  registerContinuationMessage: (messageId, conversationId) => {
    set(s => ({ messageConversation: new Map(s.messageConversation).set(messageId, conversationId) }));
  },

  finishStreaming: (messageId) => {
    const conversationId = get().messageConversation.get(messageId);
    if (conversationId === undefined) return;
    set(s => {
      const messageConversation = new Map(s.messageConversation);
      messageConversation.delete(messageId);
      const streaming = new Map(s.streaming);
      const entry = streaming.get(conversationId);
      streaming.delete(conversationId);

      // Only worth flagging "unread" if it finished somewhere the user
      // wasn't already looking.
      const isCurrentlyViewed = useChatStore.getState().activeConversation?.id === conversationId;
      const unread = new Map(s.unread);
      if (entry && !isCurrentlyViewed) {
        unread.set(conversationId, entry);
      }
      return { messageConversation, streaming, unread };
    });
  },

  markRead: (conversationId) => {
    set(s => {
      if (!s.unread.has(conversationId)) return s;
      const unread = new Map(s.unread);
      unread.delete(conversationId);
      return { unread };
    });
  },
}));
