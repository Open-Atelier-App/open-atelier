import { create } from 'zustand';

const MIN_SIDEBAR_WIDTH = 180;
const MAX_SIDEBAR_WIDTH = 480;
const MIN_RIGHT_BAR_WIDTH = 220;
const MAX_RIGHT_BAR_WIDTH = 560;
const MIN_FILE_VIEWER_WIDTH = 320;
const MAX_FILE_VIEWER_WIDTH = 900;

function loadWidth(key: string, fallback: number): number {
  const raw = localStorage.getItem(key);
  const n = raw ? Number(raw) : NaN;
  return Number.isFinite(n) && n > 0 ? n : fallback;
}

function clamp(n: number, min: number, max: number): number {
  return Math.min(Math.max(n, min), max);
}

interface UIState {
  sidebarOpen: boolean;
  rightBarOpen: boolean;
  fileViewerOpen: boolean;
  sidebarWidth: number;
  rightBarWidth: number;
  fileViewerWidth: number;
  fileViewerPath: string | null;
  // Bumped whenever a trigger writes to the file currently open in the
  // viewer, so it can tell its content-fetch effect to refetch even though
  // `fileViewerPath` itself didn't change.
  fileViewerRefreshKey: number;
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
  renameIntent: number;
  quickChatOpen: boolean;

  toggleSidebar: () => void;
  toggleRightBar: () => void;
  resizeSidebar: (deltaX: number) => void;
  resizeRightBar: (deltaX: number) => void;
  resizeFileViewer: (deltaX: number) => void;
  openFileViewer: (path: string) => void;
  closeFileViewer: () => void;
  toggleFileViewer: () => void;
  refreshFileViewer: (path: string) => void;
  setSearchOpen: (open: boolean) => void;
  setQuickChatOpen: (open: boolean) => void;
  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  setModel: (provider: string, model: string) => void;
  setShowSettings: (show: boolean) => void;
  setSettingsTab: (tab: string) => void;
  setShowOnboarding: (show: boolean) => void;
  setForceProfileSetup: (show: boolean) => void;
  setMissingProfileId: (id: number | null) => void;
  triggerNewChat: () => void;
  triggerSearchFocus: () => void;
  triggerRename: () => void;
}

export const useUIStore = create<UIState>((set) => ({
  sidebarOpen: true,
  rightBarOpen: true,
  fileViewerOpen: false,
  sidebarWidth: loadWidth('ui:sidebarWidth', 240),
  rightBarWidth: loadWidth('ui:rightBarWidth', 280),
  fileViewerWidth: loadWidth('ui:fileViewerWidth', 480),
  fileViewerPath: null,
  fileViewerRefreshKey: 0,
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
  renameIntent: 0,
  quickChatOpen: false,

  toggleSidebar: () => set(s => ({ sidebarOpen: !s.sidebarOpen })),
  toggleRightBar: () => set(s => ({ rightBarOpen: !s.rightBarOpen })),
  // Take a delta (not an absolute width) so rapid pointermove events during
  // a drag always build on the store's own latest value instead of a
  // React closure that may be stale until the next render.
  resizeSidebar: (deltaX) => set(s => {
    const sidebarWidth = clamp(s.sidebarWidth + deltaX, MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
    localStorage.setItem('ui:sidebarWidth', String(sidebarWidth));
    return { sidebarWidth };
  }),
  resizeRightBar: (deltaX) => set(s => {
    const rightBarWidth = clamp(s.rightBarWidth + deltaX, MIN_RIGHT_BAR_WIDTH, MAX_RIGHT_BAR_WIDTH);
    localStorage.setItem('ui:rightBarWidth', String(rightBarWidth));
    return { rightBarWidth };
  }),
  resizeFileViewer: (deltaX) => set(s => {
    const fileViewerWidth = clamp(s.fileViewerWidth + deltaX, MIN_FILE_VIEWER_WIDTH, MAX_FILE_VIEWER_WIDTH);
    localStorage.setItem('ui:fileViewerWidth', String(fileViewerWidth));
    return { fileViewerWidth };
  }),
  openFileViewer: (path) => set({ fileViewerOpen: true, fileViewerPath: path }),
  closeFileViewer: () => set({ fileViewerOpen: false, fileViewerPath: null }),
  toggleFileViewer: () => set(s => s.fileViewerOpen
    ? { fileViewerOpen: false, fileViewerPath: null }
    : s
  ),
  // Only bumps the refresh key when the write matches whatever's currently
  // open — otherwise a WRITE somewhere else in the project would needlessly
  // re-fetch a file that hasn't actually changed.
  refreshFileViewer: (path) => set(s =>
    s.fileViewerOpen && s.fileViewerPath === path
      ? { fileViewerRefreshKey: s.fileViewerRefreshKey + 1 }
      : s
  ),
  setSearchOpen: (open) => set({ searchOpen: open }),
  setQuickChatOpen: (open) => set({ quickChatOpen: open }),
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
  triggerRename: () => set(s => ({ renameIntent: s.renameIntent + 1 })),
}));
