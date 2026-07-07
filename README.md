# Open Atelier

A local-first AI workspace for creatives. Open Atelier brings multi-provider LLM conversations directly into your project files — no cloud accounts, no telemetry, no data leaves your machine.

## Features

- **Multi-provider LLM chat** — OpenAI, Anthropic, Google Gemini, Mistral, Groq, Together AI, DeepSeek, xAI, OpenRouter, and Ollama (local).
- **Workspace-aware** — index your project files, search across them, and let the AI reference your content with citations.
- **File operations** — create, edit, rename, and delete files directly from the chat, including real Word/Excel/PowerPoint/PDF documents and MP3 text-to-speech narration.
- **Profile system** — separate credentials, workspaces, and skills per profile.
- **Skills** — load custom Markdown instructions that shape how the AI behaves in your workspace.
- **Local-first** — all data stored locally in SQLite. Credentials encrypted on disk. No accounts, no sync, no tracking.
- **Cross-platform** — macOS and Linux (Windows support planned).

## Architecture

Open Atelier is a [Tauri 2](https://tauri.app) desktop app:

- **Frontend:** React 19, TypeScript, Tailwind CSS, Zustand state management
- **Backend:** Rust, Tauri 2, Tokio async runtime, SQLite (WAL mode)
- **LLM routing:** Provider-agnostic — each provider has its own Rust module under `src-tauri/src/llm/`

```
src/                  # React frontend
├── components/       # UI components (chat, layout, workspace, settings)
├── stores/           # Zustand state (profile, workspace, chat, UI)
├── hooks/            # Tauri event subscriptions
└── lib/              # Types, IPC wrappers

src-tauri/src/        # Rust backend
├── commands/         # Tauri command handlers
├── llm/              # LLM provider implementations + routing
├── db/               # SQLite schema and persistence
├── indexer/          # Workspace file indexing + embeddings
└── models/           # Domain types
```

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) (v20+)
- [pnpm](https://pnpm.io/) (v9+)
- **Linux only:** system libraries for WebKitGTK, plus `espeak-ng` for MP3 text-to-speech — see [setup guide](https://doc.open-atelier.app/development/setup)

### Install & Run

```bash
pnpm install
pnpm tauri dev
```

### Build for Production

```bash
pnpm tauri build
```

Outputs a `.dmg` on macOS and `.AppImage`/`.deb` on Linux.

## Configuration

Open Atelier uses a **bring-your-own-key** model. Add your API keys in **Settings** — they are encrypted and stored locally, never transmitted anywhere except the provider's API.

Supported providers and where to get keys:

| Provider | Key URL |
|----------|---------|
| OpenAI | [platform.openai.com/api-keys](https://platform.openai.com/api-keys) |
| Anthropic | [console.anthropic.com/settings/keys](https://console.anthropic.com/settings/keys) |
| Google Gemini | [aistudio.google.com/apikey](https://aistudio.google.com/apikey) |
| Mistral AI | [console.mistral.ai](https://console.mistral.ai) |
| Groq | [console.groq.com/keys](https://console.groq.com/keys) |
| Together AI | [api.together.ai/settings/api-keys](https://api.together.ai/settings/api-keys) |
| DeepSeek | [platform.deepseek.com/api_keys](https://platform.deepseek.com/api_keys) |
| xAI (Grok) | [console.x.ai](https://console.x.ai) |
| OpenRouter | [openrouter.ai/keys](https://openrouter.ai/keys) |
| Ollama | [ollama.ai](https://ollama.ai) (local, no key needed) |

## Documentation

Full documentation is available at [doc.open-atelier.app](https://doc.open-atelier.app).

Technical contributor docs live in the [`docs/`](docs/) directory of this repository:

- [System overview](docs/architecture/overview.md)
- [Frontend architecture](docs/architecture/frontend.md)
- [Backend architecture](docs/architecture/backend.md)
- [Database architecture](docs/architecture/database.md)
- [Development setup](docs/development/setup.md)
- [Testing](docs/development/testing.md)

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Ensure `pnpm lint`, `pnpm test`, `cargo test`, and `cargo clippy` pass
5. Open a pull request

All changes must pass CI on both macOS and Ubuntu.

## Updates

Open Atelier includes built-in auto-update. Release notes and update status are available at [open-atelier.app/update](https://open-atelier.app/update).

## License

[MIT](LICENSE)

## Links

- **Website:** [open-atelier.app](https://open-atelier.app)
- **Documentation:** [doc.open-atelier.app](https://doc.open-atelier.app)
- **Updates:** [open-atelier.app/update](https://open-atelier.app/update)
