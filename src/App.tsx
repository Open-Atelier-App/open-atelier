import { useState, useEffect, useCallback } from 'react';
import { Copy, Check } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { convertFileSrc } from '@tauri-apps/api/core';
import { open as openShell } from '@tauri-apps/plugin-shell';
import { exists } from '@tauri-apps/plugin-fs';
import { LeftSidebar } from './components/layout/LeftSidebar';
import { CenterPane } from './components/layout/CenterPane';
import { RightBar } from './components/layout/RightBar';
import { SettingsView } from './components/settings/SettingsView';
import { ProfileSetup } from './components/onboarding/ProfileSetup';
import { IndexProgressBar } from './components/workspace/IndexProgressBar';
import { SearchOverlay } from './components/search/SearchOverlay';
import { useUIStore } from './stores/uiStore';
import { useProfileStore } from './stores/profileStore';
import { useWorkspaceStore } from './stores/workspaceStore';
import { useTauriEvents } from './hooks/useTauriEvents';
import * as api from './lib/tauri';
import type { OfficePreview } from './lib/types';

export default function App() {
  const sidebarOpen = useUIStore(s => s.sidebarOpen);
  const rightBarOpen = useUIStore(s => s.rightBarOpen);
  const toggleSidebar = useUIStore(s => s.toggleSidebar);
  const toggleRightBar = useUIStore(s => s.toggleRightBar);
  const showSettings = useUIStore(s => s.showSettings);
  const setShowSettings = useUIStore(s => s.setShowSettings);
  const fileViewerOpen = useUIStore(s => s.fileViewerOpen);
  const fileViewerPath = useUIStore(s => s.fileViewerPath);
  const closeFileViewer = useUIStore(s => s.closeFileViewer);
  const toggleFileViewer = useUIStore(s => s.toggleFileViewer);
  const triggerNewChat = useUIStore(s => s.triggerNewChat);
  const searchOpen = useUIStore(s => s.searchOpen);
  const setSearchOpen = useUIStore(s => s.setSearchOpen);
  const loadProfiles = useProfileStore(s => s.load);
  const profiles = useProfileStore(s => s.profiles);
  const profilesLoading = useProfileStore(s => s.loading);
  const activeProfile = useProfileStore(s => s.active);
  const loadWorkspaces = useWorkspaceStore(s => s.load);
  const activeWorkspace = useWorkspaceStore(s => s.active);
  const forceProfileSetup = useUIStore(s => s.forceProfileSetup);
  const setForceProfileSetup = useUIStore(s => s.setForceProfileSetup);
  const setMissingProfileId = useUIStore(s => s.setMissingProfileId);
  const [profileSetupDismissed, setProfileSetupDismissed] = useState(false);
  const showProfileSetup = (!profilesLoading && profiles.length === 0 && !profileSetupDismissed) || forceProfileSetup;

  useTauriEvents();

  useEffect(() => {
    loadProfiles();
  }, [loadProfiles]);


  useEffect(() => {
    if (activeProfile) {
      loadWorkspaces(activeProfile.id);
    }
  }, [activeProfile, loadWorkspaces]);

  // Detect the active profile's root directory having been deleted/moved
  // externally (e.g. while the app wasn't actively switching profiles —
  // the profile-switch path in LeftSidebar already checks this, but nothing
  // previously checked it at startup / on profile load). Reuses the same
  // missingProfileId banner so the user gets the same Locate/Recreate flow.
  useEffect(() => {
    if (!activeProfile) return;
    let cancelled = false;
    exists(activeProfile.root_path)
      .then(isThere => {
        if (!cancelled && !isThere) {
          setMissingProfileId(activeProfile.id);
        }
      })
      .catch(() => {
        // If the existence check itself fails, don't block startup — fall through silently.
      });
    return () => { cancelled = true; };
  }, [activeProfile, setMissingProfileId]);

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    const meta = e.metaKey || e.ctrlKey;
    if (e.key === 'Escape') {
      if (searchOpen) { setSearchOpen(false); return; }
      if (showSettings) { setShowSettings(false); return; }
      if (fileViewerOpen) { closeFileViewer(); return; }
      return;
    }
    if (!meta) return;
    switch (e.key) {
      case '[': e.preventDefault(); toggleSidebar(); break;
      case ']': e.preventDefault(); toggleRightBar(); break;
      case '\\': e.preventDefault(); toggleFileViewer(); break;
      case ',': e.preventDefault(); setShowSettings(true); break;
      case 'n': e.preventDefault(); triggerNewChat(); break;
      case 'k': e.preventDefault(); setSearchOpen(!searchOpen); break;
    }
  }, [toggleSidebar, toggleRightBar, closeFileViewer, toggleFileViewer, setShowSettings, triggerNewChat, searchOpen, setSearchOpen, showSettings, fileViewerOpen]);

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);

  return (
    <div style={{
      display: 'flex', flexDirection: 'column', height: '100vh', overflow: 'hidden',
      background: 'var(--bg-app)', color: 'var(--text-primary)',
    }}>
      <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
        <LeftSidebar collapsed={!sidebarOpen} />

        <div style={{ flex: 1, display: 'flex', overflow: 'hidden', position: 'relative' }}>
          <CenterPane />
          {fileViewerOpen && activeWorkspace && fileViewerPath && (
            <FileViewerPanel
              path={fileViewerPath}
              workspaceId={activeWorkspace.id}
              workspacePath={activeWorkspace.path}
              onClose={closeFileViewer}
            />
          )}
        </div>

        {activeWorkspace && <RightBar collapsed={!rightBarOpen} />}
      </div>
      <IndexProgressBar />
      <SearchOverlay />
      {showSettings && <SettingsView onClose={() => setShowSettings(false)} />}
      {showProfileSetup && (
        <ProfileSetup onDone={() => { setProfileSetupDismissed(true); setForceProfileSetup(false); }} />
      )}
    </div>
  );
}

