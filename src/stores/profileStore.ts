import { create } from 'zustand';
import type { Profile } from '../lib/types';
import * as api from '../lib/tauri';
import { useWorkspaceStore } from './workspaceStore';
import { useChatStore } from './chatStore';

interface ProfileState {
  profiles: Profile[];
  active: Profile | null;
  loading: boolean;
  error: string | null;

  load: () => Promise<void>;
  create: (name: string, dir_name: string, root_path: string) => Promise<Profile>;
  switch: (id: number) => Promise<void>;
  delete: (id: number) => Promise<void>;
  update: (id: number, updates: Partial<Pick<Profile, 'name' | 'dir_name' | 'root_path'>>) => Promise<void>;
}

export const useProfileStore = create<ProfileState>((set, get) => ({
  profiles: [],
  active: null,
  loading: false,
  error: null,

  load: async () => {
    set({ loading: true, error: null });
    try {
      const [profiles, active] = await Promise.all([
        api.profileList(),
        api.profileGetActive(),
      ]);
      set({ profiles, active, loading: false });
    } catch (e) {
      set({ error: api.errorMessage(e), loading: false });
    }
  },

  create: async (name, dir_name, root_path) => {
    const profile = await api.profileCreate(name, dir_name, root_path);
    set(s => ({ profiles: [...s.profiles, profile] }));
    return profile;
  },

  switch: async (id) => {
    const profile = await api.profileSwitch(id);
    set(s => ({
      active: profile,
      profiles: s.profiles.map(p => ({ ...p, is_active: p.id === id })),
    }));
    useWorkspaceStore.getState().setActive(null);
    useChatStore.getState().closeConversation();
  },

  delete: async (id) => {
    await api.profileDelete(id);
    const { active } = get();
    set(s => ({
      profiles: s.profiles.filter(p => p.id !== id),
      active: active?.id === id ? null : active,
    }));
  },

  update: async (id, updates) => {
    const profile = await api.profileUpdate(id, updates.name, updates.dir_name, updates.root_path);
    set(s => ({
      profiles: s.profiles.map(p => p.id === id ? profile : p),
      active: s.active?.id === id ? profile : s.active,
    }));
  },
}));
