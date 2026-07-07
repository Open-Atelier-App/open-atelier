import { useState } from 'react';
import { Plus, Pencil, Trash2, Folder, FolderOpen, ChevronRight, ChevronsLeft } from 'lucide-react';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useUIStore } from '../../stores/uiStore';
import { useChatStore } from '../../stores/chatStore';
import { confirm as confirmDialog } from '@tauri-apps/plugin-dialog';
import * as api from '../../lib/tauri';
import type { FileNode } from '../../lib/types';
import { PlanPanel } from '../chat/PlanPanel';
import { fileTypeIcon } from '../../lib/fileIcons';

interface Props {
  collapsed: boolean;
}

export function RightBar({ collapsed }: Props) {
  const activeWorkspace = useWorkspaceStore(s => s.active);
  const fileTree = useWorkspaceStore(s => s.fileTree);
  const loadFileTree = useWorkspaceStore(s => s.loadFileTree);
  const openFileViewer = useUIStore(s => s.openFileViewer);
  const conversationSummary = useChatStore(s => s.activeConversation?.summary);
  const toggleRightBar = useUIStore(s => s.toggleRightBar);
  const [newFileName, setNewFileName] = useState('');
  const [showNewFile, setShowNewFile] = useState(false);

  if (!activeWorkspace) return null;

  if (collapsed) {
    return (
      <div style={{
        width: 40, background: 'var(--bg-sidebar)',
        borderLeft: '1px solid var(--border)',
        display: 'flex', flexDirection: 'column', alignItems: 'center', padding: '8px 0',
      }}>
        <button
          onClick={toggleRightBar}
          title="Expand panel (⌘])"
          style={{
            width: 28, height: 28, borderRadius: 4,
            background: 'none', border: 'none', cursor: 'pointer',
            color: 'var(--text-muted)', display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}
        >
          <ChevronsLeft size={16} />
        </button>
      </div>
    );
  }

  const handleCreateFile = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newFileName || !activeWorkspace) return;
    try {
      await api.fileCreate(activeWorkspace.id, newFileName, '');
      setNewFileName('');
      setShowNewFile(false);
      loadFileTree(activeWorkspace.id);
    } catch (e) {
      console.error(e);
    }
  };

  const handleDeleteFile = async (e: React.MouseEvent, node: FileNode) => {
    e.stopPropagation();
    if (!activeWorkspace) return;
    // A real native dialog rather than window.confirm() — see
    // LeftSidebar's project delete/forget for why that's not reliably a
    // blocking modal across WebView backends.
    const confirmed = await confirmDialog('This cannot be undone.', { title: `Delete ${node.name}?`, kind: 'warning' });
    if (!confirmed) return;
    try {
      await api.fileDelete(activeWorkspace.id, node.rel_path);
      loadFileTree(activeWorkspace.id);
    } catch (e) {
      console.error(e);
    }
  };

  const handleRenameFile = async (node: FileNode, newName: string) => {
    if (!activeWorkspace) return;
    const trimmed = newName.trim();
    if (!trimmed || trimmed === node.name) return;
    const parentDir = node.rel_path.includes('/')
      ? node.rel_path.slice(0, node.rel_path.lastIndexOf('/') + 1)
      : '';
    const newRelPath = `${parentDir}${trimmed}`;
    try {
      await api.fileRename(activeWorkspace.id, node.rel_path, newRelPath);
      loadFileTree(activeWorkspace.id);
    } catch (e) {
      console.error(e);
    }
  };

  return (
    <div style={{
      width: 280, background: 'var(--bg-sidebar)',
      borderLeft: '1px solid var(--border)',
      display: 'flex', flexDirection: 'column', overflow: 'hidden',
    }}>
      {conversationSummary && (
        <div style={{
          padding: '12px 12px 0', fontSize: 12, color: 'var(--text-muted)',
          lineHeight: 1.4, borderBottom: '1px solid var(--border)', paddingBottom: 12,
        }}>
          {conversationSummary}
        </div>
      )}

      {/* FILES section */}
      <div style={{ padding: '12px 12px 0' }}>
        <div style={{
          display: 'flex', alignItems: 'center', justifyContent: 'space-between',
          marginBottom: 6,
        }}>
          <span style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-muted)', letterSpacing: '0.06em', textTransform: 'uppercase' }}>
            Files
          </span>
          <div style={{ display: 'flex', alignItems: 'center', gap: 2 }}>
            <button
              onClick={() => setShowNewFile(v => !v)}
              style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', padding: 2 }}
              title="New file"
            >
              <Plus size={14} />
            </button>
            <button
              onClick={toggleRightBar}
              style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', padding: 2 }}
              title="Collapse panel (⌘])"
            >
              <ChevronRight size={14} />
            </button>
          </div>
        </div>

        {showNewFile && (
          <form onSubmit={handleCreateFile} style={{ marginBottom: 6 }}>
            <input
              autoFocus
              value={newFileName}
              onChange={e => setNewFileName(e.target.value)}
              placeholder="filename.md"
              onKeyDown={e => e.key === 'Escape' && setShowNewFile(false)}
              style={{
                width: '100%', padding: '4px 8px', fontSize: 12,
                background: 'var(--bg-surface)', border: '1px solid var(--accent)',
                borderRadius: 2, color: 'var(--text-primary)', outline: 'none',
              }}
            />
          </form>
        )}
      </div>

      {/* File tree */}
      <div style={{ flex: 1, overflow: 'auto', padding: '0 0 8px' }}>
        <FileTreeNodes
          nodes={fileTree}
          onOpen={node => openFileViewer(node.rel_path)}
          onDelete={handleDeleteFile}
          onRename={handleRenameFile}
          depth={0}
        />
      </div>

      {/* Running plan, if any — under the files panel */}
      <PlanPanel />
    </div>
  );
}