/** Splits a leading `---\n...\n---` YAML-ish frontmatter block off of markdown
 * content. Hand-rolled (no new dependency): only simple `key: value` pairs are
 * parsed; anything more complex in the frontmatter is ignored. Returns the
 * frontmatter as a plain key->string map (possibly empty) and the remaining
 * markdown body with the frontmatter block stripped out. */
function splitFrontmatter(raw: string): { meta: Record<string, string>; body: string } {
  if (!raw.startsWith('---\n') && !raw.startsWith('---\r\n')) {
    return { meta: {}, body: raw };
  }
  const lines = raw.split(/\r?\n/);
  let endIdx = -1;
  for (let i = 1; i < lines.length; i++) {
    if (lines[i] === '---') { endIdx = i; break; }
  }
  if (endIdx === -1) return { meta: {}, body: raw };
  const meta: Record<string, string> = {};
  for (const line of lines.slice(1, endIdx)) {
    const m = /^([A-Za-z0-9_-]+):\s*(.*)$/.exec(line);
    if (m) {
      let value = m[2].trim();
      if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
        value = value.slice(1, -1);
      }
      meta[m[1]] = value;
    }
  }
  const body = lines.slice(endIdx + 1).join('\n');
  return { meta, body };
}

/** Resolves a markdown/HTML-relative path (e.g. an image src) against the
 * directory of the file currently being previewed, returning a workspace-relative
 * path suitable for further lookups, or the original src if it's already
 * absolute/remote (http(s):, data:, or a leading slash). */
function resolveRelative(srcPath: string, basePath: string): string {
  if (/^([a-z]+:)?\/\//i.test(srcPath) || srcPath.startsWith('data:') || srcPath.startsWith('/')) {
    return srcPath;
  }
  const baseDir = basePath.includes('/') ? basePath.slice(0, basePath.lastIndexOf('/')) : '';
  const combined = baseDir ? `${baseDir}/${srcPath}` : srcPath;
  // Collapse "./" and "../" segments.
  const parts = combined.split('/');
  const out: string[] = [];
  for (const part of parts) {
    if (part === '.' || part === '') continue;
    if (part === '..') out.pop();
    else out.push(part);
  }
  return out.join('/');
}

/** Renders a markdown image, resolving workspace-relative paths via the asset
 * protocol and falling back to a styled placeholder if the image fails to load. */
