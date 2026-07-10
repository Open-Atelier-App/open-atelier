# Frontend Architecture

## Purpose

The frontend is the Tauri renderer process. It renders profiles, workspaces, file browsing, chat, settings, onboarding, and file previews. React is bootstrapped in [src/main.tsx](../../src/main.tsx#L7), and `App` composes the main layout in [src/App.tsx](../../src/App.tsx#L98).

## Stack

Verified frontend dependencies include React, React DOM, TypeScript, Vite, Tailwind, Radix packages, Zustand, Lucide, React Markdown, and remark-gfm ([package.json](../../package.json#L14)).

## State Management

- Profile state: list, active profile, create/switch/delete/update operations ([src/stores/profileStore.ts](../../src/stores/profileStore.ts#L5)).
- Workspace state: workspaces, active workspace, file tree, index progress ([src/stores/workspaceStore.ts](../../src/stores/workspaceStore.ts#L5)).
- Chat state: conversations, active conversation, messages, citations, tool calls, streaming state ([src/stores/chatStore.ts](../../src/stores/chatStore.ts#L5)).
- UI state: sidebars, file viewer, theme, selected model, settings modal, missing-profile recovery, CLI provider opt-in ([src/stores/uiStore.ts](../../src/stores/uiStore.ts#L4)).
- Favorites/recents: starred conversations and a recency trail, persisted to `localStorage` ([src/stores/recentsStore.ts](../../src/stores/recentsStore.ts#L40)).
- Active chats: conversations streaming or finished in the background, across every project ([src/stores/activeChatsStore.ts](../../src/stores/activeChatsStore.ts#L34)).
- Scratch widgets: ephemeral per-conversation `/timer`/`/countdown` state, never persisted or sent to the model ([src/stores/widgetStore.ts](../../src/stores/widgetStore.ts#L21)).

See [the July 2026 improvement pass reference](../reference/2026-07-improvement-pass.md) for the full feature list these stores support.

## IPC Boundary

The frontend does not call Rust commands directly throughout components. It uses a typed wrapper in [src/lib/tauri.ts](../../src/lib/tauri.ts#L10). This wrapper maps domain functions to command names such as `profile_list`, `workspace_open`, `file_read_raw`, `ask`, `cred_save`, and `skill_list`.

## Event Handling

`useTauriEvents` subscribes to backend events and updates stores:

- `chat://token`, `chat://done`, `chat://error` for chat streaming ([src/hooks/useTauriEvents.ts](../../src/hooks/useTauriEvents.ts#L21)).
- `tool://proposed` for proposed tool calls ([src/hooks/useTauriEvents.ts](../../src/hooks/useTauriEvents.ts#L33)).
- `index://progress`, `index://complete` for indexing ([src/hooks/useTauriEvents.ts](../../src/hooks/useTauriEvents.ts#L45)).
- `conversation://titled` for generated titles ([src/hooks/useTauriEvents.ts](../../src/hooks/useTauriEvents.ts#L53)).
- `menu://preferences` for the native Preferences menu ([src/hooks/useTauriEvents.ts](../../src/hooks/useTauriEvents.ts#L57)).

## Important UI Workflows

### App Startup

`App` loads profiles and enabled CLI providers on mount, then loads workspaces whenever the active profile changes ([src/App.tsx](../../src/App.tsx#L47), [src/App.tsx](../../src/App.tsx#L53)).

### Profile Setup And Recovery

`ProfileSetup` creates a profile folder under a chosen parent or uses an existing folder ([src/components/onboarding/ProfileSetup.tsx](../../src/components/onboarding/ProfileSetup.tsx#L75)). `App` checks whether the active profile path still exists and triggers missing-profile recovery if it does not ([src/App.tsx](../../src/App.tsx#L59)). `LeftSidebar` also checks profile and workspace paths before switching ([src/components/layout/LeftSidebar.tsx](../../src/components/layout/LeftSidebar.tsx#L59), [src/components/layout/LeftSidebar.tsx](../../src/components/layout/LeftSidebar.tsx#L113)).

### Workspace And File Navigation

`LeftSidebar` opens or creates workspaces and loads conversations ([src/components/layout/LeftSidebar.tsx](../../src/components/layout/LeftSidebar.tsx#L42)). `RightBar` displays the file tree and supports create, rename, delete, and open-in-viewer actions ([src/components/layout/RightBar.tsx](../../src/components/layout/RightBar.tsx#L53)).

### Chat

`ChatInput` filters selectable models to connected providers, sends messages, and disables input when no provider is connected ([src/components/chat/ChatInput.tsx](../../src/components/chat/ChatInput.tsx#L51)). `ChatView` renders conversation messages and pending tool calls ([src/components/chat/ChatView.tsx](../../src/components/chat/ChatView.tsx#L78)). `MessageBubble` renders markdown, error states, citations, provider badges, and "save as file" behavior ([src/components/chat/MessageBubble.tsx](../../src/components/chat/MessageBubble.tsx#L16)).

### File Viewer

`FileViewerPanel` lives in `App.tsx`. It reads raw file content, renders Markdown through `ReactMarkdown`, renders HTML through a sandboxed iframe, and falls back to code view for other text files ([src/App.tsx](../../src/App.tsx#L216)).

## Known Limitations

- UI components mostly use inline styles; no shared component library.
- File viewer type handling is embedded inside `App.tsx`; no viewer registry pattern yet.
- `ChatInput` uses `key_list_status` to filter connected providers, but that backend command currently reports only a subset of providers ([src/components/chat/ChatInput.tsx](../../src/components/chat/ChatInput.tsx#L37), [src-tauri/src/commands/settings.rs](../../src-tauri/src/commands/settings.rs#L111)).
