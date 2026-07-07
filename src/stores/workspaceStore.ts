import { create } from 'zustand';
import type { Workspace, FileNode, IndexProgress } from '../lib/types';
import * as api from '../lib/tauri';

interface WorkspaceState {
  workspaces: Workspace[];
  active: Workspace | null;
  fileTree: FileNode[];
  indexProgress: IndexProgress | null;
  loading: boolean;
  error: string | null;

  load: (profile_id: number) => Promise<void>;
  open: (path: string, parentWorkspaceId?: number | null) => Promise<Workspace>;
  close: (id: number) => Promise<void>;
  setActive: (workspace: Workspace | null) => void;
  rename: (id: number, name: string) => Promise<void>;
  delete: (id: number) => Promise<void>;
  setParent: (id: number, parentWorkspaceId: number | null) => Promise<void>;
  setDescription: (id: number, description: string) => Promise<void>;
  loadFileTree: (workspace_id: number) => Promise<void>;
  startIndex: (workspace_id: number) => Promise<void>;
  updateIndexProgress: (progress: IndexProgress) => void;
  updateWorkspaceStatus: (workspace_id: number, status: Workspace['index_status']) => void;
  updateWorkspaceDescription: (workspace_id: number, description: string) => void;
}

export const useWorkspaceStore = create<WorkspaceState>((set, get) => ({
  workspaces: [],
  active: null,
  fileTree: [],
  indexProgress: null,
  loading: false,
  error: null,

  load: async (profile_id) => {
    set({ loading: true, error: null });
    try {
      const workspaces = await api.workspaceList(profile_id);
      set({ workspaces, loading: false });
    } catch (e) {
      set({ error: api.errorMessage(e), loading: false });
    }
  },

  open: async (path, parentWorkspaceId) => {
    const workspace = await api.workspaceOpen(path, parentWorkspaceId);
    set(s => ({
      workspaces: s.workspaces.some(w => w.id === workspace.id)
        ? s.workspaces.map(w => w.id === workspace.id ? workspace : w)
        : [...s.workspaces, workspace],
    }));
    // Kick off indexing in background (fire and forget)
    api.indexStart(workspace.id).catch(() => {/* non-fatal */});
    return workspace;
  },

  setParent: async (id, parentWorkspaceId) => {
    const workspace = await api.workspaceSetParent(id, parentWorkspaceId);
    set(s => ({ workspaces: s.workspaces.map(w => w.id === id ? workspace : w) }));
  },

  setDescription: async (id, description) => {
    const workspace = await api.workspaceSetDescription(id, description);
    set(s => ({
      workspaces: s.workspaces.map(w => w.id === id ? workspace : w),
      active: s.active?.id === id ? workspace : s.active,
    }));
  },

  close: async (id) => {
    await api.workspaceClose(id);
  },

  setActive: (workspace) => {
    set({ active: workspace, fileTree: [], indexProgress: null });
    if (workspace) {
      get().loadFileTree(workspace.id);
    }
  },

  rename: async (id, name) => {
    const workspace = await api.workspaceRename(id, name);
    set(s => ({
      workspaces: s.workspaces.map(w => w.id === id ? workspace : w),
      active: s.active?.id === id ? workspace : s.active,
    }));
  },

  delete: async (id) => {
    await api.workspaceDelete(id);
    const { active } = get();
    set(s => ({
      workspaces: s.workspaces.filter(w => w.id !== id),
      active: active?.id === id ? null : active,
    }));
  },

  loadFileTree: async (workspace_id) => {
    try {
      const fileTree = await api.fileListTree(workspace_id);
      set({ fileTree });
    } catch {
      // non-fatal
    }
  },

  startIndex: async (workspace_id) => {
    await api.indexStart(workspace_id);
    get().updateWorkspaceStatus(workspace_id, 'indexing');
  },

  updateIndexProgress: (progress) => {
    set({ indexProgress: progress });
  },

  updateWorkspaceStatus: (workspace_id, status) => {
    set(s => ({
      workspaces: s.workspaces.map(w => w.id === workspace_id ? { ...w, index_status: status } : w),
      active: s.active?.id === workspace_id ? { ...s.active, index_status: status } : s.active,
    }));
  },

  // Backend-pushed via workspace://described, once run_turn auto-generates
  // a description from context.md — see run_turn's comment for why this
  // only ever fires once (while the workspace still has no description).
  updateWorkspaceDescription: (workspace_id, description) => {
    set(s => ({
      workspaces: s.workspaces.map(w => w.id === workspace_id ? { ...w, description } : w),
      active: s.active?.id === workspace_id ? { ...s.active, description } : s.active,
    }));
  },
}));
