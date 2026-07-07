import { useState } from 'react';
import { AlertTriangle, CheckCircle, XCircle, ChevronRight, Eye, ExternalLink, Settings } from 'lucide-react';
import type { TriggerResult, TriggerParseError } from '../../lib/types';
import { useUIStore } from '../../stores/uiStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { fileTypeIcon } from '../../lib/fileIcons';
import * as api from '../../lib/tauri';

// Actions whose file_path points at a file the user can actually open in the
// viewer (as opposed to e.g. LIST, whose "path" is a directory). The Office
// formats still show up here — clicking opens the file viewer, which offers
// an "open in default app" button for binary files (see App.tsx).
const OPENABLE_ACTIONS = new Set([
  'CREATE', 'WRITE', 'INSERT', 'APPEND', 'RENAME', 'READ', 'PREVIEW',
  'CREATE_DOCX', 'CREATE_XLSX', 'CREATE_PPTX', 'CREATE_MP3', 'EXPORT_PDF',
]);

// Human-readable past-tense label for an action, replacing the raw protocol
// action name (e.g. "CREATE") in the UI — this is a log of what the
// assistant did, not an explanation of the underlying trigger protocol.
const ACTION_LABELS: Record<string, string> = {
  CREATE: 'Created',
  WRITE: 'Edited',
  INSERT: 'Edited',
  APPEND: 'Edited',
  DELETE: 'Deleted',
  RENAME: 'Renamed to',
  READ: 'Read',
  PREVIEW: 'Previewed',
  LIST: 'Listed',
  CREATE_DOCX: 'Created',
  CREATE_XLSX: 'Created',
  CREATE_PPTX: 'Created',
  CREATE_MP3: 'Created',
  EXPORT_PDF: 'Exported',
};

// Infinitive form for the "couldn't ___" failure phrasing (ACTION_LABELS is
// past tense, which doesn't fit that sentence).
const ACTION_INFINITIVES: Record<string, string> = {
  CREATE: 'create',
  WRITE: 'edit',
  INSERT: 'edit',
  APPEND: 'edit',
  DELETE: 'delete',
  RENAME: 'rename',
  READ: 'read',
  PREVIEW: 'preview',
  LIST: 'list',
  CREATE_DOCX: 'create',
  CREATE_XLSX: 'create',
  CREATE_PPTX: 'create',
  CREATE_MP3: 'create',
  EXPORT_PDF: 'export',
};

// A "WARN" result is a documented, expected outcome (e.g. CREATE-ing a file
// to check whether it exists yet, per the context.md idiom) — not a
// failure, so it gets its own neutral phrasing rather than "Couldn't ___".
const WARN_LABELS: Record<string, string> = {
  CREATE: 'Already exists',
};

interface Props {
  results: TriggerResult[];
  errors: TriggerParseError[];
}

