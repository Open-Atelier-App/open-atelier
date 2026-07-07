import { useState, type ReactNode } from 'react';
import ReactMarkdown, { defaultUrlTransform } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { open as openShell } from '@tauri-apps/plugin-shell';
import { confirm as confirmDialog } from '@tauri-apps/plugin-dialog';
import { Copy, Check, User, Bot, Loader2, GitFork, ThumbsUp, ThumbsDown } from 'lucide-react';
import { fileTypeIcon } from '../../lib/fileIcons';
import type { Message, Citation } from '../../lib/types';
import { useUIStore } from '../../stores/uiStore';
import { useChatStore } from '../../stores/chatStore';
import { CitationList } from './CitationList';
import { ProviderBadge } from './ProviderBadge';
import { stripTriggers, getInFlightAction } from '../../lib/triggerStrip';
import { relativeTime } from '../../lib/time';

// Fake link scheme the backend's synthesize_confirmation (see
// commands::chat.rs) uses for file paths in its auto-generated "Created
// x, edited y." fallback message, so a mention there is clickable the same
// way the action log's "Created" rows already are — not a real URL.
const ATELIER_FILE_SCHEME = 'atelier-file:';

const IN_PROGRESS_LABELS: Record<string, string> = {
  CREATE: 'Creating',
  WRITE: 'Writing',
  INSERT: 'Editing',
  APPEND: 'Editing',
  DELETE: 'Deleting',
  RENAME: 'Renaming',
  READ: 'Reading',
  PREVIEW: 'Opening',
  LIST: 'Listing',
};

interface Props {
  message: Message;
  citations?: Citation[];
}

function CopyButton({ text, style }: { text: string; style?: React.CSSProperties }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch (e) {
      console.error('Copy failed', e);
    }
  };

  return (
    <button
      onClick={handleCopy}
      title={copied ? 'Copied!' : 'Copy'}
      style={{
        background: 'var(--bg-surface)', border: '1px solid var(--border)',
        borderRadius: 4, padding: '3px 6px', cursor: 'pointer',
        color: copied ? 'var(--success)' : 'var(--text-muted)',
        display: 'flex', alignItems: 'center', gap: 4, fontSize: 11,
        ...style,
      }}
    >
      {copied ? <Check size={11} /> : <Copy size={11} />}
    </button>
  );
}

// react-markdown's default urlTransform only allows a fixed protocol
// allowlist (http/https/mailto/etc) and blanks out anything else, which
// silently strips our custom "atelier-file:" scheme before it ever reaches
// the `a` component's href — let it through, delegate everything else to
// the default sanitizer so real links keep the same XSS protection.
function urlTransform(url: string) {
  return url.startsWith(ATELIER_FILE_SCHEME) ? url : defaultUrlTransform(url);
}

function MarkdownContent({ content }: { content: string }) {
  const openFileViewer = useUIStore(s => s.openFileViewer);

  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      urlTransform={urlTransform}
      components={{
        // Thematic breaks ("---") render as an unstyled, jarring
        // browser-default <hr> inside a chat bubble — suppress them
        // rather than trying to make a heavy rule look at home here.
        hr: () => null,
        a: ({ children, href }) => (
          <a
            href={href}
            onClick={(e) => {
              e.preventDefault();
              if (!href) return;
              if (href.startsWith(ATELIER_FILE_SCHEME)) {
                // The markdown pipeline percent-encodes the link
                // destination (e.g. a space becomes "%20") since it treats
                // it as a URI — decode it back to a real path before
                // handing it to the file viewer, or any file with a space
                // or other encodable character in its name would 404.
                openFileViewer(decodeURIComponent(href.slice(ATELIER_FILE_SCHEME.length)));
              } else {
                openShell(href).catch((err: unknown) => console.error('Failed to open link', err));
              }
            }}
            style={{ color: 'var(--accent)', cursor: 'pointer', textDecoration: 'underline' }}
          >
            {children}
          </a>
        ),
        code({ children, className }) {
          const isBlock = className?.startsWith('language-');
          if (isBlock) {
            return (
              <pre style={{
                background: 'var(--bg-surface)', border: '1px solid var(--border)',
                borderRadius: 4, padding: '10px 12px', overflow: 'auto',
                fontFamily: 'JetBrains Mono, monospace', fontSize: 12,
              }}>
                <code>{children}</code>
              </pre>
            );
          }
          return (
            <code style={{
              fontFamily: 'JetBrains Mono, monospace', fontSize: 12,
              background: 'var(--overlay)', padding: '1px 4px', borderRadius: 3,
            }}>
              {children}
            </code>
          );
        },
      }}
    >
      {content}
    </ReactMarkdown>
  );
}

