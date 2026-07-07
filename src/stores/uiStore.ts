import { create } from 'zustand';

interface UIState {
  sidebarOpen: boolean;
  rightBarOpen: boolean;
  fileViewerOpen: boolean;
  fileViewerPath: string | null;
  searchOpen: boolean;
  theme: 'light' | 'dark' | 'system';
  selectedProvider: string;
  selectedModel: string;
  showSettings: boolean;
  settingsTab: string;
  showOnboarding: boolean;
  forceProfileSetup: boolean;
  missingProfileId: number | null;
  newChatIntent: number;
  searchFocusIntent: number;

  toggleSidebar: () => void;
  toggleRightBar: () => void;
  openFileViewer: (path: string) => void;
  closeFileViewer: () => void;
  toggleFileViewer: () => void;
  setSearchOpen: (open: boolean) => void;
  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  setModel: (provider: string, model: string) => void;
  setShowSettings: (show: boolean) => void;
  setSettingsTab: (tab: string) => void;
  setShowOnboarding: (show: boolean) => void;
  setForceProfileSetup: (show: boolean) => void;
  setMissingProfileId: (id: number | null) => void;
  triggerNewChat: () => void;
  triggerSearchFocus: () => void;
}

export const useUIStore = create<UIState>((set) => ({
  sidebarOpen: true,
  rightBarOpen: true,
  fileViewerOpen: false,
  fileViewerPath: null,
  searchOpen: false,
  theme: 'system',
  selectedProvider: 'anthropic',
  selectedModel: 'claude-sonnet-4-6',
  showSettings: false,
  settingsTab: 'Profiles',
  showOnboarding: false,
  forceProfileSetup: false,
  missingProfileId: null,
  newChatIntent: 0,
  searchFocusIntent: 0,

  toggleSidebar: () => set(s => ({ sidebarOpen: !s.sidebarOpen })),
  toggleRightBar: () => set(s => ({ rightBarOpen: !s.rightBarOpen })),
  openFileViewer: (path) => set({ fileViewerOpen: true, fileViewerPath: path }),
  closeFileViewer: () => set({ fileViewerOpen: false, fileViewerPath: null }),
  toggleFileViewer: () => set(s => s.fileViewerOpen
    ? { fileViewerOpen: false, fileViewerPath: null }
    : s
  ),
  setSearchOpen: (open) => set({ searchOpen: open }),
  setTheme: (theme) => {
    set({ theme });
    const root = document.documentElement;
    if (theme === 'system') {
      root.removeAttribute('data-theme');
    } else {
      root.setAttribute('data-theme', theme);
    }
  },
  setModel: (provider, model) => set({ selectedProvider: provider, selectedModel: model }),
  setShowSettings: (show) => set({ showSettings: show }),
  setSettingsTab: (tab) => set({ settingsTab: tab }),
  setShowOnboarding: (show) => set({ showOnboarding: show }),
  setForceProfileSetup: (show) => set({ forceProfileSetup: show }),
  setMissingProfileId: (id) => set({ missingProfileId: id }),
  triggerNewChat: () => set(s => ({ newChatIntent: s.newChatIntent + 1 })),
  triggerSearchFocus: () => set(s => ({ searchFocusIntent: s.searchFocusIntent + 1 })),
}));
