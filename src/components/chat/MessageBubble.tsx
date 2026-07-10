import { memo, useState, type ReactNode } from 'react';
import ReactMarkdown, { defaultUrlTransform } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import katex from 'katex';
import { ChartBlock } from './ChartBlock';
import { MermaidDiagram } from './MermaidDiagram';
import { RecipeCard, MapCard, KanbanBoard, WeatherCard } from './InfoCards';
import { parseRecipeSpec, parseMapSpec, parseKanbanSpec, parseWeatherSpec } from '../../lib/vizSpecs';
import { parseChartSpec } from '../../lib/chartSpec';
import { convertFileSrc } from '@tauri-apps/api/core';
import { open as openShell } from '@tauri-apps/plugin-shell';
import { confirm as confirmDialog } from '@tauri-apps/plugin-dialog';
import { Copy, Check, User, Bot, Loader2, GitFork, ThumbsUp, ThumbsDown } from 'lucide-react';
import { fileTypeIcon } from '../../lib/fileIcons';
import type { Message, Citation } from '../../lib/types';
import { useUIStore } from '../../stores/uiStore';
import { useChatStore } from '../../stores/chatStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { CitationList } from './CitationList';
import { ProviderBadge } from './ProviderBadge';
import { stripTriggers, getInFlightAction } from '../../lib/triggerStrip';
import { relativeTime, fullDateTime } from '../../lib/time';

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

function resolveImageSrc(src: string, workspacePath: string | undefined): string {
  const isAbsolute = /^([a-z]+:)?\/\//i.test(src) || src.startsWith('data:');
  return isAbsolute || !workspacePath ? src : convertFileSrc(`${workspacePath}/${src}`);
}

const AUDIO_EXTENSIONS = /\.(mp3|wav|ogg)$/i;

/** A link's target, once resolved past the fake atelier-file: scheme (see
 * ATELIER_FILE_SCHEME above) and any URI percent-encoding — the same
 * decoding the `a` component below already does before opening the file
 * viewer, needed here too before checking/using the raw path. */
function resolvedLinkPath(href: string): string {
  return href.startsWith(ATELIER_FILE_SCHEME)
    ? decodeURIComponent(href.slice(ATELIER_FILE_SCHEME.length))
    : href;
}

// User messages render as plain text below (so ordinary typed markdown-ish
// characters like "*" or "_" don't get silently reinterpreted), but an
// attached image still needs to show as an image rather than its literal
// `![name](path)` source — pull just that out and render the remaining
// text as before.
function extractImages(content: string): { text: string; images: { alt: string; src: string }[] } {
  const images: { alt: string; src: string }[] = [];
  const text = content.replace(/!\[([^\]]*)\]\(([^)]+)\)\n?/g, (_match, alt: string, src: string) => {
    images.push({ alt, src });
    return '';
  }).trim();
  return { text, images };
}

// Turns "$$...$$" block math into a fenced ```math code block so it flows
// through the same fenced-code-block machinery the `code` component
// override already handles below, rather than needing a separate remark
// plugin. Inline "$...$" math is deliberately not supported — there's no
// reliable way to tell it apart from an ordinary price mention like "$5 to
// $10" in prose.
function wrapMathBlocks(content: string): string {
  return content.replace(/\$\$([\s\S]+?)\$\$/g, (_match, formula: string) => `\n\`\`\`math\n${formula.trim()}\n\`\`\`\n`);
}

function MathBlock({ formula }: { formula: string }) {
  let html: string;
  try {
    html = katex.renderToString(formula, { throwOnError: true, displayMode: true });
  } catch {
    // Malformed LaTeX shouldn't take the rest of the message down with it —
    // fall back to showing the raw source like an ordinary code block.
    return (
      <pre style={{
        background: 'var(--bg-surface)', border: '1px solid var(--border)',
        borderRadius: 4, padding: '10px 12px', overflow: 'auto',
        fontFamily: 'JetBrains Mono, monospace', fontSize: 12, color: 'var(--error)',
      }}>
        {formula}
      </pre>
    );
  }
  return (
    <div style={{ overflowX: 'auto', margin: '4px 0 12px' }} dangerouslySetInnerHTML={{ __html: html }} />
  );
}