export function MessageBubble({ message, citations }: Props) {
  const [showCopy, setShowCopy] = useState(false);
  const [feedback, setFeedback] = useState<'up' | 'down' | null>(null);
  const [forking, setForking] = useState(false);
  const forkConversation = useChatStore(s => s.forkConversation);

  const handleFork = async () => {
    if (forking) return;
    const confirmed = await confirmDialog(
      'Creates a new, separate chat containing a copy of everything up to and including this message — useful to branch off and try a different direction without losing this point in the original conversation. The original chat is untouched.',
      { title: 'Fork this conversation?' },
    );
    if (!confirmed) return;
    setForking(true);
    try {
      await forkConversation(message.id);
    } catch (e) {
      console.error('Failed to fork conversation', e);
    } finally {
      setForking(false);
    }
  };

  if (message.role === 'user') {
    return (
      <div
        style={{ display: 'flex', justifyContent: 'flex-end', padding: '4px 24px', gap: 6, position: 'relative' }}
        onMouseEnter={() => setShowCopy(true)}
        onMouseLeave={() => setShowCopy(false)}
      >
        {showCopy && (
          <div style={{ alignSelf: 'center' }}>
            <CopyButton text={message.content} />
          </div>
        )}
        <div style={{
          background: 'var(--bg-surface)', borderRadius: 10,
          padding: '10px 14px', maxWidth: '70%',
          fontSize: 14, color: 'var(--text-primary)',
          border: '1px solid var(--border)',
          whiteSpace: 'pre-wrap', wordBreak: 'break-word',
        }}>
          {message.content}
        </div>
        <div style={{
          alignSelf: 'flex-start', flexShrink: 0, width: 20, height: 20, borderRadius: '50%',
          background: 'var(--overlay)', display: 'flex', alignItems: 'center', justifyContent: 'center',
        }}>
          <User size={12} color="var(--text-muted)" />
        </div>
      </div>
    );
  }

  // display_override is set only when the model produced nothing but
  // triggers (no chat prose) — a synthesized "Created X, edited Y."
  // confirmation, kept separate from `content` so this fabricated text
  // never leaks back into the model's own conversation history (see
  // run_turn in commands/chat.rs).
  const displayContent = message.display_override ?? stripTriggers(message.content);
  const inFlight = message.status === 'streaming' ? getInFlightAction(message.content) : null;
  const inFlightLabel = inFlight && inFlight.action !== 'MESSAGE'
    ? `${IN_PROGRESS_LABELS[inFlight.action] ?? inFlight.action}${inFlight.path ? ` ${inFlight.path}` : ''}…`
    : null;

  return (
    <div style={{ padding: '4px 24px', position: 'relative' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 4 }}>
        <div style={{
          flexShrink: 0, width: 20, height: 20, borderRadius: '50%',
          background: 'var(--overlay)', display: 'flex', alignItems: 'center', justifyContent: 'center',
        }}>
          <Bot size={12} color="var(--accent)" />
        </div>
        {message.provider && (
          <>
            <ProviderBadge provider={message.provider} size={12} />
            <span style={{ fontSize: 10, color: 'var(--text-muted)' }}>{message.model ?? message.provider}</span>
          </>
        )}
      </div>
      <div style={{
        background: 'var(--bg-surface)',
        border: '1px solid var(--border)',
        borderRadius: 10,
        padding: '14px 18px',
        fontSize: 14, color: 'var(--text-primary)',
        lineHeight: 1.6,
        overflowWrap: 'break-word',
      }}>
        {message.status === 'error' ? (
          <div>
            <div style={{ fontSize: 14, color: 'var(--text-primary)', marginBottom: displayContent.trim() ? 8 : 4 }}>
              {displayContent.trim()
                ? "Sorry, something went wrong before I could finish — here's what I had so far:"
                : "Sorry, something went wrong and I couldn't respond."}
            </div>
            {displayContent.trim() && <MarkdownContent content={displayContent} />}
            <details style={{ marginTop: 8, fontSize: 12, color: 'var(--text-muted)' }}>
              <summary style={{ cursor: 'pointer' }}>Technical details</summary>
              <div style={{ marginTop: 4, color: 'var(--error)', whiteSpace: 'pre-wrap' }}>{message.error}</div>
            </details>
          </div>
        ) : (
          <MarkdownContent content={displayContent} />
        )}
        {message.status === 'streaming' && (
          inFlightLabel ? (
            <div style={{
              display: 'flex', alignItems: 'center', gap: 6, marginTop: displayContent ? 8 : 0,
              padding: '4px 8px', borderRadius: 4, background: 'var(--overlay)',
              color: 'var(--text-muted)', fontSize: 12, width: 'fit-content',
            }}>
              <Loader2 size={12} color="var(--accent)" style={{ animation: 'spin 1s linear infinite', flexShrink: 0 }} />
              {inFlight?.path && (() => {
                const { Icon, color } = fileTypeIcon(inFlight.path);
                return <Icon size={12} color={color} style={{ flexShrink: 0 }} />;
              })()}
              {inFlightLabel}
            </div>
          ) : (
            <span style={{
              display: 'inline-block', width: 6, height: 14,
              background: 'var(--accent)', marginLeft: 2,
              animation: 'blink 1s step-end infinite',
            }} />
          )
        )}
      </div>
      {citations && citations.length > 0 && <CitationList citations={citations} />}
      {(message.status === 'complete' || message.status === 'error') && (
        <div style={{ display: 'flex', alignItems: 'center', gap: 2, marginTop: 6, paddingLeft: 14 }}>
          <ActionIconButton title="Copy" onClick={() => navigator.clipboard.writeText(displayContent).catch(() => {})}>
            <Copy size={13} />
          </ActionIconButton>
          <ActionIconButton title="Fork a new chat from here" onClick={handleFork} disabled={forking}>
            <GitFork size={13} />
          </ActionIconButton>
          <ActionIconButton
            title="Good response"
            onClick={() => setFeedback(f => f === 'up' ? null : 'up')}
            active={feedback === 'up'}
          >
            <ThumbsUp size={13} />
          </ActionIconButton>
          <ActionIconButton
            title="Bad response"
            onClick={() => setFeedback(f => f === 'down' ? null : 'down')}
            active={feedback === 'down'}
          >
            <ThumbsDown size={13} />
          </ActionIconButton>
          <span style={{ fontSize: 11, color: 'var(--text-muted)', marginLeft: 4 }}>
            {relativeTime(message.created_at)}
          </span>
        </div>
      )}
    </div>
  );
}

function ActionIconButton({
  children, title, onClick, active, disabled,
}: {
  children: ReactNode;
  title: string;
  onClick: () => void;
  active?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title}
      style={{
        background: 'none', border: 'none', cursor: disabled ? 'default' : 'pointer',
        padding: 5, borderRadius: 4, display: 'flex', alignItems: 'center',
        color: active ? 'var(--accent)' : 'var(--text-muted)',
        opacity: disabled ? 0.5 : 1,
      }}
    >
      {children}
    </button>
  );
}
