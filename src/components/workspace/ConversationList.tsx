import { useEffect, useState } from 'react';
import { Trash2, FolderPlus, Folder, ChevronDown, ChevronRight, Pencil, X } from 'lucide-react';
import { confirm as confirmDialog } from '@tauri-apps/plugin-dialog';
import type { Conversation } from '../../lib/types';
import { useChatStore } from '../../stores/chatStore';
import { useConversationGroupStore } from '../../stores/conversationGroupStore';
import { relativeTime } from '../../lib/time';

interface Props {
  workspaceId: number;
  conversations: Conversation[];
  onSelect: (conv: Conversation) => void;
}

// WebKit-based WebViews (what Tauri uses on macOS and Linux) are known to
// silently fail to read custom dataTransfer MIME types back out on drop —
// only a handful of standard types (text/plain, text/uri-list, text/html)
// are reliably readable across engines. So both draggable things here (a
// conversation row, or a folder header being reordered) go over the same
// "text/plain" slot, distinguished by a string prefix instead of a type.
const DRAG_TYPE = 'text/plain';
const CONV_PREFIX = 'atelier-conversation:';
const GROUP_PREFIX = 'atelier-group:';

function groupByDate(conversations: Conversation[]): Record<string, Conversation[]> {
  const groups: Record<string, Conversation[]> = {};
  for (const c of conversations) {
    const d = new Date(c.updated_at);
    const today = new Date();
    let key: string;
    if (d.toDateString() === today.toDateString()) {
      key = 'Today';
    } else {
      const yesterday = new Date(today);
      yesterday.setDate(yesterday.getDate() - 1);
      if (d.toDateString() === yesterday.toDateString()) {
        key = 'Yesterday';
      } else {
        key = d.toLocaleDateString('en-US', { month: 'long', day: 'numeric' });
      }
    }
    (groups[key] ??= []).push(c);
  }
  return groups;
}