function FileTreeNodes({
  nodes, onOpen, onDelete, onRename, depth,
}: {
  nodes: FileNode[];
  onOpen: (node: FileNode) => void;
  onDelete: (e: React.MouseEvent, node: FileNode) => void;
  onRename: (node: FileNode, newName: string) => void;
  depth: number;
}) {
  return (
    <>
      {nodes.map(node => (
        <FileTreeRow key={node.rel_path} node={node} onOpen={onOpen} onDelete={onDelete} onRename={onRename} depth={depth} />
      ))}
    </>
  );
}

function FileTreeRow({
  node, onOpen, onDelete, onRename, depth,
}: {
  node: FileNode;
  onOpen: (node: FileNode) => void;
  onDelete: (e: React.MouseEvent, node: FileNode) => void;
  onRename: (node: FileNode, newName: string) => void;
  depth: number;
}) {
  const [expanded, setExpanded] = useState(true);
  const [hovered, setHovered] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState(node.name);

  const startRename = (e: React.MouseEvent) => {
    e.stopPropagation();
    setRenameValue(node.name);
    setRenaming(true);
  };

  const commitRename = () => {
    setRenaming(false);
    onRename(node, renameValue);
  };

  return (
    <>
      <div
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
        onClick={() => {
          if (renaming) return;
          if (node.is_dir) { setExpanded(v => !v); } else { onOpen(node); }
        }}
        style={{
          padding: `2px 12px 2px ${12 + depth * 12}px`,
          cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 6,
          background: hovered ? 'var(--overlay)' : 'none',
          minHeight: 24,
        }}
      >
        {renaming ? (
          <input
            autoFocus
            value={renameValue}
            onChange={e => setRenameValue(e.target.value)}
            onClick={e => e.stopPropagation()}
            onBlur={commitRename}
            onKeyDown={e => {
              if (e.key === 'Enter') { e.preventDefault(); commitRename(); }
              if (e.key === 'Escape') { e.preventDefault(); setRenaming(false); }
            }}
            style={{
              flex: 1, fontSize: 12, padding: '2px 4px',
              background: 'var(--bg-surface)', border: '1px solid var(--accent)',
              borderRadius: 2, color: 'var(--text-primary)', outline: 'none',
            }}
          />
        ) : (
          <span style={{ fontSize: 12, color: 'var(--text-muted)', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', display: 'flex', alignItems: 'center', gap: 5 }}>
            {node.is_dir ? (
              expanded ? <FolderOpen size={13} color="var(--text-muted)" /> : <Folder size={13} color="var(--text-muted)" />
            ) : (
              (() => { const { Icon, color } = fileTypeIcon(node.name); return <Icon size={13} color={color} />; })()
            )}
            <span style={{ overflow: 'hidden', textOverflow: 'ellipsis' }}>{node.name}</span>
          </span>
        )}
        {hovered && !node.is_dir && !renaming && (
          <span style={{ display: 'flex', gap: 2, flexShrink: 0 }}>
            <button
              onClick={startRename}
              style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', padding: 1 }}
              title="Rename"
            >
              <Pencil size={11} />
            </button>
            <button
              onClick={e => onDelete(e, node)}
              style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--error)', padding: 1 }}
              title="Delete"
            >
              <Trash2 size={11} />
            </button>
          </span>
        )}
      </div>
      {node.is_dir && expanded && node.children && (
        <FileTreeNodes nodes={node.children} onOpen={onOpen} onDelete={onDelete} onRename={onRename} depth={depth + 1} />
      )}
    </>
  );
}
