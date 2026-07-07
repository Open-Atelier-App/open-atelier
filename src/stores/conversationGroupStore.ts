import { create } from 'zustand';
import type { ConversationGroup } from '../lib/types';
import * as api from '../lib/tauri';

interface ConversationGroupState {
  groups: ConversationGroup[];

  loadForWorkspace: (workspaceId: number) => Promise<void>;
  create: (workspaceId: number, name: string) => Promise<void>;
  rename: (id: number, name: string) => Promise<void>;
  remove: (id: number) => Promise<void>;
  reorder: (workspaceId: number, orderedIds: number[]) => Promise<void>;
  clear: () => void;
}

export const useConversationGroupStore = create<ConversationGroupState>((set) => ({
  groups: [],

  loadForWorkspace: async (workspaceId) => {
    try {
      const groups = await api.conversationGroupList(workspaceId);
      set({ groups });
    } catch {
      set({ groups: [] });
    }
  },

  create: async (workspaceId, name) => {
    const group = await api.conversationGroupCreate(workspaceId, name);
    set(s => ({ groups: [...s.groups, group] }));
  },

  rename: async (id, name) => {
    const group = await api.conversationGroupRename(id, name);
    set(s => ({ groups: s.groups.map(g => g.id === id ? group : g) }));
  },

  remove: async (id) => {
    await api.conversationGroupDelete(id);
    set(s => ({ groups: s.groups.filter(g => g.id !== id) }));
  },

  reorder: async (workspaceId, orderedIds) => {
    const groups = await api.conversationGroupReorder(workspaceId, orderedIds);
    set({ groups });
  },

  clear: () => set({ groups: [] }),
}));