function MarkdownImage({
  src, alt, basePath, workspacePath,
}: {
  src?: string;
  alt?: string;
  basePath: string;
  workspacePath: string;
}) {
  const [broken, setBroken] = useState(false);
  if (!src || broken) {
    return (
      <span style={{
        display: 'inline-flex', alignItems: 'center', gap: 6,
        padding: '6px 10px', background: 'var(--overlay)', borderRadius: 4,
        color: 'var(--text-muted)', fontSize: 11,
      }}>
        ⚠ Image unavailable{alt ? `: ${alt}` : ''}
      </span>
    );
  }
  const resolved = resolveRelative(src, basePath);
  const resolvedSrc = /^([a-z]+:)?\/\//i.test(src) || src.startsWith('data:')
    ? src
    : convertFileSrc(`${workspacePath}/${resolved}`);
  return (
    <img
      src={resolvedSrc}
      alt={alt}
      onError={() => setBroken(true)}
      style={{ maxWidth: '100%', borderRadius: 4, border: '1px solid var(--border)' }}
    />
  );
}

function CopyButton({ text, title }: { text: string; title?: string }) {
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
      title={title ?? (copied ? 'Copied!' : 'Copy')}
      style={{
        display: 'flex', alignItems: 'center', gap: 4, flexShrink: 0,
        padding: '3px 8px', borderRadius: 4, fontSize: 11, border: 'none', cursor: 'pointer',
        background: 'var(--overlay)', color: copied ? 'var(--success)' : 'var(--text-muted)',
      }}
    >
      {copied ? <Check size={11} /> : <Copy size={11} />}
      {copied ? 'Copied' : 'Copy'}
    </button>
  );
}

