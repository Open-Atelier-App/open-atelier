// Types mirroring Rust structs (kept in sync with src-tauri/src/models/mod.rs)

export interface Profile {
  id: number;
  name: string;
  dir_name: string;
  root_path: string;
  created_at: number;
  last_active_at: number;
  is_active: boolean;
}

export interface Workspace {
  id: number;
  profile_id: number;
  path: string;
  name: string;
  created_at: number;
  last_opened_at: number;
  index_status: 'idle' | 'indexing' | 'complete' | 'error';
  settings_json: string | null;
  // A sub-project's parent, if any — see workspaceSetParent/workspaceOpen.
  parent_workspace_id: number | null;
  // One-sentence, user-editable summary shown under the project name.
  description: string | null;
}

export interface WorkspaceFile {
  id: number;
  workspace_id: number;
  rel_path: string;
  abs_path: string;
  ext: string | null;
  size_bytes: number;
  mtime: number;
  content_hash: string | null;
  index_state: 'pending' | 'indexed' | 'skipped' | 'error';
  skip_reason: string | null;
  indexed_at: number | null;
}

export interface FileNode {
  name: string;
  rel_path: string;
  is_dir: boolean;
  children?: FileNode[];
  size_bytes?: number;
  ext?: string;
}

export type DocxBlock =
  | { kind: 'heading1'; text: string }
  | { kind: 'heading2'; text: string }
  | { kind: 'heading3'; text: string }
  | { kind: 'bullet'; text: string }
  | { kind: 'paragraph'; text: string };

export interface XlsxSheet {
  name: string;
  rows: string[][];
}

export interface PptxBullet {
  text: string;
  // True for a nested markdown sub-heading ("## " inside a slide's body,
  // under the slide's own "# " title) — rendered bold, no bullet dot.
  heading: boolean;
}

export interface PptxSlide {
  title: string;
  bullets: PptxBullet[];
}

export type OfficePreview =
  | { kind: 'docx'; blocks: DocxBlock[] }
  | { kind: 'xlsx'; sheets: XlsxSheet[] }
  | { kind: 'pptx'; slides: PptxSlide[] };

export interface Conversation {
  id: number;
  workspace_id: number;
  title: string;
  created_at: number;
  updated_at: number;
  provider: string | null;
  model: string | null;
  // Short LLM-generated resume of the conversation, shown above the files
  // panel — refreshed after each settled turn once there's enough to
  // summarize. See conversation://summarized.
  summary: string | null;
  // Set once the conversation has been compressed (see conversationCompress)
  // — a thorough memory of everything before compressed_at, used instead of
  // the full transcript on future turns so the user can keep chatting in
  // the same thread, optionally with a different provider/model.
  compressed_memory: string | null;
  compressed_at: number | null;
  // Which user-defined folder (see ConversationGroup) this conversation is
  // filed under, if any — set via drag-and-drop in ConversationList.
  group_id: number | null;
}

// A user-defined folder for organizing a workspace's conversations by
// subject, created/renamed/reordered by the user via drag-and-drop.
export interface ConversationGroup {
  id: number;
  workspace_id: number;
  name: string;
  position: number;
  created_at: number;
}

export interface Message {
  id: number;
  conversation_id: number;
  role: 'system' | 'user' | 'assistant';
  content: string;
  created_at: number;
  token_count: number | null;
  input_tokens: number | null;
  output_tokens: number | null;
  error: string | null;
  status: 'streaming' | 'complete' | 'error' | 'cancelled';
  provider: string | null;
  model: string | null;
  // Set only for a turn that was all triggers and no chat prose — a
  // synthesized "Created X, edited Y." confirmation to show instead of an
  // empty bubble. `content` itself always stays the model's raw, unedited
  // output (also used as this conversation's history), so render this in
  // place of stripping triggers from `content` whenever it's present.
  display_override: string | null;
}

export type PlanStatus = 'pending' | 'running' | 'done' | 'failed';

