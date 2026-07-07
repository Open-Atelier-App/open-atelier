import type { Conversation, Message } from './types';

/**
 * Number of messages sent since the conversation's last compression point
 * (or all messages, if it's never been compressed). Used both to decide
 * whether the model picker should stay locked (see ChatInput) and whether
 * the "Compress session" button has anything new to fold in (see
 * ChatView) — right after compressing there are none yet, so the picker
 * re-opens; it locks again once the first post-compression message goes
 * out, same as an ordinary new conversation.
 */
export function messagesSinceCompression(conversation: Conversation | null, messages: Message[]): number {
  if (!conversation) return 0;
  return conversation.compressed_at
    ? messages.filter(m => m.created_at > conversation.compressed_at!).length
    : messages.length;
}
