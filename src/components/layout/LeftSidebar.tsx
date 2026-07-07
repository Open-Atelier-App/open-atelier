import { useState } from 'react';
import { FolderOpen, Settings, ChevronDown, ChevronRight, ChevronLeft, ChevronsRight, Plus, FolderIcon, Trash2, Search } from 'lucide-react';
import logoSrc from '../../../icon.png';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useProfileStore } from '../../stores/profileStore';
import { useUIStore } from '../../stores/uiStore';
import { useChatStore } from '../../stores/chatStore';
import { open as openDialog, message as messageDialog, confirm as confirmDialog } from '@tauri-apps/plugin-dialog';
import { mkdir, exists } from '@tauri-apps/plugin-fs';
import * as api from '../../lib/tauri';
import { NewProjectDialog } from './NewProjectDialog';

interface Props {
  collapsed: boolean;
}

export function LeftSidebar({ collapsed }: Props) {
  const workspaces = useWorkspaceStore(s => s.workspaces);
  const activeWorkspace = useWorkspaceStore(s => s.active);
  const openWorkspace = useWorkspaceStore(s => s.open);
  const setActiveWorkspace = useWorkspaceStore(s => s.setActive);
  const profiles = useProfileStore(s => s.profiles);
  const activeProfile = useProfileStore(s => s.active);
  const switchProfile = useProfileStore(s => s.switch);
  const setShowSettings = useUIStore(s => s.setShowSettings);
  const setSearchOpen = useUIStore(s => s.setSearchOpen);
  const toggleSidebar = useUIStore(s => s.toggleSidebar);
  const closeConversation = useChatStore(s => s.closeConversation);
  const loadConversations = useChatStore(s => s.loadConversations);
  const deleteWorkspace = useWorkspaceStore(s => s.delete);
  const updateProfile = useProfileStore(s => s.update);
  const setForceProfileSetup = useUIStore(s => s.setForceProfileSetup);
  // Lifted to uiStore so App.tsx's active-profile-on-mount check can also
  // trigger this same recovery banner (e.g. if the directory was deleted
  // externally while the app wasn't actively switching profiles).
  const missingProfileId = useUIStore(s => s.missingProfileId);
  const setMissingProfileId = useUIStore(s => s.setMissingProfileId);
  const [profileMenuOpen, setProfileMenuOpen] = useState(false);
  const [showNewProjectDialog, setShowNewProjectDialog] = useState(false);
  const [missingWorkspace, setMissingWorkspace] = useState<typeof workspaces[0] | null>(null);
  // Which top-level projects currently show their sub-projects. Starts
  // with whichever project the active sub-project belongs to, so opening
  // a sub-project's conversation doesn't leave it hidden in a collapsed
  // parent.
  const [expandedProjects, setExpandedProjects] = useState<Set<number>>(() => {
    const parent = activeWorkspace?.parent_workspace_id;
    return parent ? new Set([parent]) : new Set();
  });

  const handleOpenFolder = async () => {
    try {
      const selected = await openDialog({ directory: true, multiple: false, defaultPath: activeProfile?.root_path });
      if (selected && typeof selected === 'string') {
        const ws = await openWorkspace(selected);
        setActiveWorkspace(ws);
        closeConversation();
        loadConversations(ws.id);
      }
    } catch (e) {
      console.error('Failed to open folder', e);
      // workspace_open rejects paths outside the active profile's folder —
      // surface that reason instead of failing silently.
      await messageDialog(api.errorMessage(e), { title: 'Could not open project', kind: 'error' });
    }
  };

  const handleSelectWorkspace = async (ws: typeof workspaces[0]) => {
    try {
      const isThere = await exists(ws.path);
      if (!isThere) {
        setMissingWorkspace(ws);
        return;
      }
    } catch {
      // If the existence check itself fails, fall through and attempt to open anyway.
    }
    setMissingWorkspace(null);
    setActiveWorkspace(ws);
    closeConversation();
    loadConversations(ws.id);
  };

  const handleLocateWorkspace = async (ws: typeof workspaces[0]) => {
    try {
      const selected = await openDialog({ directory: true, multiple: false });
      if (selected && typeof selected === 'string') {
        const updated = await api.workspaceRelocate(ws.id, selected);
        useWorkspaceStore.setState(s => ({
          workspaces: s.workspaces.map(w => w.id === ws.id ? updated : w),
        }));
        setMissingWorkspace(null);
        setActiveWorkspace(updated);
        closeConversation();
        loadConversations(updated.id);
      }
    } catch (e) {
      console.error('Failed to relocate workspace', e);
    }
  };

  const handleForgetWorkspace = async (ws: typeof workspaces[0]) => {
    // Uses the Tauri dialog plugin's native confirm rather than the
    // WebView's own window.confirm() — the latter isn't reliably a real,
    // blocking modal across WebView backends, which made this destructive
    // action too easy to trigger without a genuine confirmation step.
    const confirmed = await confirmDialog('This will not delete files on disk.', {
      title: `Forget project "${ws.name}"?`,
      kind: 'warning',
    });
    if (!confirmed) return;
    try {
      await deleteWorkspace(ws.id);
      setMissingWorkspace(null);
    } catch (e) {
      console.error('Failed to forget workspace', e);
    }
  };

  const handleDeleteWorkspace = async (e: React.MouseEvent, ws: typeof workspaces[0]) => {
    e.stopPropagation();
    const confirmed = await confirmDialog(
      'This will permanently delete the project directory and all its contents. This action cannot be undone.',
      { title: `Delete project "${ws.name}"?`, kind: 'warning' },
    );
    if (!confirmed) return;
    try {
      await deleteWorkspace(ws.id);
    } catch (e) {
      console.error('Failed to delete workspace', e);
      await messageDialog(api.errorMessage(e), { title: 'Delete failed', kind: 'error' });
    }
  };

  const handleSwitchProfile = async (id: number) => {
    const target = profiles.find(p => p.id === id);
    if (target) {
      try {
        const isThere = await exists(target.root_path);
        if (!isThere) {
          setMissingProfileId(id);
          setProfileMenuOpen(false);
          return;
        }
      } catch {
        // fall through and attempt switch anyway
      }
    }
    setMissingProfileId(null);
    await switchProfile(id);
    setProfileMenuOpen(false);
  };

  const handleLocateProfile = async (id: number) => {
    try {
      const selected = await openDialog({ directory: true, multiple: false });
      if (selected && typeof selected === 'string') {
        await updateProfile(id, { root_path: selected });
        setMissingProfileId(null);
      }
    } catch (e) {
      console.error('Failed to relocate profile', e);
      await messageDialog(`Could not relocate the profile: ${api.errorMessage(e)}`, { title: 'Relocate failed', kind: 'error' });
    }
  };

  const handleOpenProfileFolder = async (e: React.MouseEvent, path: string) => {
    e.stopPropagation();
    try {
      await api.openPath(path);
    } catch (e) {
      console.error('Failed to open profile folder', e);
      await messageDialog(`Could not open the folder: ${api.errorMessage(e)}`, { title: 'Open failed', kind: 'error' });
    }
  };

  const handleRecreateProfileDir = async (id: number) => {
    try {
      const profile = await api.profileRecreateDir(id);
      setMissingProfileId(null);
      await messageDialog(`Recreated the profile folder at:\n${profile.root_path}`, { title: 'Profile folder recreated', kind: 'info' });
    } catch (e) {
      console.error('Failed to recreate profile directory', e);
      await messageDialog(`Could not recreate the profile folder: ${api.errorMessage(e)}`, { title: 'Recreate failed', kind: 'error' });
    }
  };

  const handleCreateProject = async (fullPath: string, parentWorkspaceId: number | null) => {
    await mkdir(fullPath, { recursive: true });
    const ws = await openWorkspace(fullPath, parentWorkspaceId);
    if (parentWorkspaceId) {
      setExpandedProjects(s => new Set(s).add(parentWorkspaceId));
    }
    setActiveWorkspace(ws);
    closeConversation();
    loadConversations(ws.id);
  };

  if (collapsed) {
    return (
      <div
        style={{
          width: 52,
          background: 'var(--bg-sidebar)',
          borderRight: '1px solid var(--border)',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          padding: '8px 0',
          gap: 4,
        }}
      >
        <img src={logoSrc} alt="Open Atelier logo" style={{ height: 24, width: 'auto', marginBottom: 4 }} />
        <button
          onClick={toggleSidebar}
          title="Expand sidebar (⌘[)"
          style={{
            width: 36, height: 36, borderRadius: 4,
            background: 'none', border: 'none', cursor: 'pointer',
            color: 'var(--text-muted)', display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}
        >
          <ChevronsRight size={16} />
        </button>
        <button
          onClick={() => setShowNewProjectDialog(true)}
          title="New project"
          style={{
            width: 36, height: 36, borderRadius: 4,
            background: 'none', border: 'none', cursor: 'pointer',
            color: 'var(--text-muted)', display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}
        >
          <Plus size={18} />
        </button>
        <button
          onClick={handleOpenFolder}
          title="Open folder"
          style={{
            width: 36, height: 36, borderRadius: 4,
            background: 'none', border: 'none', cursor: 'pointer',
            color: 'var(--text-muted)', display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}
        >
          <FolderOpen size={18} />
        </button>
        <button
          onClick={() => setSearchOpen(true)}
          title="Search"
          style={{
            width: 36, height: 36, borderRadius: 4,
            background: 'none', border: 'none', cursor: 'pointer',
            color: 'var(--text-muted)', display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}
        >
          <Search size={16} />
        </button>
        {workspaces.map(ws => (
          <button
            key={ws.id}
            title={ws.name}
            onClick={() => handleSelectWorkspace(ws)}
            style={{
              width: 36, height: 36, borderRadius: 4,
              background: activeWorkspace?.id === ws.id ? 'var(--overlay)' : 'none',
              border: activeWorkspace?.id === ws.id ? '2px solid var(--accent)' : '2px solid transparent',
              cursor: 'pointer',
              color: 'var(--text-muted)', display: 'flex', alignItems: 'center', justifyContent: 'center',
            }}
          >
            <FolderIcon size={16} />
          </button>
        ))}
        <div style={{ flex: 1 }} />
        <button
          onClick={() => setShowSettings(true)}
          title="Settings"
          style={{
            width: 36, height: 36, borderRadius: 4,
            background: 'none', border: 'none', cursor: 'pointer',
            color: 'var(--text-muted)', display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}
        >
          <Settings size={18} />
        </button>
        {showNewProjectDialog && (
          <NewProjectDialog
            profileRootPath={activeProfile?.root_path ?? ''}
            topLevelProjects={workspaces.filter(w => !w.parent_workspace_id)}
            onCreate={handleCreateProject}
            onClose={() => setShowNewProjectDialog(false)}
          />
        )}
      </div>
    );
  }

  return (
    <div
      style={{
        width: 240,
        background: 'var(--bg-sidebar)',
        borderRight: '1px solid var(--border)',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
      }}
    >
      {/* Header */}
      <div style={{ padding: '16px 16px 8px', borderBottom: '1px solid var(--border)' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <img src={logoSrc} alt="Open Atelier logo" style={{ height: 24, width: 'auto' }} />
          <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-primary)', letterSpacing: '-0.01em', flex: 1 }}>
            Open Atelier
          </span>
          <button
            onClick={() => setSearchOpen(true)}
            title="Search (⌘K)"
            style={{
              width: 22, height: 22, borderRadius: 4, flexShrink: 0,
              background: 'none', border: 'none', cursor: 'pointer',
              color: 'var(--text-muted)', display: 'flex', alignItems: 'center', justifyContent: 'center',
            }}
          >
            <Search size={14} />
          </button>
          <button
            onClick={toggleSidebar}
            title="Collapse sidebar (⌘[)"
            style={{
              width: 22, height: 22, borderRadius: 4, flexShrink: 0,
              background: 'none', border: 'none', cursor: 'pointer',
              color: 'var(--text-muted)', display: 'flex', alignItems: 'center', justifyContent: 'center',
            }}
          >
            <ChevronLeft size={14} />
          </button>
        </div>
      </div>

      {/* Projects section title */}
      <div style={{ padding: '10px 16px 4px', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <span style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '0.04em' }}>
          Projects
        </span>
        <button
          onClick={() => setShowNewProjectDialog(true)}
          title="New project"
          style={{
            width: 20, height: 20, borderRadius: 4,
            background: 'none', border: 'none', cursor: 'pointer',
            color: 'var(--text-muted)', display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}
        >
          <Plus size={13} />
        </button>
      </div>

      {showNewProjectDialog && (
        <NewProjectDialog
          profileRootPath={activeProfile?.root_path ?? ''}
          topLevelProjects={workspaces.filter(w => !w.parent_workspace_id)}
          onCreate={handleCreateProject}
          onClose={() => setShowNewProjectDialog(false)}
        />
      )}

      {/* Missing workspace recovery banner */}
      {missingWorkspace && (
        <div style={{ padding: '10px 12px', background: 'var(--overlay)', borderBottom: '1px solid var(--border)' }}>
          <div style={{ fontSize: 12, color: 'var(--text-primary)', marginBottom: 6 }}>
            We couldn't find "{missingWorkspace.path}".
          </div>
          <div style={{ display: 'flex', gap: 6 }}>
            <button
              onClick={() => handleLocateWorkspace(missingWorkspace)}
              style={{ flex: 1, padding: '5px 8px', background: 'var(--accent)', border: 'none', borderRadius: 4, color: '#fff', fontSize: 11, cursor: 'pointer' }}
            >
              Locate folder
            </button>
            <button
              onClick={() => handleForgetWorkspace(missingWorkspace)}
              style={{ flex: 1, padding: '5px 8px', background: 'none', border: '1px solid var(--error)', borderRadius: 4, color: 'var(--error)', fontSize: 11, cursor: 'pointer' }}
            >
              Forget this project
            </button>
            <button
              onClick={() => setMissingWorkspace(null)}
              style={{ padding: '5px 8px', background: 'none', border: '1px solid var(--border)', borderRadius: 4, color: 'var(--text-muted)', fontSize: 11, cursor: 'pointer' }}
            >
              ✕
            </button>
          </div>
        </div>
      )}

      {/* Workspace list */}
      <div style={{ flex: 1, overflow: 'auto', padding: '4px 0' }}>
        {workspaces.length === 0 && (
          <div style={{ padding: '12px 16px', color: 'var(--text-muted)', fontSize: 12 }}>
            No projects yet
          </div>
        )}
        {workspaces.filter(ws => !ws.parent_workspace_id).map(ws => {
          const children = workspaces.filter(c => c.parent_workspace_id === ws.id);
          const expanded = expandedProjects.has(ws.id);
          return (
            <div key={ws.id}>
              <WorkspaceRow
                ws={ws}
                isActive={activeWorkspace?.id === ws.id}
                onSelect={() => handleSelectWorkspace(ws)}
                onDelete={e => handleDeleteWorkspace(e, ws)}
                hasChildren={children.length > 0}
                expanded={expanded}
                onToggleExpand={() => setExpandedProjects(s => {
                  const next = new Set(s);
                  if (next.has(ws.id)) next.delete(ws.id); else next.add(ws.id);
                  return next;
                })}
              />
              {expanded && children.map(child => (
                <WorkspaceRow
                  key={child.id}
                  ws={child}
                  isActive={activeWorkspace?.id === child.id}
                  onSelect={() => handleSelectWorkspace(child)}
                  onDelete={e => handleDeleteWorkspace(e, child)}
                  indent
                />
              ))}
            </div>
          );
        })}
      </div>

      {/* Bottom: settings + profile switcher */}
      <div style={{ borderTop: '1px solid var(--border)', padding: '8px 0' }}>
        <button
          onClick={() => setShowSettings(true)}
          style={{
            width: '100%', padding: '0 16px', height: 36,
            background: 'none', border: 'none', cursor: 'pointer',
            display: 'flex', alignItems: 'center', gap: 8,
            color: 'var(--text-muted)', fontSize: 13,
          }}
        >
          <Settings size={14} />
          Settings
        </button>

        {/* Profile switcher */}
        <div style={{ position: 'relative' }}>
          <button
            onClick={() => setProfileMenuOpen(o => !o)}
            style={{
              width: '100%', padding: '0 16px', height: 36,
              background: 'none', border: 'none', cursor: 'pointer',
              display: 'flex', alignItems: 'center', gap: 8,
              color: 'var(--text-muted)', fontSize: 13,
            }}
          >
            <div style={{
              width: 22, height: 22, borderRadius: '50%',
              background: 'var(--accent)', display: 'flex', alignItems: 'center', justifyContent: 'center',
              fontSize: 11, fontWeight: 600, color: '#fff', flexShrink: 0,
            }}>
              {(activeProfile?.name ?? 'U')[0].toUpperCase()}
            </div>
            <span style={{ flex: 1, textAlign: 'left', overflow: 'hidden', textOverflow: 'ellipsis' }}>
              {activeProfile?.name ?? 'No profile'}
            </span>
            <ChevronDown size={13} />
          </button>

          {profileMenuOpen && (
            <div style={{
              position: 'absolute', bottom: '100%', left: 8, right: 8,
              background: 'var(--bg-surface)', border: '1px solid var(--border)',
              borderRadius: 8, boxShadow: '0 4px 16px rgba(0,0,0,0.12)',
              zIndex: 100, overflow: 'hidden',
            }}>
              {profiles.map(p => (
                <div
                  key={p.id}
                  onClick={() => handleSwitchProfile(p.id)}
                  style={{
                    width: '100%', padding: '8px 12px',
                    background: p.is_active ? 'var(--overlay)' : 'none',
                    border: 'none', cursor: 'pointer',
                    textAlign: 'left', fontSize: 13,
                    color: 'var(--text-primary)', display: 'flex', alignItems: 'center', gap: 8,
                  }}
                >
                  <div style={{
                    width: 20, height: 20, borderRadius: '50%', background: 'var(--accent)',
                    display: 'flex', alignItems: 'center', justifyContent: 'center',
                    fontSize: 10, fontWeight: 600, color: '#fff', flexShrink: 0,
                  }}>
                    {p.name[0].toUpperCase()}
                  </div>
                  <div style={{ flex: 1, minWidth: 0, overflow: 'hidden' }}>
                    <div style={{ overflow: 'hidden', textOverflow: 'ellipsis' }}>{p.name}</div>
                    <div style={{ fontSize: 10, color: 'var(--text-muted)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {p.root_path}
                    </div>
                  </div>
                  {p.is_active && <span style={{ color: 'var(--accent)', fontSize: 11, flexShrink: 0 }}>✓</span>}
                  <button
                    onClick={e => handleOpenProfileFolder(e, p.root_path)}
                    title="Show in file manager"
                    style={{
                      width: 22, height: 22, borderRadius: 4, flexShrink: 0,
                      background: 'none', border: 'none', cursor: 'pointer',
                      color: 'var(--text-muted)', display: 'flex', alignItems: 'center', justifyContent: 'center',
                    }}
                  >
                    <FolderOpen size={12} />
                  </button>
                </div>
              ))}
              <button
                onClick={() => { setForceProfileSetup(true); setProfileMenuOpen(false); }}
                style={{
                  width: '100%', padding: '8px 12px',
                  background: 'none', border: 'none', borderTop: '1px solid var(--border)', cursor: 'pointer',
                  textAlign: 'left', fontSize: 13,
                  color: 'var(--text-muted)', display: 'flex', alignItems: 'center', gap: 8,
                }}
              >
                <Plus size={13} />
                New profile
              </button>
            </div>
          )}

          {/* Missing profile recovery */}
          {missingProfileId !== null && (
            <div style={{
              position: 'absolute', bottom: '100%', left: 8, right: 8,
              background: 'var(--bg-surface)', border: '1px solid var(--border)',
              borderRadius: 8, boxShadow: '0 4px 16px rgba(0,0,0,0.12)',
              zIndex: 100, padding: 10,
            }}>
              <div style={{ fontSize: 12, color: 'var(--text-primary)', marginBottom: 8 }}>
                That profile's folder couldn't be found.
              </div>
              <div style={{ display: 'flex', gap: 6 }}>
                <button
                  onClick={() => handleLocateProfile(missingProfileId)}
                  style={{ flex: 1, padding: '5px 8px', background: 'var(--accent)', border: 'none', borderRadius: 4, color: '#fff', fontSize: 11, cursor: 'pointer' }}
                >
                  Locate
                </button>
                <button
                  onClick={() => handleRecreateProfileDir(missingProfileId)}
                  style={{ flex: 1, padding: '5px 8px', background: 'none', border: '1px solid var(--border)', borderRadius: 4, color: 'var(--text-muted)', fontSize: 11, cursor: 'pointer' }}
                >
                  Recreate
                </button>
                <button
                  onClick={() => setMissingProfileId(null)}
                  style={{ padding: '5px 8px', background: 'none', border: '1px solid var(--border)', borderRadius: 4, color: 'var(--text-muted)', fontSize: 11, cursor: 'pointer' }}
                >
                  ✕
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function WorkspaceRow({
  ws, isActive, onSelect, onDelete, hasChildren, expanded, onToggleExpand, indent,
}: {
  ws: ReturnType<typeof useWorkspaceStore.getState>['workspaces'][0];
  isActive: boolean;
  onSelect: () => void;
  onDelete: (e: React.MouseEvent) => void;
  hasChildren?: boolean;
  expanded?: boolean;
  onToggleExpand?: () => void;
  indent?: boolean;
}) {
  const [hovered, setHovered] = useState(false);
  return (
    <div
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onClick={onSelect}
      style={{
        width: '100%', padding: indent ? '0 12px 0 28px' : '0 12px',
        height: 36, background: isActive ? 'var(--overlay)' : 'none',
        border: 'none', borderLeft: isActive ? '2px solid var(--accent)' : '2px solid transparent',
        cursor: 'pointer', textAlign: 'left',
        display: 'flex', alignItems: 'center', gap: 8,
      }}
    >
      {hasChildren && (
        <button
          onClick={e => { e.stopPropagation(); onToggleExpand?.(); }}
          style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', padding: 0, display: 'flex', flexShrink: 0 }}
        >
          {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </button>
      )}
      <FolderIcon size={14} color={isActive ? 'var(--accent)' : 'var(--text-muted)'} />
      <span
        style={{
          fontSize: 13, color: isActive ? 'var(--text-primary)' : 'var(--text-muted)',
          fontWeight: isActive ? 500 : 400,
          flex: 1,
          overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
        }}
      >
        {ws.name}
      </span>
      {hovered && (
        <button
          onClick={onDelete}
          title="Remove project"
          style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--error)', padding: 2, flexShrink: 0 }}
        >
          <Trash2 size={12} />
        </button>
      )}
    </div>
  );
}