export interface Plan {
  id: number;
  conversation_id: number;
  title: string;
  status: PlanStatus;
  created_at: number;
}

export interface PlanTask {
  id: number;
  plan_id: number;
  seq: number;
  description: string;
  status: PlanStatus;
  summary: string | null;
  created_at: number;
  updated_at: number;
}

export interface PlanWithTasks {
  plan: Plan;
  tasks: PlanTask[];
}

export interface Citation {
  id: number;
  message_id: number;
  chunk_id: number | null;
  file_id: number | null;
  rel_path: string;
  page: number | null;
  heading: string | null;
  snippet: string;
  rank: number;
  score: number;
}

export interface ToolCall {
  id: number;
  message_id: number;
  tool_name: string;
  arguments_json: string;
  status: 'pending' | 'approved' | 'rejected' | 'executed' | 'error';
  result_json: string | null;
  created_at: number;
}

export interface IndexProgress {
  workspace_id: number;
  done: number;
  total: number;
  current_file: string | null;
}

export interface ChatToken {
  message_id: number;
  delta: string;
}

export interface ChatDone {
  message_id: number;
  citations: Citation[];
  has_more: boolean;
  display_override: string | null;
}

export interface ChatContinuation {
  message: Message;
}

export interface ChatError {
  message_id: number;
  error: AtelierError;
}

export interface ToolProposed {
  tool_call_id: number;
  tool_name: string;
  arguments: Record<string, unknown>;
}

export interface AtelierError {
  code: string;
  message: string;
  detail: string | null;
}

export interface KeyStatus {
  provider: string;
  exists: boolean;
  /** Which storage backend the credential lives in: "keychain" | "local_encrypted" | "none". */
  backend: string;
}

export type Provider =
  | 'openai' | 'openai-codex'
  | 'anthropic'
  | 'google'
  | 'ollama'
  | 'groq' | 'openrouter' | 'mistral' | 'together' | 'deepseek' | 'xai';

export interface ModelOption {
  id: string;
  name: string;
  provider: Provider;
}

export const MODEL_OPTIONS: ModelOption[] = [
  { id: 'gpt-4o-mini', name: 'GPT-4o mini', provider: 'openai' },
  { id: 'gpt-4o', name: 'GPT-4o', provider: 'openai' },
  { id: 'o4-mini', name: 'o4-mini', provider: 'openai' },
  { id: 'codex-mini-latest', name: 'Codex Mini', provider: 'openai-codex' },
  { id: 'claude-haiku-4-5', name: 'Claude Haiku', provider: 'anthropic' },
  { id: 'claude-sonnet-4-6', name: 'Claude Sonnet', provider: 'anthropic' },
  { id: 'gemini-2.0-flash', name: 'Gemini Flash', provider: 'google' },
  { id: 'gemini-2.5-pro', name: 'Gemini Pro', provider: 'google' },
  { id: 'llama3', name: 'Llama 3 (Ollama)', provider: 'ollama' },
  { id: 'mistral', name: 'Mistral (Ollama)', provider: 'ollama' },
  { id: 'llama-3.3-70b-versatile', name: 'Llama 3.3 70B (Groq)', provider: 'groq' },
  { id: 'openai/gpt-4o-mini', name: 'GPT-4o mini (OpenRouter)', provider: 'openrouter' },
  { id: 'mistral-small-latest', name: 'Mistral Small', provider: 'mistral' },
  { id: 'mistral-medium-latest', name: 'Mistral Medium', provider: 'mistral' },
  { id: 'mistral-large-latest', name: 'Mistral Large', provider: 'mistral' },
  { id: 'codestral-latest', name: 'Codestral', provider: 'mistral' },
  { id: 'pixtral-large-latest', name: 'Pixtral Large', provider: 'mistral' },
  { id: 'meta-llama/Llama-3.3-70B-Instruct-Turbo', name: 'Llama 3.3 70B (Together)', provider: 'together' },
  { id: 'deepseek-chat', name: 'DeepSeek Chat', provider: 'deepseek' },
  { id: 'grok-3', name: 'Grok 3', provider: 'xai' },
  { id: 'grok-3-mini', name: 'Grok 3 Mini', provider: 'xai' },
];

