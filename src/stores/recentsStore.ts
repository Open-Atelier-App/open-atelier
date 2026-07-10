import { create } from 'zustand';

export interface RecentEntry {
  conversationId: number;
  workspaceId: number;
  title: string;
}

export interface FavoriteWorkspaceEntry {
  workspaceId: number;
  name: string;
}

interface RecentsState {
  favorites: RecentEntry[];
  favoriteWorkspaces: FavoriteWorkspaceEntry[];
  recents: RecentEntry[];
  isFavorite: (conversationId: number) => boolean;
  toggleFavorite: (entry: RecentEntry) => void;
  isWorkspaceFavorite: (workspaceId: number) => boolean;
  toggleWorkspaceFavorite: (entry: FavoriteWorkspaceEntry) => void;
  removeWorkspaceEntry: (workspaceId: number) => void;
  recordOpened: (entry: RecentEntry) => void;
  renameEntry: (conversationId: number, title: string) => void;
  removeEntry: (conversationId: number) => void;
}

const FAVORITES_KEY = 'atelier:favorites';
const FAVORITE_WORKSPACES_KEY = 'atelier:favorite-workspaces';
const RECENTS_KEY = 'atelier:recents';
const MAX_RECENTS = 10;

function load<T>(key: string): T[] {
  try {
    const raw = localStorage.getItem(key);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function save<T>(key: string, entries: T[]) {
  localStorage.setItem(key, JSON.stringify(entries));
}

// Quick access to favorited/recently-opened conversations across projects,
// shown above the project tree in the left sidebar — deliberately
// localStorage-only for now (no backend sync across devices), same as the
// per-conversation chat drafts.
export const useRecentsStore = create<RecentsState>((set, get) => ({
  favorites: load<RecentEntry>(FAVORITES_KEY),
  favoriteWorkspaces: load<FavoriteWorkspaceEntry>(FAVORITE_WORKSPACES_KEY),
  recents: load<RecentEntry>(RECENTS_KEY),

  isFavorite: (conversationId) => get().favorites.some(f => f.conversationId === conversationId),

  toggleFavorite: (entry) => {
    const { favorites } = get();
    const next = favorites.some(f => f.conversationId === entry.conversationId)
      ? favorites.filter(f => f.conversationId !== entry.conversationId)
      : [entry, ...favorites];
    save(FAVORITES_KEY, next);
    set({ favorites: next });
  },

  isWorkspaceFavorite: (workspaceId) => get().favoriteWorkspaces.some(f => f.workspaceId === workspaceId),

  toggleWorkspaceFavorite: (entry) => {
    const { favoriteWorkspaces } = get();
    const next = favoriteWorkspaces.some(f => f.workspaceId === entry.workspaceId)
      ? favoriteWorkspaces.filter(f => f.workspaceId !== entry.workspaceId)
      : [entry, ...favoriteWorkspaces];
    save(FAVORITE_WORKSPACES_KEY, next);
    set({ favoriteWorkspaces: next });
  },

  // Drops a deleted project from favorites so it doesn't dangle.
  removeWorkspaceEntry: (workspaceId) => {
    const { favoriteWorkspaces } = get();
    const next = favoriteWorkspaces.filter(f => f.workspaceId !== workspaceId);
    save(FAVORITE_WORKSPACES_KEY, next);
    set({ favoriteWorkspaces: next });
  },

  recordOpened: (entry) => {
    const { recents } = get();
    const next = [entry, ...recents.filter(r => r.conversationId !== entry.conversationId)].slice(0, MAX_RECENTS);
    save(RECENTS_KEY, next);
    set({ recents: next });
  },

  // Keeps the sidebar's cached titles in sync when a conversation gets
  // renamed (manually or via auto-titling) after being favorited/opened.
  renameEntry: (conversationId, title) => {
    const { favorites, recents } = get();
    const rename = (list: RecentEntry[]) => list.map(e => e.conversationId === conversationId ? { ...e, title } : e);
    const nextFavorites = rename(favorites);
    const nextRecents = rename(recents);
    save(FAVORITES_KEY, nextFavorites);
    save(RECENTS_KEY, nextRecents);
    set({ favorites: nextFavorites, recents: nextRecents });
  },

  // Drops a deleted conversation from both lists so they don't dangle.
  removeEntry: (conversationId) => {
    const { favorites, recents } = get();
    const nextFavorites = favorites.filter(f => f.conversationId !== conversationId);
    const nextRecents = recents.filter(r => r.conversationId !== conversationId);
    save(FAVORITES_KEY, nextFavorites);
    save(RECENTS_KEY, nextRecents);
    set({ favorites: nextFavorites, recents: nextRecents });
  },
}));