function MarkdownContent({ content }: { content: string }) {
  const openFileViewer = useUIStore(s => s.openFileViewer);
  const workspacePath = useWorkspaceStore(s => s.active?.path);

  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      urlTransform={urlTransform}
      components={{
        // Thematic breaks ("---") render as an unstyled, jarring
        // browser-default <hr> inside a chat bubble — suppress them
        // rather than trying to make a heavy rule look at home here.
        hr: () => null,
        // Bare browser-default list styling (the ~40px UA-stylesheet
        // indent) reads as broken inside the bubble's own padding — same
        // treatment as the file viewer's markdown rendering.
        ul: ({ children }) => <ul style={{ margin: '0 0 12px', paddingLeft: 22 }}>{children}</ul>,
        ol: ({ children }) => <ol style={{ margin: '0 0 12px', paddingLeft: 22 }}>{children}</ol>,
        li: ({ children }) => <li style={{ marginBottom: 4 }}>{children}</li>,
        // A message's image reference (e.g. an attached photo) is a path
        // relative to the workspace root, same as everywhere else in the
        // app — resolve it through the asset protocol rather than letting
        // the browser treat it as a plain relative URL, which would 404.
        img: ({ src, alt }) => {
          if (!src) return null;
          return (
            <img
              src={resolveImageSrc(src, workspacePath)}
              alt={alt}
              style={{ maxWidth: '100%', borderRadius: 4, border: '1px solid var(--border)' }}
            />
          );
        },
        blockquote: ({ children }) => (
          <blockquote style={{
            margin: '0 0 12px', padding: '4px 12px',
            borderLeft: '3px solid var(--accent)', color: 'var(--text-muted)',
            background: 'var(--overlay)', borderRadius: '0 4px 4px 0',
          }}>
            {children}
          </blockquote>
        ),
        table: ({ children }) => (
          <div style={{ overflow: 'auto', marginBottom: 12 }}>
            <table style={{ borderCollapse: 'collapse', width: '100%', fontSize: 13 }}>
              {children}
            </table>
          </div>
        ),
        thead: ({ children }) => <thead style={{ background: 'var(--overlay)' }}>{children}</thead>,
        th: ({ children }) => (
          <th style={{ border: '1px solid var(--border)', padding: '6px 10px', textAlign: 'left', fontWeight: 600, color: 'var(--text-primary)' }}>
            {children}
          </th>
        ),
        td: ({ children }) => (
          <td style={{ border: '1px solid var(--border)', padding: '6px 10px', color: 'var(--text-primary)' }}>
            {children}
          </td>
        ),
        a: ({ children, href }) => {
          if (href && AUDIO_EXTENSIONS.test(resolvedLinkPath(href))) {
            return (
              <audio
                controls
                src={resolveImageSrc(resolvedLinkPath(href), workspacePath)}
                style={{ display: 'block', width: '100%', maxWidth: 360, marginBottom: 4 }}
              />
            );
          }
          return (
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
          );
        },
        code({ children, className }) {
          if (className === 'language-math') {
            return <MathBlock formula={String(children).trim()} />;
          }
          if (className === 'language-chart') {
            const spec = parseChartSpec(String(children).trim());
            if (spec) return <ChartBlock spec={spec} />;
          }
          if (className === 'language-mermaid') {
            return <MermaidDiagram code={String(children).trim()} />;
          }
          if (className === 'language-recipe') {
            const recipe = parseRecipeSpec(String(children).trim());
            if (recipe) return <RecipeCard recipe={recipe} />;
          }
          if (className === 'language-map') {
            const spec = parseMapSpec(String(children).trim());
            if (spec) return <MapCard spec={spec} />;
          }
          if (className === 'language-kanban') {
            const board = parseKanbanSpec(String(children).trim());
            if (board) return <KanbanBoard board={board} />;
          }
          if (className === 'language-weather') {
            const weather = parseWeatherSpec(String(children).trim());
            if (weather) return <WeatherCard weather={weather} />;
          }
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
      {wrapMathBlocks(content)}
    </ReactMarkdown>
  );
}

// Memoized so a token streaming into one message doesn't re-render (and
// re-parse the markdown of) every other message in a long conversation —
// chatStore's appendToken keeps the same object reference for every message
// except the one being streamed into, so a shallow prop comparison here is
// enough to skip the rest.
export const MessageBubble = memo(function MessageBubble({ message, citations }: Props) {
  const [showCopy, setShowCopy] = useState(false);
  const [feedback, setFeedback] = useState<'up' | 'down' | null>(null);
  const [forking, setForking] = useState(false);
  const forkConversation = useChatStore(s => s.forkConversation);
  const workspacePath = useWorkspaceStore(s => s.active?.path);

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
    const { text, images } = extractImages(message.content);
    return (
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', padding: '4px 24px' }}>
        <div
          style={{ display: 'flex', justifyContent: 'flex-end', gap: 6, position: 'relative', width: '100%' }}
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
            {images.map(img => (
              <img
                key={img.src}
                src={resolveImageSrc(img.src, workspacePath)}
                alt={img.alt}
                style={{ maxWidth: 240, maxHeight: 240, borderRadius: 6, display: 'block', marginBottom: text ? 8 : 0 }}
              />
            ))}
            {text}
          </div>
          <div style={{
            alignSelf: 'flex-start', flexShrink: 0, width: 20, height: 20, borderRadius: '50%',
            background: 'var(--overlay)', display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}>
            <User size={12} color="var(--text-muted)" />
          </div>
        </div>
        <span
          title={fullDateTime(message.created_at)}
          style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 4, marginRight: 26, cursor: 'default' }}
        >
          {relativeTime(message.created_at)}
        </span>
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
          <span
            title={fullDateTime(message.created_at)}
            style={{ fontSize: 11, color: 'var(--text-muted)', marginLeft: 4, cursor: 'default' }}
          >
            {relativeTime(message.created_at)}
          </span>
        </div>
      )}
    </div>
  );
});

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