export interface ProviderRegistryEntry {
  id: string;
  name: string;
  type: 'api_key';
  keyUrl: string;
  credType: 'api_key' | 'bearer_token' | 'azure' | 'ollama_url';
}

export const PROVIDER_REGISTRY: ProviderRegistryEntry[] = [
  { id: 'anthropic', name: 'Anthropic', type: 'api_key', keyUrl: 'https://console.anthropic.com/settings/keys', credType: 'api_key' },
  { id: 'openai', name: 'OpenAI', type: 'api_key', keyUrl: 'https://platform.openai.com/api-keys', credType: 'api_key' },
  { id: 'google', name: 'Google Gemini', type: 'api_key', keyUrl: 'https://aistudio.google.com/apikey', credType: 'api_key' },
  { id: 'ollama', name: 'Ollama (local)', type: 'api_key', keyUrl: 'https://ollama.ai', credType: 'ollama_url' },
  { id: 'groq', name: 'Groq', type: 'api_key', keyUrl: 'https://console.groq.com/keys', credType: 'api_key' },
  { id: 'openrouter', name: 'OpenRouter', type: 'api_key', keyUrl: 'https://openrouter.ai/keys', credType: 'api_key' },
  { id: 'mistral', name: 'Mistral AI', type: 'api_key', keyUrl: 'https://console.mistral.ai', credType: 'api_key' },
  { id: 'together', name: 'Together AI', type: 'api_key', keyUrl: 'https://api.together.ai/settings/api-keys', credType: 'api_key' },
  { id: 'deepseek', name: 'DeepSeek', type: 'api_key', keyUrl: 'https://platform.deepseek.com/api_keys', credType: 'api_key' },
  { id: 'xai', name: 'xAI (Grok)', type: 'api_key', keyUrl: 'https://console.x.ai', credType: 'api_key' },
];

/**
 * Per-provider badge letter + color shown next to model names/messages.
 * Not the providers' actual logos (no bundled trademarked brand assets) —
 * just a stand-in monogram so different providers are visually distinct.
 */
// ── LLM Functions trigger types ──────────────────────────────────────────────

export interface TriggerResult {
  action: string;
  // WARN is a documented, expected non-failure (e.g. CREATE finding the
  // file already exists per the context.md idiom) — not an error.
  status: 'OK' | 'WARN' | 'FAIL';
  detail: string;
  file_path: string | null;
}

export interface TriggerParseError {
  raw: string;
  message: string;
  suggestion: string | null;
}

export interface PermissionLevel {
  label: string;
  description: string;
  allowed_triggers: string[];
}

export interface ProviderPermission {
  display_name: string;
  available_levels: string[];
  default_level: string;
  api_mapping: Record<string, unknown>;
  notes: string | null;
}

export interface PermissionConfig {
  version: number;
  levels: Record<string, PermissionLevel>;
  providers: Record<string, ProviderPermission>;
}

export interface UndoResult {
  action: string;
  file_path: string;
  restored: boolean;
  detail: string;
}

export const PROVIDER_BADGE: Record<string, { letter: string; color: string }> = {
  anthropic: { letter: 'A', color: '#d97757' },
  openai: { letter: 'O', color: '#10a37f' },
  'openai-codex': { letter: 'O', color: '#10a37f' },
  google: { letter: 'G', color: '#4285f4' },
  ollama: { letter: 'L', color: '#6b7280' },
  groq: { letter: 'G', color: '#f55036' },
  openrouter: { letter: 'R', color: '#8b5cf6' },
  mistral: { letter: 'M', color: '#ff7000' },
  together: { letter: 'T', color: '#0f6fff' },
  deepseek: { letter: 'D', color: '#4d6bfe' },
  xai: { letter: 'X', color: '#000000' },
};
