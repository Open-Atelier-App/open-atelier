import { useState, useRef, useEffect } from 'react';
import { Send, Square, ChevronDown, X, Pencil, FileIcon, FolderIcon, ImagePlus, SlashSquare } from 'lucide-react';
import { confirm as confirmDialog, message as messageDialog } from '@tauri-apps/plugin-dialog';
import { useChatStore } from '../../stores/chatStore';
import { useUIStore } from '../../stores/uiStore';
import { useWidgetStore } from '../../stores/widgetStore';
import { useProfileStore } from '../../stores/profileStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { MODEL_OPTIONS } from '../../lib/types';
import * as api from '../../lib/tauri';
import type { KeyStatus, FileNode } from '../../lib/types';
import { ProviderBadge } from './ProviderBadge';
import { PermissionDropdown } from './PermissionDropdown';
import { messagesSinceCompression } from '../../lib/conversation';

interface Props {
  conversationId?: number;
  workspaceId?: number;
  autoFocus?: boolean;
}

function draftKeyFor(conversationId: number | undefined, workspaceId: number | undefined): string | null {
  if (conversationId != null) return `chat-draft:conversation:${conversationId}`;
  if (workspaceId !== undefined) return `chat-draft:workspace:${workspaceId}`;
  return null;
}

function flattenFileTree(nodes: FileNode[]): FileNode[] {
  const out: FileNode[] = [];
  for (const node of nodes) {
    out.push(node);
    if (node.children) out.push(...flattenFileTree(node.children));
  }
  return out;
}

interface ListLineMatch {
  lineStart: number;
  lineEnd: number;
  indent: string;
  marker: string;
  isOrdered: boolean;
  rest: string;
}

/** If the line the cursor is on is a markdown list item ("- ", "* ", "1. ",
 * optionally indented), returns its parts so Enter/Tab can continue or
 * indent it — plain-text equivalent of what a rich editor would call "list
 * mode," since the composer is a plain textarea, not contenteditable. */
function matchListLine(value: string, cursor: number): ListLineMatch | null {
  const lineStart = value.lastIndexOf('\n', cursor - 1) + 1;
  const nextNewline = value.indexOf('\n', cursor);
  const lineEnd = nextNewline === -1 ? value.length : nextNewline;
  const line = value.slice(lineStart, lineEnd);

  const bullet = /^(\s*)([-*])\s(.*)$/.exec(line);
  if (bullet) {
    return { lineStart, lineEnd, indent: bullet[1], marker: bullet[2], isOrdered: false, rest: bullet[3] };
  }
  const ordered = /^(\s*)(\d+)\.\s(.*)$/.exec(line);
  if (ordered) {
    return { lineStart, lineEnd, indent: ordered[1], marker: ordered[2], isOrdered: true, rest: ordered[3] };
  }
  return null;
}

/**
 * Finds the `@mention` (if any) the cursor is currently inside of: scans
 * back from the cursor to the nearest `@` that starts a word (preceded by
 * nothing, whitespace, or a newline), stopping early if whitespace is hit
 * first — a completed "@foo bar" isn't an active mention anymore once the
 * cursor has moved past the space.
 */
function activeMentionAt(text: string, cursor: number): { start: number; query: string } | null {
  let i = cursor - 1;
  while (i >= 0) {
    const ch = text[i];
    if (ch === '@') {
      const prev = text[i - 1];
      if (i === 0 || prev === ' ' || prev === '\n') {
        return { start: i, query: text.slice(i + 1, cursor) };
      }
      return null;
    }
    if (ch === ' ' || ch === '\n') return null;
    i--;
  }
  return null;
}

/** Reads a File as base64, stripping the "data:image/png;base64," prefix the FileReader data URL includes. */
function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve((reader.result as string).split(',')[1] ?? '');
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

interface PendingAttachment {
  relPath: string;
  name: string;
  previewUrl: string;
}

interface SlashCommand {
  id: string;
  hint: string;
  run: () => void | Promise<void>;
}

