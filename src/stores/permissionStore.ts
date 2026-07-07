import { create } from 'zustand';
import type { PermissionConfig, TriggerResult, TriggerParseError } from '../lib/types';
import * as api from '../lib/tauri';

interface PermissionState {
  config: PermissionConfig | null;
  activeLevel: Record<string, string>;
  triggerResults: TriggerResult[];
  triggerErrors: TriggerParseError[];

  loadConfig: () => Promise<void>;
  getLevel: (provider: string) => Promise<string>;
  setLevel: (provider: string, level: string) => Promise<string[]>;
  addTriggerResult: (result: TriggerResult) => void;
  addTriggerError: (error: TriggerParseError) => void;
  clearTriggerFeedback: () => void;
}

export const usePermissionStore = create<PermissionState>((set, get) => ({
  config: null,
  activeLevel: {},
  triggerResults: [],
  triggerErrors: [],

  loadConfig: async () => {
    try {
      const config = await api.getPermissionConfig();
      set({ config });
    } catch {
      // Config not available (e.g. dev mode without bundled resources)
    }
  },

  getLevel: async (provider) => {
    const cached = get().activeLevel[provider];
    if (cached) return cached;
    try {
      const level = await api.getPermissionLevel(provider);
      set(s => ({ activeLevel: { ...s.activeLevel, [provider]: level } }));
      return level;
    } catch {
      return 'chat_only';
    }
  },

  setLevel: async (provider, level) => {
    const triggers = await api.setPermissionLevel(provider, level);
    set(s => ({ activeLevel: { ...s.activeLevel, [provider]: level } }));
    return triggers;
  },

  addTriggerResult: (result) => {
    set(s => ({ triggerResults: [...s.triggerResults, result] }));
  },

  addTriggerError: (error) => {
    set(s => ({ triggerErrors: [...s.triggerErrors, error] }));
  },

  clearTriggerFeedback: () => {
    set({ triggerResults: [], triggerErrors: [] });
  },
}));