export function ConversationList({ workspaceId, conversations, onSelect }: Props) {
  const deleteConversation = useChatStore(s => s.deleteConversation);
  const setConversationGroup = useChatStore(s => s.setConversationGroup);
  const groups = useConversationGroupStore(s => s.groups);
  const loadGroups = useConversationGroupStore(s => s.loadForWorkspace);
  const createGroup = useConversationGroupStore(s => s.create);
  const renameGroup = useConversationGroupStore(s => s.rename);
  const removeGroup = useConversationGroupStore(s => s.remove);
  const reorderGroups = useConversationGroupStore(s => s.reorder);

  const [hoveredId, setHoveredId] = useState<number | null>(null);
  const [collapsed, setCollapsed] = useState<Set<number>>(new Set());
  const [creatingGroup, setCreatingGroup] = useState(false);
  const [newGroupName, setNewGroupName] = useState('');
  const [editingGroupId, setEditingGroupId] = useState<number | null>(null);
  const [groupNameDraft, setGroupNameDraft] = useState('');
  const [dragOverTarget, setDragOverTarget] = useState<number | 'ungrouped' | null>(null);

  useEffect(() => { loadGroups(workspaceId); }, [workspaceId, loadGroups]);

  const byGroup = new Map<number, Conversation[]>();
  const ungrouped: Conversation[] = [];
  for (const c of conversations) {
    if (c.group_id != null) {
      const list = byGroup.get(c.group_id);
      if (list) list.push(c); else byGroup.set(c.group_id, [c]);
    } else {
      ungrouped.push(c);
    }
  }

  const handleDelete = async (e: React.MouseEvent, conv: Conversation) => {
    e.stopPropagation();
    if (!await confirmDialog(`Delete "${conv.title}"?`)) return;
    await deleteConversation(conv.id);
  };

  const handleCreateGroup = async () => {
    const name = newGroupName.trim();
    setCreatingGroup(false);
    setNewGroupName('');
    if (!name) return;
    try {
      await createGroup(workspaceId, name);
    } catch (e) {
      console.error('Failed to create folder', e);
    }
  };

  const handleRenameGroup = async (id: number) => {
    const name = groupNameDraft.trim();
    setEditingGroupId(null);
    if (!name) return;
    try {
      await renameGroup(id, name);
    } catch (e) {
      console.error('Failed to rename folder', e);
    }
  };

  const handleDeleteGroup = async (e: React.MouseEvent, id: number, name: string) => {
    e.stopPropagation();
    if (!await confirmDialog(`Delete folder "${name}"? Its conversations move back to the main list — nothing is deleted.`)) return;
    try {
      await removeGroup(id);
    } catch (err) {
      console.error('Failed to delete folder', err);
    }
  };

  const handleDrop = async (e: React.DragEvent, targetGroupId: number | null) => {
    e.preventDefault();
    setDragOverTarget(null);
    const raw = e.dataTransfer.getData(DRAG_TYPE);

    if (raw.startsWith(CONV_PREFIX)) {
      const convId = Number(raw.slice(CONV_PREFIX.length));
      try {
        await setConversationGroup(convId, targetGroupId);
      } catch (err) {
        console.error('Failed to move conversation', err);
      }
      return;
    }

    if (raw.startsWith(GROUP_PREFIX) && targetGroupId != null) {
      const draggedGroupId = Number(raw.slice(GROUP_PREFIX.length));
      const order = groups.map(g => g.id);
      const from = order.indexOf(draggedGroupId);
      const to = order.indexOf(targetGroupId);
      if (from === -1 || to === -1 || from === to) return;
      order.splice(to, 0, order.splice(from, 1)[0]);
      try {
        await reorderGroups(workspaceId, order);
      } catch (err) {
        console.error('Failed to reorder folders', err);
      }
    }
  };

  const toggleCollapsed = (id: number) => {
    setCollapsed(s => {
      const next = new Set(s);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };

  const renderConversationRow = (conv: Conversation) => (
    <div
      key={conv.id}
      draggable
      onDragStart={e => {
        e.dataTransfer.effectAllowed = 'move';
        e.dataTransfer.setData(DRAG_TYPE, `${CONV_PREFIX}${conv.id}`);
      }}
      style={{ position: 'relative', borderBottom: '1px solid var(--border)' }}
      onMouseEnter={() => setHoveredId(conv.id)}
      onMouseLeave={() => setHoveredId(null)}
    >
      <button
        onClick={() => onSelect(conv)}
        style={{
          width: '100%', height: 64, padding: '0 48px 0 24px',
          background: hoveredId === conv.id ? 'var(--overlay)' : 'none',
          border: 'none', cursor: 'pointer', textAlign: 'left',
          display: 'flex', flexDirection: 'column', justifyContent: 'center', gap: 3,
        }}
      >
        <div style={{
          fontSize: 14, fontWeight: 500, color: 'var(--text-primary)',
          overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
        }}>
          {conv.title}
        </div>
        <div style={{ fontSize: 12, color: 'var(--text-muted)', display: 'flex', gap: 8 }}>
          {conv.model && <span>{conv.model}</span>}
          <span>{relativeTime(conv.updated_at)}</span>
        </div>
      </button>
      {hoveredId === conv.id && (
        <button
          onClick={e => handleDelete(e, conv)}
          title="Delete conversation"
          style={{
            position: 'absolute', right: 12, top: '50%', transform: 'translateY(-50%)',
            background: 'none', border: 'none', cursor: 'pointer',
            color: 'var(--text-muted)', padding: 4, borderRadius: 3,
            display: 'flex', alignItems: 'center',
          }}
          onMouseEnter={e => (e.currentTarget.style.color = 'var(--error)')}
          onMouseLeave={e => (e.currentTarget.style.color = 'var(--text-muted)')}
        >
          <Trash2 size={14} />
        </button>
      )}
    </div>
  );

  if (conversations.length === 0 && groups.length === 0) {
    return (
      <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--text-muted)', fontSize: 13 }}>
        No conversations yet
      </div>
    );
  }

  const dateGroups = groupByDate(ungrouped);

  return (
    <div style={{ flex: 1, overflow: 'auto' }}>
      <div style={{ display: 'flex', justifyContent: 'flex-end', padding: '8px 24px 0' }}>
        {creatingGroup ? (
          <input
            autoFocus
            value={newGroupName}
            onChange={e => setNewGroupName(e.target.value)}
            onBlur={handleCreateGroup}
            onKeyDown={e => {
              if (e.key === 'Enter') handleCreateGroup();
              if (e.key === 'Escape') { setCreatingGroup(false); setNewGroupName(''); }
            }}
            placeholder="Folder name…"
            style={{
              fontSize: 12, padding: '3px 6px', background: 'var(--bg-app)',
              border: '1px solid var(--border)', borderRadius: 4, color: 'var(--text-primary)', outline: 'none',
            }}
          />
        ) : (
          <button
            onClick={() => setCreatingGroup(true)}
            title="Group conversations into a new folder"
            style={{
              display: 'flex', alignItems: 'center', gap: 4, background: 'none', border: 'none',
              cursor: 'pointer', color: 'var(--text-muted)', fontSize: 11, padding: '2px 4px',
            }}
          >
            <FolderPlus size={13} /> New folder
          </button>
        )}
      </div>

      {groups.map(group => {
        const groupConvs = (byGroup.get(group.id) ?? []).slice().sort((a, b) => b.updated_at - a.updated_at);
        const isCollapsed = collapsed.has(group.id);
        const isDragOver = dragOverTarget === group.id;
        return (
          <div key={group.id}>
            <div
              draggable
              onDragStart={e => {
                e.dataTransfer.effectAllowed = 'move';
                e.dataTransfer.setData(DRAG_TYPE, `${GROUP_PREFIX}${group.id}`);
              }}
              onDragOver={e => { e.preventDefault(); e.dataTransfer.dropEffect = 'move'; setDragOverTarget(group.id); }}
              onDragLeave={() => setDragOverTarget(prev => prev === group.id ? null : prev)}
              onDrop={e => handleDrop(e, group.id)}
              onClick={() => toggleCollapsed(group.id)}
              style={{
                display: 'flex', alignItems: 'center', gap: 6, padding: '8px 24px',
                cursor: 'pointer',
                background: isDragOver ? 'var(--overlay)' : 'none',
                border: '1px dashed', borderColor: isDragOver ? 'var(--accent)' : 'transparent',
              }}
            >
              {isCollapsed ? <ChevronRight size={13} color="var(--text-muted)" /> : <ChevronDown size={13} color="var(--text-muted)" />}
              <Folder size={13} color="var(--text-muted)" />
              {editingGroupId === group.id ? (
                <input
                  autoFocus
                  value={groupNameDraft}
                  onClick={e => e.stopPropagation()}
                  onChange={e => setGroupNameDraft(e.target.value)}
                  onBlur={() => handleRenameGroup(group.id)}
                  onKeyDown={e => {
                    if (e.key === 'Enter') handleRenameGroup(group.id);
                    if (e.key === 'Escape') setEditingGroupId(null);
                  }}
                  style={{
                    fontSize: 12, fontWeight: 600, padding: '1px 4px', background: 'var(--bg-app)',
                    border: '1px solid var(--accent)', borderRadius: 3, color: 'var(--text-primary)', outline: 'none',
                  }}
                />
              ) : (
                <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-primary)', flex: 1 }}>
                  {group.name}
                </span>
              )}
              <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{groupConvs.length}</span>
              <button
                onClick={e => { e.stopPropagation(); setGroupNameDraft(group.name); setEditingGroupId(group.id); }}
                title="Rename folder"
                style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', padding: 2, display: 'flex' }}
              >
                <Pencil size={12} />
              </button>
              <button
                onClick={e => handleDeleteGroup(e, group.id, group.name)}
                title="Delete folder"
                style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', padding: 2, display: 'flex' }}
              >
                <X size={12} />
              </button>
            </div>
            {!isCollapsed && groupConvs.map(renderConversationRow)}
          </div>
        );
      })}

      {groups.length > 0 && (
        <div
          onDragOver={e => { e.preventDefault(); e.dataTransfer.dropEffect = 'move'; setDragOverTarget('ungrouped'); }}
          onDragLeave={() => setDragOverTarget(prev => prev === 'ungrouped' ? null : prev)}
          onDrop={e => handleDrop(e, null)}
          style={{
            padding: '10px 24px 4px',
            fontSize: 11, fontWeight: 600, color: 'var(--text-muted)',
            letterSpacing: '0.06em', textTransform: 'uppercase',
            background: dragOverTarget === 'ungrouped' ? 'var(--overlay)' : 'none',
            border: '1px dashed', borderColor: dragOverTarget === 'ungrouped' ? 'var(--accent)' : 'transparent',
          }}
        >
          Ungrouped
        </div>
      )}

      {Object.entries(dateGroups).map(([date, convs]) => (
        <div key={date}>
          <div style={{
            padding: '10px 24px 4px',
            fontSize: 11, fontWeight: 600, color: 'var(--text-muted)',
            letterSpacing: '0.06em', textTransform: 'uppercase',
          }}>
            {date}
          </div>
          {convs.map(renderConversationRow)}
        </div>
      ))}
    </div>
  );
}