function ActionRow({ result }: { result: TriggerResult }) {
  const openFileViewer = useUIStore(s => s.openFileViewer);
  const setShowSettings = useUIStore(s => s.setShowSettings);
  const setSettingsTab = useUIStore(s => s.setSettingsTab);
  const activeWorkspace = useWorkspaceStore(s => s.active);
  const [expanded, setExpanded] = useState(false);
  const ok = result.status === 'OK';
  const warn = result.status === 'WARN';
  const openable = (ok || warn) && !!result.file_path && OPENABLE_ACTIONS.has(result.action);
  // Every connector-not-configured/expired message the backend produces
  // (handle_connector_read_trigger / handle_gdrive_read in commands::chat)
  // points the user at "Settings > Connectors" verbatim — a reliable,
  // connector-agnostic signal to offer a direct jump there instead of
  // leaving the user to find the tab themselves.
  const needsConnectorSetup = !ok && !warn && result.detail.includes('Settings > Connectors');
  const label = ok
    ? (ACTION_LABELS[result.action] ?? result.action)
    : warn
      ? (WARN_LABELS[result.action] ?? result.action)
      : `Couldn't ${ACTION_INFINITIVES[result.action] ?? result.action.toLowerCase()}`;
  const { Icon: FileIcon, color: fileIconColor } = result.file_path ? fileTypeIcon(result.file_path) : { Icon: null, color: '' };

  const handleOpenExternal = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!activeWorkspace || !result.file_path) return;
    api.openPath(`${activeWorkspace.path}/${result.file_path}`).catch((err: unknown) => console.error('Failed to open file', err));
  };

  const handleOpenConnectorSettings = (e: React.MouseEvent | React.KeyboardEvent) => {
    e.stopPropagation();
    setSettingsTab('Connectors');
    setShowSettings(true);
  };

  return (
    <div>
      <button
        onClick={() => setExpanded(v => !v)}
        style={{
          width: '100%', display: 'flex', alignItems: 'center', gap: 6,
          padding: '3px 0', background: 'none', border: 'none', cursor: 'pointer',
          textAlign: 'left', color: 'var(--text-muted)', font: 'inherit',
        }}
      >
        <ChevronRight size={11} style={{ flexShrink: 0, transform: expanded ? 'rotate(90deg)' : 'none', transition: 'transform 100ms' }} />
        {ok
          ? <CheckCircle size={12} color="#22c55e" style={{ flexShrink: 0 }} />
          : warn
            ? <AlertTriangle size={12} color="#f59e0b" style={{ flexShrink: 0 }} />
            : <XCircle size={12} color="var(--error)" style={{ flexShrink: 0 }} />}
        <span>{label}</span>
        {FileIcon && <FileIcon size={12} color={fileIconColor} style={{ flexShrink: 0 }} />}
        {result.file_path && <span>{result.file_path}</span>}
        {openable && (
          <div style={{ display: 'flex', gap: 4, marginLeft: 'auto', flexShrink: 0 }}>
            <span
              role="button"
              tabIndex={0}
              title="Preview in Atelier"
              onClick={e => { e.stopPropagation(); openFileViewer(result.file_path!); }}
              onKeyDown={e => { if (e.key === 'Enter') { e.stopPropagation(); openFileViewer(result.file_path!); } }}
              style={{ display: 'flex', alignItems: 'center', color: 'var(--accent)', cursor: 'pointer', padding: 2 }}
            >
              <Eye size={12} />
            </span>
            <span
              role="button"
              tabIndex={0}
              title="Open in default app"
              onClick={handleOpenExternal}
              onKeyDown={e => { if (e.key === 'Enter') handleOpenExternal(e as unknown as React.MouseEvent); }}
              style={{ display: 'flex', alignItems: 'center', color: 'var(--text-muted)', cursor: 'pointer', padding: 2 }}
            >
              <ExternalLink size={12} />
            </span>
          </div>
        )}
        {needsConnectorSetup && (
          <span
            role="button"
            tabIndex={0}
            title="Set up in Settings > Connectors"
            onClick={handleOpenConnectorSettings}
            onKeyDown={e => { if (e.key === 'Enter') handleOpenConnectorSettings(e); }}
            style={{
              display: 'flex', alignItems: 'center', gap: 4, marginLeft: 'auto', flexShrink: 0,
              color: 'var(--accent)', cursor: 'pointer', padding: '2px 4px', fontSize: 11,
            }}
          >
            <Settings size={12} /> Set up
          </span>
        )}
      </button>
      {expanded && (
        <div style={{ padding: '2px 0 6px 31px', color: 'var(--text-muted)', fontSize: 11 }}>
          {result.detail || 'No further detail.'}
          {needsConnectorSetup && (
            <div style={{ marginTop: 4 }}>
              <button
                onClick={handleOpenConnectorSettings}
                style={{
                  display: 'flex', alignItems: 'center', gap: 4, padding: '4px 8px',
                  borderRadius: 4, fontSize: 11, border: '1px solid var(--border)',
                  background: 'var(--overlay)', color: 'var(--text-primary)', cursor: 'pointer',
                }}
              >
                <Settings size={11} /> Open Connectors settings
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export function TriggerErrorBlock({ results, errors }: Props) {
  if (results.length === 0 && errors.length === 0) return null;

  // MESSAGE isn't a file operation — it's the chat text itself, so listing
  // it here alongside real actions is just noise for the user.
  const actions = results.filter(r => r.action !== 'MESSAGE');

  return (
    <div style={{
      margin: '8px 24px', padding: '6px 14px',
      background: 'var(--bg-surface)', border: '1px solid var(--border)',
      borderRadius: 8, fontSize: 12,
    }}>
      {actions.map((r, i) => <ActionRow key={i} result={r} />)}

      {errors.length > 0 && (
        <div style={{ marginTop: actions.length > 0 ? 4 : 0 }}>
          {errors.map((e, i) => (
            <div key={i} style={{ display: 'flex', alignItems: 'flex-start', gap: 6, padding: '3px 0', color: 'var(--text-muted)' }}>
              <AlertTriangle size={12} color="#f59e0b" style={{ flexShrink: 0, marginTop: 2 }} />
              <span>
                Didn't understand an instruction: {e.message}
                {e.suggestion && <span style={{ color: '#3b82f6' }}> — did you mean: {e.suggestion}?</span>}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