export function ChatInput({ workspaceId, autoFocus }: Props) {
  // Lazy initializer (not a plain `useState('')`) so a draft left over from
  // before the app was last closed is restored on the very first render —
  // reading the store directly here since the `activeConversation` selector
  // below isn't available yet at this point in the component.
  const [input, setInput] = useState(() => {
    const key = draftKeyFor(useChatStore.getState().activeConversation?.id, workspaceId);
    return key ? localStorage.getItem(key) ?? '' : '';
  });
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
  const closeConversation = useChatStore(s => s.closeConversation);
  const forkConversation = useChatStore(s => s.forkConversation);
  const triggerRename = useUIStore(s => s.triggerRename);
  const setSearchOpen = useUIStore(s => s.setSearchOpen);
  const provider = useUIStore(s => s.selectedProvider);
  const model = useUIStore(s => s.selectedModel);
  const setModel = useUIStore(s => s.setModel);
  const compressConversation = useChatStore(s => s.compressConversation);
  const [modelMenuOpen, setModelMenuOpen] = useState(false);
  const [connectedProviders, setConnectedProviders] = useState<Set<string>>(new Set());
  const activeProfile = useProfileStore(s => s.active);
  const newChatIntent = useUIStore(s => s.newChatIntent);
  const searchFocusIntent = useUIStore(s => s.searchFocusIntent);
  const fileTree = useWorkspaceStore(s => s.fileTree);
  const [mention, setMention] = useState<{ start: number; query: string } | null>(null);
  const [mentionIndex, setMentionIndex] = useState(0);
  const [slashIndex, setSlashIndex] = useState(0);

  const mentionMatches = mention
    ? flattenFileTree(fileTree)
      .filter(f => f.rel_path.toLowerCase().includes(mention.query.toLowerCase()))
      .slice(0, 8)
    : [];

  const applyMention = (file: FileNode) => {
    if (!mention) return;
    const cursor = mention.start + 1 + mention.query.length;
    const next = `${input.slice(0, mention.start)}@${file.rel_path} ${input.slice(cursor)}`;
    setInput(next);
    setMention(null);
    // Restore focus + cursor right after the inserted path, on the next
    // tick once React has applied the new value.
    const newCursor = mention.start + file.rel_path.length + 2;
    requestAnimationFrame(() => {
      textareaRef.current?.focus();
      textareaRef.current?.setSelectionRange(newCursor, newCursor);
    });
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value);
    const next = activeMentionAt(e.target.value, e.target.selectionStart);
    setMention(next);
    setMentionIndex(0);
    setSlashIndex(0);
  };

  // Shortcuts to actions already reachable elsewhere in the UI (the title's
  // inline rename, ChatView's "Compress session" button, a message's fork
  // button, Cmd+K search) — not a general-purpose command system, just a
  // faster way to reach the same handful of actions from the keyboard.
  const baseSlashCommands: SlashCommand[] = [
    ...(activeConversation ? [{
      id: 'new', hint: 'Start a new conversation',
      run: () => closeConversation(),
    }] : []),
    ...(activeConversation ? [{
      id: 'rename', hint: 'Rename this conversation',
      run: () => triggerRename(),
    }] : []),
    ...(activeConversation && messagesSinceCompression(activeConversation, messages) > 0 ? [{
      id: 'compress', hint: 'Summarize into memory and continue',
      run: async () => {
        const confirmed = await confirmDialog(
          'This summarizes the conversation so far into a memory and continues from there, keeping the same model.',
          { title: 'Compress this session?' },
        );
        if (!confirmed) return;
        await compressConversation(activeConversation.id, activeConversation.provider ?? provider, activeConversation.model ?? model);
      },
    }] : []),
    ...(activeConversation && messages.length > 0 ? [{
      id: 'fork', hint: 'Fork a new chat from the last message',
      run: () => forkConversation(messages[messages.length - 1].id),
    }] : []),
    ...(activeConversation ? [{
      id: 'timer', hint: 'Start a stopwatch',
      run: () => useWidgetStore.getState().addWidget(activeConversation.id, 'timer'),
    }] : []),
    ...(activeConversation ? [{
      id: 'countdown', hint: 'Start a countdown timer',
      run: () => useWidgetStore.getState().addWidget(activeConversation.id, 'countdown'),
    }] : []),
    {
      id: 'search', hint: 'Search conversations',
      run: () => setSearchOpen(true),
    },
  ];
  const slashCommands: SlashCommand[] = [
    ...baseSlashCommands,
    {
      id: 'help', hint: 'List available commands',
      run: async () => {
        await messageDialog(
          baseSlashCommands.map(c => `/${c.id} — ${c.hint}`).join('\n'),
          { title: 'Chat commands', kind: 'info' },
        );
      },
    },
  ];

  // Only while the whole message is still just a bare "/word" — once a
  // space is typed the command is considered settled (or abandoned), same
  // as how the @mention picker gives up once whitespace follows the @.
  const activeSlash = !mention && input.startsWith('/') && !/\s/.test(input) ? input.slice(1) : null;
  const slashMatches = activeSlash !== null
    ? slashCommands.filter(c => c.id.startsWith(activeSlash.toLowerCase()))
    : [];

  const runSlashCommand = (cmd: SlashCommand) => {
    setInput('');
    Promise.resolve(cmd.run()).catch(e => console.error(`Slash command /${cmd.id} failed`, e));
  };

  // Images are uploaded (saved into the workspace's attachments/ folder)
  // as soon as they're dropped/pasted/picked, not deferred until send —
  // that way the preview thumbnail is backed by a real file right away,
  // and send just has to reference paths already known to exist.
  const [attachments, setAttachments] = useState<PendingAttachment[]>([]);
  const [attaching, setAttaching] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const effectiveWorkspaceId = activeConversation?.workspace_id ?? workspaceId;

  const attachImages = async (files: File[]) => {
    const images = files.filter(f => f.type.startsWith('image/'));
    if (images.length === 0 || effectiveWorkspaceId === undefined) return;
    setAttaching(true);
    try {
      for (const file of images) {
        const base64Data = await fileToBase64(file);
        const saved = await api.imageAttachmentSave(effectiveWorkspaceId, file.name || 'image.png', base64Data);
        setAttachments(prev => [...prev, {
          relPath: saved.rel_path,
          name: file.name || saved.rel_path,
          previewUrl: URL.createObjectURL(file),
        }]);
      }
    } catch (e) {
      console.error('Failed to attach image', e);
    } finally {
      setAttaching(false);
    }
  };

  const removeAttachment = (relPath: string) => {
    setAttachments(prev => {
      const removed = prev.find(a => a.relPath === relPath);
      if (removed) URL.revokeObjectURL(removed.previewUrl);
      return prev.filter(a => a.relPath !== relPath);
    });
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    attachImages(Array.from(e.dataTransfer.files));
  };

  const handlePaste = (e: React.ClipboardEvent) => {
    const files = Array.from(e.clipboardData.items)
      .filter(item => item.kind === 'file' && item.type.startsWith('image/'))
      .map(item => item.getAsFile())
      .filter((f): f is File => f !== null);
    if (files.length > 0) {
      e.preventDefault();
      attachImages(files);
    }
  };

  // The model picker is freely editable at any point in a conversation, not
  // just before its first message — switching models mid-thread sends the
  // full existing history to whatever model is picked next, same as it
  // already does for a fresh conversation.
  //
  // Reopening an existing conversation should still show (and send with)
  // its own last-used model rather than whatever the global picker was
  // last set to from browsing some other chat — sync the picker to the
  // conversation's own provider/model whenever the active conversation
  // changes, without preventing the user from picking something else
  // afterward.
  const activeConversationId = activeConversation?.id;
  useEffect(() => {
    if (activeConversation?.provider && activeConversation?.model) {
      setModel(activeConversation.provider, activeConversation.model);
    }
    // Intentionally keyed on the conversation's id, not its provider/model:
    // this should re-sync exactly once per conversation switch, not fight
    // the user every time they pick something else from the dropdown
    // afterward (which also updates activeConversation.provider/model once
    // the next message goes out).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeConversationId]);

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

  // Persist the unsent draft per conversation (or per workspace, before a
  // conversation exists yet) so it survives switching to another chat and
  // back, and survives closing/reloading the app entirely — previously
  // `input` was a single piece of component state shared across whatever
  // conversation happened to be active, so switching chats mid-draft would
  // carry the wrong text over instead of losing it cleanly.
  const draftKey = draftKeyFor(activeConversation?.id, workspaceId);

  // Adjusting state during render (React's documented pattern for
  // "resetting state when a prop changes") instead of in an effect, since
  // an effect that turns around and calls setState immediately just
  // triggers an extra render for no benefit.
  const [loadedDraftKey, setLoadedDraftKey] = useState(draftKey);
  if (draftKey !== loadedDraftKey) {
    setLoadedDraftKey(draftKey);
    setInput(draftKey ? localStorage.getItem(draftKey) ?? '' : '');
  }

  useEffect(() => {
    if (!draftKey) return;
    if (input) localStorage.setItem(draftKey, input);
    else localStorage.removeItem(draftKey);
  }, [draftKey, input]);

  const visibleModels = MODEL_OPTIONS.filter(m => connectedProviders.has(m.provider));
  // Fall back to the full option list so a conversation's own model still
  // shows correctly even if its provider isn't currently connected.
  const currentModel = (visibleModels.find(m => m.id === model && m.provider === provider))
    ?? MODEL_OPTIONS.find(m => m.id === model && m.provider === provider);
  const canSend = visibleModels.length > 0;

  // Keep the picker on a connected model — if whatever's selected loses its
  // connection (key removed, etc.), fall back to the first one that works.
  useEffect(() => {
    const stillConnected = visibleModels.some(m => m.id === model && m.provider === provider);
    if (!stillConnected && visibleModels.length > 0) {
      setModel(visibleModels[0].provider, visibleModels[0].id);
    }
  }, [model, provider, visibleModels, setModel]);

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
    if ((!input.trim() && attachments.length === 0) || !canSend) return;
    // Images render inline via the same markdown-image path already used
    // elsewhere in the app, so attaching one is just prepending a
    // `![name](path)` reference — no separate message field/rendering path
    // needed for it to show up in the conversation.
    const imageLines = attachments.map(a => `![${a.name}](${a.relPath})`).join('\n');
    const content = [imageLines, input.trim()].filter(Boolean).join('\n\n');

    if (streaming) {
      if (canQueue) {
        queueMessage(content);
        setInput('');
        setAttachments([]);
      }
      return;
    }

    setInput('');
    setAttachments([]);
    if (workspaceId !== undefined) {
      await startConversationAndSend(workspaceId, content, provider, model);
    } else {
      await sendMessage(content, provider, model);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (activeSlash !== null && slashMatches.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSlashIndex(i => (i + 1) % slashMatches.length);
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSlashIndex(i => (i - 1 + slashMatches.length) % slashMatches.length);
        return;
      }
      if (e.key === 'Tab' || e.key === 'Enter') {
        e.preventDefault();
        runSlashCommand(slashMatches[slashIndex]);
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        setInput('');
        return;
      }
    }
    if (mention && mentionMatches.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setMentionIndex(i => (i + 1) % mentionMatches.length);
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        setMentionIndex(i => (i - 1 + mentionMatches.length) % mentionMatches.length);
        return;
      }
      if (e.key === 'Tab' || e.key === 'Enter') {
        e.preventDefault();
        applyMention(mentionMatches[mentionIndex]);
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        setMention(null);
        return;
      }
    }
    // List continuation/indentation only kicks in on the plain Enter/Tab
    // that would otherwise send the message or leave the field — Shift/Cmd
    // variants are left alone so a real newline or a forced send still work
    // inside a list.
    //
    // The cursor is repositioned synchronously on the DOM element itself,
    // not via a requestAnimationFrame callback after setInput — a deferred
    // fix loses the race against fast typing (real or scripted) that
    // continues right after this keydown, landing characters at the old
    // cursor position and scrambling the line. Writing `.value` directly
    // here is safe: setInput is called with that exact same string, so
    // React's next render is a no-op write that doesn't touch the
    // selection.
    if (e.key === 'Tab') {
      const textarea = e.currentTarget;
      const list = matchListLine(textarea.value, textarea.selectionStart);
      if (list) {
        e.preventDefault();
        if (e.shiftKey) {
          const removeCount = Math.min(2, list.indent.length);
          if (removeCount === 0) return;
          const next = textarea.value.slice(0, list.lineStart) + textarea.value.slice(list.lineStart + removeCount);
          const newCursor = Math.max(list.lineStart, textarea.selectionStart - removeCount);
          textarea.value = next;
          textarea.setSelectionRange(newCursor, newCursor);
          setInput(next);
        } else {
          const cursor = textarea.selectionStart;
          const next = textarea.value.slice(0, list.lineStart) + '  ' + textarea.value.slice(list.lineStart);
          const newCursor = cursor + 2;
          textarea.value = next;
          textarea.setSelectionRange(newCursor, newCursor);
          setInput(next);
        }
        return;
      }
    }

    if (e.key === 'Enter' && !e.shiftKey && !e.metaKey && !e.ctrlKey) {
      const textarea = e.currentTarget;
      const list = matchListLine(textarea.value, textarea.selectionStart);
      if (list) {
        e.preventDefault();
        if (list.rest.trim() === '') {
          // Empty list item: Enter exits the list (drops the marker) rather
          // than piling up empty bullets — the following Enter, now on a
          // genuinely empty line, sends as usual.
          const next = textarea.value.slice(0, list.lineStart) + textarea.value.slice(list.lineEnd);
          const newCursor = list.lineStart;
          textarea.value = next;
          textarea.setSelectionRange(newCursor, newCursor);
          setInput(next);
        } else {
          const nextMarker = list.isOrdered ? `${Number(list.marker) + 1}.` : list.marker;
          const insertion = `\n${list.indent}${nextMarker} `;
          const cursor = textarea.selectionStart;
          const next = textarea.value.slice(0, cursor) + insertion + textarea.value.slice(textarea.selectionEnd);
          const newCursor = cursor + insertion.length;
          textarea.value = next;
          textarea.setSelectionRange(newCursor, newCursor);
          setInput(next);
        }
        return;
      }
    }

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
      {/* Model + permission selectors sit in their own row above the
          composer bubble, not inside it — keeps the typing area free of
          controls and out from under the file viewer panel. */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
        <div style={{ position: 'relative', flexShrink: 0 }}>
          <button
            onClick={() => setModelMenuOpen(o => !o)}
            style={{
              background: 'var(--overlay)', border: 'none', borderRadius: 4,
              padding: '4px 8px', cursor: 'pointer', fontSize: 11,
              color: 'var(--text-muted)', display: 'flex', alignItems: 'center', gap: 4,
              whiteSpace: 'nowrap',
            }}
          >
            {currentModel && <ProviderBadge provider={currentModel.provider} size={13} />}
            {visibleModels.length === 0 ? 'No model connected' : (currentModel?.name ?? model)}
            <ChevronDown size={11} />
          </button>
          {modelMenuOpen && (
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
        <PermissionDropdown />
      </div>
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
                onClick={() => {
                  removeQueuedMessage(i);
                  setInput(queued);
                  textareaRef.current?.focus();
                }}
                title="Edit"
                style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', padding: 2, flexShrink: 0 }}
              >
                <Pencil size={12} />
              </button>
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
      {(attachments.length > 0 || attaching) && (
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginBottom: 8 }}>
          {attachments.map(a => (
            <div key={a.relPath} style={{ position: 'relative', flexShrink: 0 }}>
              <img
                src={a.previewUrl}
                alt={a.name}
                style={{ width: 56, height: 56, objectFit: 'cover', borderRadius: 6, border: '1px solid var(--border)' }}
              />
              <button
                onClick={() => removeAttachment(a.relPath)}
                title="Remove"
                style={{
                  position: 'absolute', top: -6, right: -6, width: 18, height: 18, borderRadius: '50%',
                  background: 'var(--bg-surface)', border: '1px solid var(--border)', cursor: 'pointer',
                  display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--text-muted)',
                }}
              >
                <X size={11} />
              </button>
            </div>
          ))}
          {attaching && (
            <div style={{
              width: 56, height: 56, borderRadius: 6, border: '1px dashed var(--border)',
              display: 'flex', alignItems: 'center', justifyContent: 'center',
              fontSize: 10, color: 'var(--text-muted)',
            }}>
              …
            </div>
          )}
        </div>
      )}
      <div
        onDragOver={e => e.preventDefault()}
        onDrop={handleDrop}
        style={{
          display: 'flex', gap: 8, alignItems: 'flex-end',
          background: 'var(--bg-surface)', border: '1px solid var(--border)',
          borderRadius: 8, padding: '6px 8px 6px 12px',
        }}>
        {/* Image attachment picker */}
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*"
          multiple
          onChange={e => {
            attachImages(Array.from(e.target.files ?? []));
            e.target.value = '';
          }}
          style={{ display: 'none' }}
        />
        <button
          onClick={() => fileInputRef.current?.click()}
          title="Attach an image"
          disabled={effectiveWorkspaceId === undefined}
          style={{
            width: 28, height: 28, borderRadius: 4, flexShrink: 0,
            background: 'none', border: 'none', cursor: effectiveWorkspaceId === undefined ? 'default' : 'pointer',
            color: 'var(--text-muted)', display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}
        >
          <ImagePlus size={16} />
        </button>

        {/* Textarea */}
        <div style={{ position: 'relative', flex: 1 }}>
          {activeSlash !== null && slashMatches.length > 0 && (
            <div style={{
              position: 'absolute', bottom: '100%', left: 0, marginBottom: 6,
              background: 'var(--bg-surface)', border: '1px solid var(--border)',
              borderRadius: 6, boxShadow: '0 4px 16px rgba(0,0,0,0.12)',
              minWidth: 220, maxWidth: 360, maxHeight: 240, overflow: 'auto', zIndex: 20,
            }}>
              {slashMatches.map((cmd, i) => (
                <button
                  key={cmd.id}
                  onClick={() => runSlashCommand(cmd)}
                  onMouseEnter={() => setSlashIndex(i)}
                  style={{
                    width: '100%', padding: '6px 10px', display: 'flex', alignItems: 'center', gap: 6,
                    background: i === slashIndex ? 'var(--overlay)' : 'none',
                    border: 'none', cursor: 'pointer', textAlign: 'left',
                  }}
                >
                  <SlashSquare size={13} color="var(--text-muted)" style={{ flexShrink: 0 }} />
                  <span style={{ display: 'flex', flexDirection: 'column', gap: 1, overflow: 'hidden' }}>
                    <span style={{ fontSize: 13, color: 'var(--text-primary)' }}>/{cmd.id}</span>
                    <span style={{ fontSize: 11, color: 'var(--text-muted)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {cmd.hint}
                    </span>
                  </span>
                </button>
              ))}
            </div>
          )}
          {mention && mentionMatches.length > 0 && (
            <div style={{
              position: 'absolute', bottom: '100%', left: 0, marginBottom: 6,
              background: 'var(--bg-surface)', border: '1px solid var(--border)',
              borderRadius: 6, boxShadow: '0 4px 16px rgba(0,0,0,0.12)',
              minWidth: 220, maxWidth: 360, maxHeight: 240, overflow: 'auto', zIndex: 20,
            }}>
              {mentionMatches.map((file, i) => (
                <button
                  key={file.rel_path}
                  onClick={() => applyMention(file)}
                  onMouseEnter={() => setMentionIndex(i)}
                  style={{
                    width: '100%', padding: '6px 10px', display: 'flex', alignItems: 'center', gap: 6,
                    background: i === mentionIndex ? 'var(--overlay)' : 'none',
                    border: 'none', cursor: 'pointer', textAlign: 'left',
                  }}
                >
                  {file.is_dir ? <FolderIcon size={13} color="var(--text-muted)" /> : <FileIcon size={13} color="var(--text-muted)" />}
                  <span style={{ fontSize: 13, color: 'var(--text-primary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {file.rel_path}
                  </span>
                </button>
              ))}
            </div>
          )}
          <textarea
            ref={textareaRef}
            value={input}
            onChange={handleInputChange}
            onKeyDown={handleKeyDown}
            onPaste={handlePaste}
            placeholder={streaming && canQueue ? 'Message... (queued until the current reply finishes)' : 'Message... (Enter to send, Shift+Enter for new line) — @ to reference a file, / for commands'}
            rows={1}
            disabled={(streaming && !canQueue) || !canSend}
            autoFocus={autoFocus}
            style={{
              width: '100%', background: 'none', border: 'none', outline: 'none',
              resize: 'none', fontSize: 14, color: 'var(--text-primary)',
              lineHeight: 1.5, minHeight: 24,
              fontFamily: 'inherit',
            }}
          />
        </div>

        {/* Send / Stop */}
        <button
          onClick={streaming ? cancelStreaming : handleSend}
          disabled={!streaming && ((!input.trim() && attachments.length === 0) || !canSend)}
          style={{
            width: 32, height: 32, borderRadius: 4,
            background: streaming ? 'var(--error)' : (input.trim() || attachments.length > 0) ? 'var(--accent)' : 'var(--overlay)',
            border: 'none', cursor: streaming || input.trim() || attachments.length > 0 ? 'pointer' : 'default',
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
    </div>
  );
}
