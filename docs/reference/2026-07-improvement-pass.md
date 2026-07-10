# July 2026 Improvement Pass

Reference for everything added on `claude/open-atelier-improvements-yj8swm`, working through a
French-language improvement report point by point. Grouped by area; each entry says what it does,
where it lives, and any scope decision worth knowing about.

## Auto-updater & release infra

- **Auto-updater pubkey** — [src-tauri/tauri.conf.json](../../src-tauri/tauri.conf.json) now carries
  the signing key from the report rather than the one previously committed. The update endpoint
  contract itself (`open-atelier.app/update`) was left untouched, per explicit instruction that it's
  already live in production.
- **macOS icon padding** — `src-tauri/icons/*` regenerated from `icon-macos.png`, padded to square
  with transparent fill instead of the cropped source.
- **Release workflow** — [.github/workflows/release.yml](../../.github/workflows/release.yml)
  rewritten: a test job, frozen lockfile installs, artifact renaming to the
  `Open-Atelier-<version>-<target>.tar.gz` contract, checksums, and a draft GitHub Release job.

## Composer (`ChatInput`)

- **Draft autosave** — an unsent message survives switching conversations and reloading the app.
  `draftKeyFor` computes a `localStorage` key per conversation (or per workspace before one exists)
  ([src/components/chat/ChatInput.tsx](../../src/components/chat/ChatInput.tsx#L23)); a lazy
  `useState` initializer restores it on first mount, and a render-time comparison against the last
  seen key re-loads it when the conversation changes ([ChatInput.tsx](../../src/components/chat/ChatInput.tsx#L84)).
- **`@` file/folder mentions** — typing `@` opens an autocomplete over the workspace's file tree
  (`activeMentionAt`, `flattenFileTree` in ChatInput.tsx), arrow keys to navigate, Tab/Enter to
  insert the path.
- **Image attachments** — drag-drop, paste, or the picker button save an image into the workspace's
  `attachments/` folder via the new `image_attachment_save` Tauri command
  ([src-tauri/src/commands/files.rs](../../src-tauri/src/commands/files.rs)) and prepend a
  `![name](path)` reference — the same markdown-image path already used elsewhere, so it renders
  inline with no separate code path.
- **Slash-command palette** — typing `/` at the start of an empty message opens a picker of
  shortcuts to actions already wired to buttons elsewhere: `/new`, `/rename`, `/compress`, `/fork`,
  `/timer`, `/countdown`, `/search`, `/help` (`baseSlashCommands` in
  [ChatInput.tsx](../../src/components/chat/ChatInput.tsx#L155)). Conversation-scoped ones only
  appear once there's an active conversation for them to act on. **Deliberately not built**: the
  report's fuller "Commands" proposal (report section 2) — direct-execution commands that bypass the
  model to run backend actions, plus a user config file and a plugin system. That's a real
  architecture and product-scope decision, escalated rather than guessed at; this palette is the
  scoped-down version that was approved.
- **Model & permission selector repositioning** — both moved out of the composer's input pill into
  their own toolbar row above it, so the typing area stays free of controls and isn't visually
  cluttered (report 7.7 / 7.12).
- **Resizable panels** — left sidebar, right bar, and file viewer each take a drag handle
  (`ResizeHandle`), widths clamped and persisted to `localStorage` via `uiStore`'s
  `resizeSidebar` / `resizeRightBar` / `resizeFileViewer`.
- **File viewer no longer covers the composer** — the preview panel is a flex sibling of the chat
  column now, not an absolutely-positioned overlay, so the chat column actually shrinks to make room
  for it instead of the panel floating on top of the composer's right edge.

## Sidebar

- **Favorites & recents** — [src/stores/recentsStore.ts](../../src/stores/recentsStore.ts#L40)
  tracks starred conversations and a running "recent" trail (max 10), persisted to `localStorage`.
  Surfaced as new FAVORITES / RECENT sections in
  [LeftSidebar.tsx](../../src/components/layout/LeftSidebar.tsx#L20); a star toggle also lives in
  `ConversationList`.
- **Active / parallel chats panel** — an ACTIVE sidebar section tracks conversations streaming in
  the background across every project, not just the one on screen — a spinner while generating, a
  dot once it finishes somewhere you weren't looking.
  [src/stores/activeChatsStore.ts](../../src/stores/activeChatsStore.ts#L34) maps message IDs to
  conversations (the only thing `chat://token`/`chat://done`/`chat://error` events actually carry),
  since resolving that server-side would have meant changing the Rust event payloads for a purely
  client-side bookkeeping problem.
- **Quick Chat modal** — `⌘⇧N` opens a small composer
  ([src/components/chat/QuickChatModal.tsx](../../src/components/chat/QuickChatModal.tsx#L20)) that
  creates a conversation and sends a message via direct API calls, deliberately bypassing
  `chatStore`'s normal `sendMessage`/`activeConversation` mutation — that would otherwise yank the
  user over to the new conversation instead of leaving whatever they were looking at undisturbed.

## Chat message viewers

Everything below is a `code` component override in `MarkdownContent`
([src/components/chat/MessageBubble.tsx](../../src/components/chat/MessageBubble.tsx#L233)), keyed
off the fenced code block's language tag. Malformed input for any structured (JSON) block falls back
to a plain code block rather than crashing the message.

| Fence | Renders | Parser / component |
|---|---|---|
| `$$...$$` (not fenced — inline in prose) | KaTeX block math | `wrapMathBlocks` rewrites it to a `` ```math `` block first; `MathBlock` renders it |
| `` ```chart `` | Bar / line / pie chart, hand-rolled SVG | [src/lib/chartSpec.ts](../../src/lib/chartSpec.ts#L14) → `ChartBlock` ([ChartBlock.tsx](../../src/components/chat/ChartBlock.tsx#L46)) |
| `` ```mermaid `` | Real diagram (flowchart, sequence, etc.) via the `mermaid` package | `MermaidDiagram` ([MermaidDiagram.tsx](../../src/components/chat/MermaidDiagram.tsx#L15)) |
| `` ```recipe `` | Structured recipe card (title, prep/cook time, ingredients, steps, notes) | [vizSpecs.ts](../../src/lib/vizSpecs.ts#L19) → `RecipeCard` ([InfoCards.tsx](../../src/components/chat/InfoCards.tsx#L6)) |
| `` ```map `` | Location card + "Open in Maps" button (OpenStreetMap, no API key) | [vizSpecs.ts](../../src/lib/vizSpecs.ts#L49) → `MapCard` ([InfoCards.tsx](../../src/components/chat/InfoCards.tsx#L56)) |
| `` ```kanban `` | Static read-only column/card board | [vizSpecs.ts](../../src/lib/vizSpecs.ts#L87) → `KanbanBoard` ([InfoCards.tsx](../../src/components/chat/InfoCards.tsx#L94)) |
| `` ```weather `` | Weather card for data the model already has | [vizSpecs.ts](../../src/lib/vizSpecs.ts#L114) → `WeatherCard` ([InfoCards.tsx](../../src/components/chat/InfoCards.tsx#L138)) |
| A link to `.mp3`/`.wav`/`.ogg` | Inline `<audio>` player instead of plain text | `a` component override in MessageBubble.tsx |
| Blockquotes, tables | Styled to match the file viewer's existing look | `blockquote` / `table` / `thead` / `th` / `td` overrides in MessageBubble.tsx |

Two scope notes worth keeping in mind:

- **Math is block-only.** Inline `$...$` is not supported — there's no reliable way to tell it apart
  from an ordinary price mention like "$5 to $10" in prose.
- **Map and weather are display components, not live lookups.** Both render structured data the
  model already produced; neither calls a maps or weather API. Building real lookups would mean new
  API key infrastructure this app doesn't have yet — a product decision, not a rendering one.
- **Charts use a hand-rolled SVG renderer**, not a charting library — the categorical palette
  (`CATEGORICAL` in ChartBlock.tsx) was validated with the dataviz skill's CVD/contrast checker
  before shipping, rather than picked by eye. Mermaid is the one exception that pulled in a real
  dependency, since reimplementing its layout engine by hand isn't reasonable.

## Scratch widgets (`/timer`, `/countdown`)

[src/stores/widgetStore.ts](../../src/stores/widgetStore.ts#L21) tracks ephemeral, per-conversation
widgets — not messages, not persisted, never sent to the model. `ConversationWidgets`
([src/components/chat/ConversationWidgets.tsx](../../src/components/chat/ConversationWidgets.tsx#L210))
renders them below the message list, above the composer. The stopwatch and countdown both track
timestamps in state rather than reading the clock during render, to satisfy the stricter
`react-hooks/purity` and `set-state-in-effect` lint rules now in force in this codebase — a real
constraint worth knowing about before writing a similar ticking-clock component elsewhere.

## What's still out of scope

Lower-priority per the report's own priority table, or explicitly speculative rather than a concrete
ask — not started:

- Recipe/kanban *interactivity* beyond static display (drag-drop reordering, checking off
  ingredients) — the report only asked for a display card, not a full editor.
- The report's remaining "Autres Idées" grab-bag items that never made even a loose priority list:
  image carousels, polls, a calendar view, in-chat code execution, translation side-by-side.