function FileViewerPanel({
  path, workspaceId, workspacePath, onClose,
}: {
  path: string;
  workspaceId: number;
  workspacePath: string;
  onClose: () => void;
}) {
  const [content, setContent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [officePreview, setOfficePreview] = useState<OfficePreview | null>(null);
  const [officeError, setOfficeError] = useState<string | null>(null);
  const [activeSheet, setActiveSheet] = useState(0);
  const [exportingPdf, setExportingPdf] = useState(false);
  const openFileViewer = useUIStore(s => s.openFileViewer);
  const loadFileTree = useWorkspaceStore(s => s.loadFileTree);

  const lowerPath = path.toLowerCase();
  const isHtml = lowerPath.endsWith('.html') || lowerPath.endsWith('.htm');
  const isMarkdown = lowerPath.endsWith('.md') || lowerPath.endsWith('.markdown');
  const hasToggle = isHtml || isMarkdown;
  // Word/Excel/PowerPoint are real ZIP-based binary formats — reading them
  // as UTF-8 text (like every other file in this viewer) always fails, so
  // they get a structural preview (extracted text/table content) instead.
  const isOfficePreviewable = ['.docx', '.xlsx', '.pptx'].some(ext => lowerPath.endsWith(ext));
  const isPdf = lowerPath.endsWith('.pdf');
  const isMp3 = lowerPath.endsWith('.mp3');
  const isOfficeBinary = isOfficePreviewable || isPdf;
  // Binary formats that skip the raw UTF-8 text read entirely — Office/PDF
  // get their own structural preview above, MP3 gets an <audio> player.
  const skipsRawRead = isOfficeBinary || isMp3;

  const [mode, setMode] = useState<'code' | 'preview'>(hasToggle ? 'preview' : 'code');
  const [modeForPath, setModeForPath] = useState(path);
  if (path !== modeForPath) {
    // Reset per-file state whenever a different file is opened (derived during render, not in an effect).
    setModeForPath(path);
    setMode(hasToggle ? 'preview' : 'code');
    setActiveSheet(0);
    setOfficePreview(null);
    setOfficeError(null);
  }

  useEffect(() => {
    if (skipsRawRead) return;
    api.fileReadRaw(workspaceId, path)
      .then(setContent)
      .catch((e: unknown) => setError(api.errorMessage(e)));
  }, [path, workspaceId, skipsRawRead]);

  useEffect(() => {
    if (!isOfficePreviewable) return;
    api.fileReadOfficePreview(workspaceId, path)
      .then(setOfficePreview)
      .catch((e: unknown) => setOfficeError(api.errorMessage(e)));
  }, [path, workspaceId, isOfficePreviewable]);

  // Direct "Export to PDF" button for HTML files — same html_to_pdf
  // renderer as the EXPORT_PDF trigger, just invoked straight from the UI
  // instead of requiring the model to do it.
  const handleExportPdf = async () => {
    setExportingPdf(true);
    try {
      const pdfPath = await api.fileExportPdf(workspaceId, path);
      await loadFileTree(workspaceId);
      openFileViewer(pdfPath);
    } catch (e: unknown) {
      setError(api.errorMessage(e));
    } finally {
      setExportingPdf(false);
    }
  };

  // For HTML preview: rather than injecting a <base> tag into srcDoc (which
  // is unreliable — srcDoc documents get an opaque "about:srcdoc" origin in
  // some WebKit/sandbox combinations, so a <base href="asset://..."> doesn't
  // always get honored for relative resource resolution), point the iframe's
  // `src` directly at the file's own asset:// URL. The browser then resolves
  // every relative <link>/<script>/<img> against that URL natively, with no
  // string-rewriting needed.
  const htmlAssetSrc = content !== null && isHtml
    ? convertFileSrc(`${workspacePath}/${path}`)
    : undefined;

  // Most WebViews (WKWebView on macOS, WebView2 on Windows) render a PDF
  // natively when it's loaded directly, same as a browser tab — no PDF.js
  // or other renderer needed. Some webkit2gtk builds on Linux lack a PDF
  // plugin, hence the "open in default app" fallback link kept alongside it.
  const pdfAssetSrc = isPdf ? convertFileSrc(`${workspacePath}/${path}`) : undefined;
  const mp3AssetSrc = isMp3 ? convertFileSrc(`${workspacePath}/${path}`) : undefined;

  const { meta: frontmatter, body: markdownBody } = isMarkdown && content !== null
    ? splitFrontmatter(content)
    : { meta: {}, body: content ?? '' };
  const hasFrontmatter = Object.keys(frontmatter).length > 0;

  return (
    <div style={{
      position: 'absolute', top: 0, right: 0, bottom: 0,
      width: 480, background: 'var(--bg-surface)',
      borderLeft: '1px solid var(--border)',
      zIndex: 50, display: 'flex', flexDirection: 'column',
    }}>
      <div style={{
        padding: '0 16px', height: 48, borderBottom: '1px solid var(--border)',
        display: 'flex', alignItems: 'center', gap: 8, flexShrink: 0,
      }}>
        <button
          onClick={onClose}
          style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', fontSize: 16 }}
        >
          ✕
        </button>
        <span style={{
          fontSize: 13, fontWeight: 500, color: 'var(--text-primary)',
          flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
        }}>
          {path.split('/').pop()}
        </span>
        {hasToggle && (
          <div style={{ display: 'flex', gap: 2, flexShrink: 0 }}>
            <button
              onClick={() => setMode('code')}
              style={{
                padding: '3px 10px', borderRadius: 4, fontSize: 11,
                background: mode === 'code' ? 'var(--accent)' : 'var(--overlay)',
                color: mode === 'code' ? '#fff' : 'var(--text-muted)',
                border: 'none', cursor: 'pointer',
              }}
            >
              Code
            </button>
            <button
              onClick={() => setMode('preview')}
              style={{
                padding: '3px 10px', borderRadius: 4, fontSize: 11,
                background: mode === 'preview' ? 'var(--accent)' : 'var(--overlay)',
                color: mode === 'preview' ? '#fff' : 'var(--text-muted)',
                border: 'none', cursor: 'pointer',
              }}
            >
              Preview
            </button>
          </div>
        )}
        {content !== null && (
          <CopyButton text={content} title={mode === 'preview' ? 'Copy preview' : 'Copy code'} />
        )}
        {isHtml && (
          <button
            onClick={handleExportPdf}
            disabled={exportingPdf}
            title="Export to PDF"
            style={{
              display: 'flex', alignItems: 'center', gap: 4, flexShrink: 0,
              padding: '3px 8px', borderRadius: 4, fontSize: 11, border: 'none',
              cursor: exportingPdf ? 'default' : 'pointer',
              background: 'var(--overlay)', color: exportingPdf ? 'var(--text-muted)' : 'var(--text-primary)',
            }}
          >
            {exportingPdf ? 'Exporting…' : 'Export to PDF'}
          </button>
        )}
      </div>
      <div style={{ flex: 1, overflow: 'auto', padding: (mode === 'preview' && isHtml) || isPdf ? 0 : 16 }}>
        {isOfficePreviewable && (
          <OfficeDocumentPreview
            preview={officePreview}
            error={officeError}
            activeSheet={activeSheet}
            onSheetChange={setActiveSheet}
            onOpenDefault={() => api.openPath(`${workspacePath}/${path}`).catch((e: unknown) => setError(api.errorMessage(e)))}
            openError={error}
          />
        )}
        {isPdf && (
          <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
            <div style={{
              display: 'flex', justifyContent: 'flex-end', padding: '6px 10px',
              borderBottom: '1px solid var(--border)', flexShrink: 0,
            }}>
              <button
                onClick={() => api.openPath(`${workspacePath}/${path}`).catch((e: unknown) => setError(api.errorMessage(e)))}
                style={{
                  padding: '3px 10px', background: 'var(--overlay)', border: '1px solid var(--border)',
                  borderRadius: 4, color: 'var(--text-muted)', fontSize: 12, cursor: 'pointer',
                }}
              >
                Open in default app
              </button>
            </div>
            <iframe
              src={pdfAssetSrc}
              title="PDF preview"
              style={{ flex: 1, width: '100%', border: 'none', background: '#fff' }}
            />
            {error && <div style={{ padding: 12, color: 'var(--error)', fontSize: 13 }}>{error}</div>}
          </div>
        )}
        {isMp3 && (
          <div>
            <audio controls src={mp3AssetSrc} style={{ width: '100%' }} />
            <div style={{ marginTop: 12, textAlign: 'center' }}>
              <button
                onClick={() => api.openPath(`${workspacePath}/${path}`).catch((e: unknown) => setError(api.errorMessage(e)))}
                style={{
                  padding: '3px 10px', background: 'var(--overlay)', border: '1px solid var(--border)',
                  borderRadius: 4, color: 'var(--text-muted)', fontSize: 12, cursor: 'pointer',
                }}
              >
                Open in default app
              </button>
            </div>
            {error && <div style={{ padding: 12, color: 'var(--error)', fontSize: 13 }}>{error}</div>}
          </div>
        )}
        {isOfficeBinary && !isOfficePreviewable && !isPdf && (
          <div style={{ textAlign: 'center', padding: '32px 16px', color: 'var(--text-muted)', fontSize: 13 }}>
            <p>This is a binary document — Atelier can't show it inline.</p>
            <button
              onClick={() => api.openPath(`${workspacePath}/${path}`).catch((e: unknown) => setError(api.errorMessage(e)))}
              style={{
                marginTop: 8, padding: '6px 14px', background: 'var(--accent)', border: 'none',
                borderRadius: 4, color: '#fff', fontSize: 13, cursor: 'pointer',
              }}
            >
              Open in default app
            </button>
            {error && <div style={{ marginTop: 12, color: 'var(--error)' }}>{error}</div>}
          </div>
        )}
        {!isOfficeBinary && error && <div style={{ color: 'var(--error)', fontSize: 13, padding: error && mode === 'preview' && isHtml ? 16 : 0 }}>{error}</div>}
        {content !== null && mode === 'code' && (
          <pre style={{
            margin: 0, fontFamily: 'JetBrains Mono, monospace',
            fontSize: 12, lineHeight: 1.6, color: 'var(--text-primary)',
            whiteSpace: 'pre-wrap', wordBreak: 'break-word',
          }}>
            {content}
          </pre>
        )}
        {content !== null && mode === 'preview' && isHtml && (
          <iframe
            src={htmlAssetSrc}
            sandbox="allow-same-origin allow-scripts"
            title="HTML preview"
            style={{ width: '100%', height: '100%', border: 'none', background: '#fff' }}
          />
        )}
        {content !== null && mode === 'preview' && isMarkdown && (
          <div style={{ fontSize: 13, color: 'var(--text-primary)', lineHeight: 1.6 }}>
            {hasFrontmatter && (
              <div style={{
                marginBottom: 16, paddingBottom: 12, borderBottom: '1px solid var(--border)',
              }}>
                {frontmatter.title && (
                  <div style={{ fontSize: 20, fontWeight: 700, color: 'var(--text-primary)', marginBottom: 4 }}>
                    {frontmatter.title}
                  </div>
                )}
                {frontmatter.date && (
                  <div style={{ fontSize: 12, color: 'var(--text-muted)' }}>
                    {frontmatter.date}
                  </div>
                )}
              </div>
            )}
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              components={{
                h1: ({ children }) => <h1 style={{ fontSize: 22, fontWeight: 700, margin: '20px 0 10px', borderBottom: '1px solid var(--border)', paddingBottom: 6, color: 'var(--text-primary)' }}>{children}</h1>,
                h2: ({ children }) => <h2 style={{ fontSize: 19, fontWeight: 700, margin: '18px 0 8px', borderBottom: '1px solid var(--border)', paddingBottom: 4, color: 'var(--text-primary)' }}>{children}</h2>,
                h3: ({ children }) => <h3 style={{ fontSize: 16, fontWeight: 600, margin: '16px 0 6px', color: 'var(--text-primary)' }}>{children}</h3>,
                h4: ({ children }) => <h4 style={{ fontSize: 14, fontWeight: 600, margin: '14px 0 6px', color: 'var(--text-primary)' }}>{children}</h4>,
                h5: ({ children }) => <h5 style={{ fontSize: 13, fontWeight: 600, margin: '12px 0 4px', color: 'var(--text-muted)' }}>{children}</h5>,
                h6: ({ children }) => <h6 style={{ fontSize: 12, fontWeight: 600, margin: '10px 0 4px', color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '0.04em' }}>{children}</h6>,
                p: ({ children }) => <p style={{ margin: '0 0 12px' }}>{children}</p>,
                blockquote: ({ children }) => (
                  <blockquote style={{
                    margin: '0 0 12px', padding: '4px 12px',
                    borderLeft: '3px solid var(--accent)', color: 'var(--text-muted)',
                    background: 'var(--overlay)', borderRadius: '0 4px 4px 0',
                  }}>
                    {children}
                  </blockquote>
                ),
                ul: ({ children }) => <ul style={{ margin: '0 0 12px', paddingLeft: 22 }}>{children}</ul>,
                ol: ({ children }) => <ol style={{ margin: '0 0 12px', paddingLeft: 22 }}>{children}</ol>,
                li: ({ children }) => <li style={{ marginBottom: 4 }}>{children}</li>,
                hr: () => <hr style={{ border: 'none', borderTop: '1px solid var(--border)', margin: '20px 0' }} />,
                a: ({ children, href }) => (
                  <a
                    href={href}
                    onClick={(e) => {
                      e.preventDefault();
                      if (href) openShell(href).catch((err: unknown) => console.error('Failed to open link', err));
                    }}
                    style={{ color: 'var(--accent)', cursor: 'pointer', textDecoration: 'underline' }}
                  >
                    {children}
                  </a>
                ),
                img: ({ src, alt }) => (
                  <MarkdownImage src={src} alt={alt} basePath={path} workspacePath={workspacePath} />
                ),
                table: ({ children }) => (
                  <div style={{ overflow: 'auto', marginBottom: 12 }}>
                    <table style={{ borderCollapse: 'collapse', width: '100%', fontSize: 12 }}>
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
                code({ children, className }) {
                  const isBlock = className?.startsWith('language-');
                  if (isBlock) {
                    return (
                      <pre style={{
                        background: 'var(--bg-app)', border: '1px solid var(--border)',
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
              {markdownBody}
            </ReactMarkdown>
          </div>
        )}
      </div>
    </div>
  );
}

// Flattens a structural office preview into plain text for the copy
// button — there's no "rendered" form to copy for these binary formats,
// so this is the same extracted text/table content shown on screen.
function officePreviewText(preview: OfficePreview, activeSheet: number): string {
  if (preview.kind === 'docx') {
    return preview.blocks.map(b => b.text).join('\n\n');
  }
  if (preview.kind === 'xlsx') {
    const sheet = preview.sheets[activeSheet] ?? preview.sheets[0];
    return sheet ? sheet.rows.map(row => row.join('\t')).join('\n') : '';
  }
  return preview.slides
    .map((s, i) => [
      `Slide ${i + 1}: ${s.title}`,
      ...s.bullets.map(b => b.heading ? b.text : `- ${b.text}`),
    ].join('\n'))
    .join('\n\n');
}

function OfficeDocumentPreview({
  preview, error, activeSheet, onSheetChange, onOpenDefault, openError,
}: {
  preview: OfficePreview | null;
  error: string | null;
  activeSheet: number;
  onSheetChange: (index: number) => void;
  onOpenDefault: () => void;
  openError: string | null;
}) {
  const openButton = (
    <button
      onClick={onOpenDefault}
      style={{
        padding: '4px 10px', background: 'var(--overlay)', border: '1px solid var(--border)',
        borderRadius: 4, color: 'var(--text-muted)', fontSize: 12, cursor: 'pointer', flexShrink: 0,
      }}
    >
      Open in default app
    </button>
  );

  const header = (label: string) => (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 14 }}>
      <span style={{ fontSize: 11, color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '0.04em' }}>
        {label} · text preview
      </span>
      <div style={{ display: 'flex', gap: 6 }}>
        {preview && <CopyButton text={officePreviewText(preview, activeSheet)} title="Copy preview" />}
        {openButton}
      </div>
    </div>
  );

  if (error) {
    return (
      <div>
        <div style={{ display: 'flex', justifyContent: 'flex-end', marginBottom: 12 }}>{openButton}</div>
        <div style={{ color: 'var(--error)', fontSize: 13 }}>Couldn't preview this file: {error}</div>
        {openError && <div style={{ marginTop: 8, color: 'var(--error)', fontSize: 13 }}>{openError}</div>}
      </div>
    );
  }

  if (!preview) {
    return <div style={{ color: 'var(--text-muted)', fontSize: 13 }}>Loading preview…</div>;
  }

  if (preview.kind === 'docx') {
    return (
      <div>
        {header('Word document')}
        <div style={{ fontSize: 13, color: 'var(--text-primary)', lineHeight: 1.6 }}>
          {preview.blocks.length === 0 && (
            <div style={{ color: 'var(--text-muted)' }}>(empty document)</div>
          )}
          {preview.blocks.map((block, i) => {
            if (block.kind === 'heading1') {
              return <div key={i} style={{ fontSize: 20, fontWeight: 700, margin: '4px 0 10px' }}>{block.text}</div>;
            }
            if (block.kind === 'heading2') {
              return <div key={i} style={{ fontSize: 16, fontWeight: 600, margin: '4px 0 8px' }}>{block.text}</div>;
            }
            if (block.kind === 'heading3') {
              return <div key={i} style={{ fontSize: 14, fontWeight: 600, margin: '4px 0 6px', color: 'var(--text-primary)' }}>{block.text}</div>;
            }
            if (block.kind === 'bullet') {
              return (
                <div key={i} style={{ display: 'flex', gap: 8, margin: '0 0 6px', paddingLeft: 4 }}>
                  <span style={{ color: 'var(--text-muted)' }}>•</span>
                  <span>{block.text}</span>
                </div>
              );
            }
            return <p key={i} style={{ margin: '0 0 12px' }}>{block.text}</p>;
          })}
        </div>
      </div>
    );
  }

  if (preview.kind === 'xlsx') {
    const sheet = preview.sheets[activeSheet] ?? preview.sheets[0];
    return (
      <div>
        {header('Excel workbook')}
        {preview.sheets.length > 1 && (
          <div style={{ display: 'flex', gap: 4, marginBottom: 10, flexWrap: 'wrap' }}>
            {preview.sheets.map((s, i) => (
              <button
                key={s.name}
                onClick={() => onSheetChange(i)}
                style={{
                  padding: '3px 10px', borderRadius: 4, fontSize: 12, border: 'none', cursor: 'pointer',
                  background: i === activeSheet ? 'var(--accent)' : 'var(--overlay)',
                  color: i === activeSheet ? '#fff' : 'var(--text-muted)',
                }}
              >
                {s.name}
              </button>
            ))}
          </div>
        )}
        {sheet && sheet.rows.length > 0 ? (
          <div style={{ overflow: 'auto' }}>
            <table style={{ borderCollapse: 'collapse', width: '100%', fontSize: 12 }}>
              <thead>
                <tr style={{ background: 'var(--overlay)' }}>
                  {sheet.rows[0].map((cell, i) => (
                    <th key={i} style={{ border: '1px solid var(--border)', padding: '6px 10px', textAlign: 'left', fontWeight: 600, color: 'var(--text-primary)' }}>
                      {cell}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {sheet.rows.slice(1).map((row, ri) => (
                  <tr key={ri}>
                    {row.map((cell, ci) => (
                      <td key={ci} style={{ border: '1px solid var(--border)', padding: '6px 10px', color: 'var(--text-primary)' }}>
                        {cell}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div style={{ color: 'var(--text-muted)', fontSize: 13 }}>(empty sheet)</div>
        )}
      </div>
    );
  }

  return (
    <div>
      {header('PowerPoint deck')}
      {preview.slides.length === 0 && (
        <div style={{ color: 'var(--text-muted)' }}>(no slides)</div>
      )}
      {preview.slides.map((slide, i) => (
        <div key={i} style={{ marginBottom: 16 }}>
          <div style={{ fontSize: 11, color: 'var(--text-muted)', marginBottom: 4 }}>Slide {i + 1}</div>
          {/* A real 16:9 slide surface (white background, dark text, fixed
              aspect ratio) rather than a theme-colored card — this is meant
              to look like what actually opens in PowerPoint, not like part
              of Atelier's own dark UI. Title/content boxes are positioned
              at the exact percentages of slide width/height that
              build_pptx's slide_xml uses for its title/content <a:xfrm>
              (457200,274638 / 11277600,1143000 and 457200,1600200 /
              11277600,4800600 EMU on a 12192000x6858000 EMU slide) —
              this is the actual layout our own generated .pptx has, not a
              generic approximation, though a slide from PowerPoint/Keynote/
              Google Slides with a different layout will only get the
              generic approximation. */}
          <div style={{
            position: 'relative', aspectRatio: '16 / 9', background: '#ffffff', color: '#1a1a1a',
            fontFamily: 'Calibri, "Segoe UI", Helvetica, Arial, sans-serif',
            borderRadius: 4, border: '1px solid var(--border)',
            boxShadow: '0 1px 4px rgba(0,0,0,0.25)', overflow: 'hidden',
          }}>
            {slide.title && (
              <div style={{
                position: 'absolute', left: '3.75%', top: '4%', width: '92.5%', height: '16.67%',
                display: 'flex', alignItems: 'center',
                fontSize: 'clamp(12px, 2.6vw, 20px)', fontWeight: 700,
                overflow: 'hidden', textOverflow: 'ellipsis',
              }}>
                {slide.title}
              </div>
            )}
            <div style={{
              position: 'absolute', left: '3.75%', top: '23.34%', width: '92.5%', height: '70%',
              overflow: 'auto',
            }}>
              {slide.bullets.map((bullet, bi) => (
                bullet.heading ? (
                  <div key={bi} style={{ fontSize: 'clamp(10px, 1.8vw, 14px)', fontWeight: 700, margin: '8px 0 4px' }}>
                    {bullet.text}
                  </div>
                ) : (
                  <div key={bi} style={{ display: 'flex', gap: 6, fontSize: 'clamp(9px, 1.6vw, 13px)', margin: '0 0 5px', paddingLeft: 4 }}>
                    <span style={{ color: '#666', flexShrink: 0 }}>•</span>
                    <span>{bullet.text}</span>
                  </div>
                )
              ))}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
