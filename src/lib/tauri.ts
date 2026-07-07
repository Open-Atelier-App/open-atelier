import { invoke } from '@tauri-apps/api/core';
import type {
  Profile, Workspace, WorkspaceFile, FileNode,
  Conversation, ConversationGroup, Message, Citation, ToolCall, KeyStatus,
  PermissionConfig, UndoResult, OfficePreview, PlanWithTasks, PlanTask,
} from './types';

/**
 * A rejected Tauri command surfaces its Rust `Err` value as a plain JSON
 * object (`{ code, message, detail }`, from AtelierError's Serialize impl)
 * — not an Error instance and not a string. `String(e)` on a plain object
 * gives the useless "[object Object]" instead of the actual message. Use
 * this wherever a caught error is shown to the user.
 */
export function errorMessage(e: unknown): string {
  if (e && typeof e === 'object' && 'message' in e) {
    const message = (e as { message: unknown }).message;
    if (typeof message === 'string' && message) return message;
  }
  return String(e);
}

// ── Profile ──────────────────────────────────────────────────────────────────
// NOTE: Tauri 2 expects camelCase keys from JS; Rust receives them as snake_case.

export const profileList = () =>
  invoke<Profile[]>('profile_list');

export const profileCreate = (name: string, dirName: string, rootPath: string) =>
  invoke<Profile>('profile_create', { name, dirName, rootPath });

export const profileUpdate = (id: number, name?: string, dirName?: string, rootPath?: string) =>
  invoke<Profile>('profile_update', { id, name, dirName, rootPath });

export const profileDelete = (id: number) =>
  invoke<void>('profile_delete', { id });

export const profileSwitch = (id: number) =>
  invoke<Profile>('profile_switch', { id });

export const profileGetActive = () =>
  invoke<Profile | null>('profile_get_active');

export const profileRecreateDir = (id: number) =>
  invoke<Profile>('profile_recreate_dir', { id });

// ── Workspace ─────────────────────────────────────────────────────────────────

export const workspaceOpen = (path: string, parentWorkspaceId?: number | null) =>
  invoke<Workspace>('workspace_open', { path, parentWorkspaceId: parentWorkspaceId ?? null });

export const workspaceClose = (id: number) =>
  invoke<void>('workspace_close', { id });

export const workspaceList = (profileId: number) =>
  invoke<Workspace[]>('workspace_list', { profileId });

/** Sets or clears (pass null) a project's parent, making/unmaking it a sub-project. */
export const workspaceSetParent = (id: number, parentWorkspaceId: number | null) =>
  invoke<Workspace>('workspace_set_parent', { id, parentWorkspaceId });

/** Sets or clears (pass an empty string) the project's one-sentence description. */
export const workspaceSetDescription = (id: number, description: string) =>
  invoke<Workspace>('workspace_set_description', { id, description });

export const workspaceRename = (id: number, name: string) =>
  invoke<Workspace>('workspace_rename', { id, name });

export const workspaceDelete = (id: number) =>
  invoke<void>('workspace_delete', { id });

export const workspaceRelocate = (id: number, newPath: string) =>
  invoke<Workspace>('workspace_relocate', { id, newPath });

/** Suggests a project name from its file listing, using whichever provider/model the workspace last actually chatted with. */
export const workspaceSuggestName = (id: number) =>
  invoke<string>('workspace_suggest_name', { id });

// ── Files ────────────────────────────────────────────────────────────────────

export const fileListTree = (workspaceId: number) =>
  invoke<FileNode[]>('file_list_tree', { workspaceId });

export const fileCreate = (workspaceId: number, relPath: string, content: string) =>
  invoke<WorkspaceFile>('file_create', { workspaceId, relPath, content });

export const fileRename = (workspaceId: number, oldRelPath: string, newRelPath: string) =>
  invoke<WorkspaceFile>('file_rename', { workspaceId, oldRelPath, newRelPath });

export const fileDelete = (workspaceId: number, relPath: string) =>
  invoke<void>('file_delete', { workspaceId, relPath });

export const fileWrite = (workspaceId: number, relPath: string, content: string) =>
  invoke<WorkspaceFile>('file_write', { workspaceId, relPath, content });

export const fileReadRaw = (workspaceId: number, relPath: string) =>
  invoke<string>('file_read_raw', { workspaceId, relPath });

export const fileReadOfficePreview = (workspaceId: number, relPath: string) =>
  invoke<OfficePreview>('file_read_office_preview', { workspaceId, relPath });

export const fileExportPdf = (workspaceId: number, relPath: string) =>
  invoke<string>('file_export_pdf', { workspaceId, relPath });

// ── Index ────────────────────────────────────────────────────────────────────

export const indexStart = (workspaceId: number) =>
  invoke<void>('index_start', { workspaceId });

export const indexCancel = (workspaceId: number) =>
  invoke<void>('index_cancel', { workspaceId });

export const indexStatus = (workspaceId: number) =>
  invoke<{ done: number; total: number; status: string }>('index_status', { workspaceId });

// ── Search ───────────────────────────────────────────────────────────────────

export const searchHybrid = (workspaceId: number, query: string, limit?: number) =>
  invoke<Citation[]>('search_hybrid', { workspaceId, query, limit: limit ?? 8 });

// ── Chat ─────────────────────────────────────────────────────────────────────

export const conversationList = (workspaceId: number) =>
  invoke<Conversation[]>('conversation_list', { workspaceId });

export const conversationCreate = (workspaceId: number, title?: string) =>
  invoke<Conversation>('conversation_create', { workspaceId, title });

export const conversationRename = (id: number, title: string) =>
  invoke<Conversation>('conversation_rename', { id, title });

export const conversationDelete = (id: number) =>
  invoke<void>('conversation_delete', { id });

export const conversationGet = (id: number) =>
  invoke<{ conversation: Conversation; messages: Message[] }>('conversation_get', { id });

/** Compresses the conversation into a memory block so it can continue in the same thread, optionally with a different provider/model. */
export const conversationCompress = (id: number, provider: string, model: string) =>
  invoke<Conversation>('conversation_compress', { id, provider, model });

/** Creates a new conversation containing a copy of every message up to and including upToMessageId. */
export const conversationFork = (id: number, upToMessageId: number) =>
  invoke<Conversation>('conversation_fork', { id, upToMessageId });

// ── Conversation groups (folders) ───────────────────────────────────────────

export const conversationGroupList = (workspaceId: number) =>
  invoke<ConversationGroup[]>('conversation_group_list', { workspaceId });

export const conversationGroupCreate = (workspaceId: number, name: string) =>
  invoke<ConversationGroup>('conversation_group_create', { workspaceId, name });

export const conversationGroupRename = (id: number, name: string) =>
  invoke<ConversationGroup>('conversation_group_rename', { id, name });

export const conversationGroupDelete = (id: number) =>
  invoke<void>('conversation_group_delete', { id });

/** Rewrites every group's position in a workspace to match orderedIds' order. */
export const conversationGroupReorder = (workspaceId: number, orderedIds: number[]) =>
  invoke<ConversationGroup[]>('conversation_group_reorder', { workspaceId, orderedIds });

/** Files (or, with groupId null, unfiles) a conversation under a folder. */
export const conversationSetGroup = (conversationId: number, groupId: number | null) =>
  invoke<Conversation>('conversation_set_group', { conversationId, groupId });

export const ask = (
  conversationId: number,
  content: string,
  provider: string,
  model: string,
) =>
  invoke<Message>('ask', { conversationId, content, provider, model });

// ── Tools ────────────────────────────────────────────────────────────────────

export const toolApprove = (toolCallId: number) =>
  invoke<ToolCall>('tool_approve', { toolCallId });

export const toolReject = (toolCallId: number) =>
  invoke<ToolCall>('tool_reject', { toolCallId });

export const toolList = (messageId: number) =>
  invoke<ToolCall[]>('tool_list', { messageId });

// ── Settings ─────────────────────────────────────────────────────────────────

export const keySave = (provider: string, key: string) =>
  invoke<void>('key_save', { provider, key });

export const keyDelete = (provider: string) =>
  invoke<void>('key_delete', { provider });

export const keyGet = (provider: string) =>
  invoke<string | null>('key_get', { provider });

export const keyTest = (provider: string) =>
  invoke<boolean>('key_test', { provider });

export const keyTestWithValue = (provider: string, value: string) =>
  invoke<boolean>('key_test_with_value', { provider, value });

export const keyTestProfile = (provider: string, profileId: number) =>
  invoke<boolean>('key_test_profile', { provider, profileId });

export const keyListStatus = () =>
  invoke<KeyStatus[]>('key_list_status');

export const settingsGet = (key: string) =>
  invoke<unknown>('settings_get', { key });

export const settingsSet = (key: string, value: unknown) =>
  invoke<void>('settings_set', { key, value });

export const factoryReset = () =>
  invoke<void>('factory_reset');

// ── Credentials (non-API-key auth: OAuth tokens, Azure endpoint+key, etc.) ──

export interface CredentialEntry {
  provider: string;
  credType: 'api_key' | 'bearer_token' | 'azure' | 'ollama_url';
  label: string;
}

export const credSave = (provider: string, credType: string, value: string) =>
  invoke<void>('cred_save', { provider, credType, value });

export const credDelete = (provider: string, credType: string) =>
  invoke<void>('cred_delete', { provider, credType });

export const credGet = (provider: string, credType: string) =>
  invoke<string | null>('cred_get', { provider, credType });

export const credGetWithBackend = (provider: string, credType: string) =>
  invoke<[string, string] | null>('cred_get_with_backend', { provider, credType });

export const credGetMasked = (provider: string, credType: string, profileId?: number) =>
  invoke<string | null>('cred_get_masked', { provider, credType, profileId: profileId ?? null });

// ── Per-profile credential commands ──────────────────────────────────────

export const credSaveProfile = (provider: string, credType: string, value: string, profileId: number) =>
  invoke<void>('cred_save_profile', { provider, credType, value, profileId });

export const credDeleteProfile = (provider: string, credType: string, profileId: number) =>
  invoke<void>('cred_delete_profile', { provider, credType, profileId });

export const credGetProfile = (provider: string, credType: string, profileId: number) =>
  invoke<string | null>('cred_get_profile', { provider, credType, profileId });

export const credGetWithBackendProfile = (provider: string, credType: string, profileId: number) =>
  invoke<[string, string] | null>('cred_get_with_backend_profile', { provider, credType, profileId });

// ── Connectors ────────────────────────────────────────────────────────────

/** Validates a GitHub personal access token, returning the authenticated username. */
export const connectorGithubTest = (token: string) =>
  invoke<string>('connector_github_test', { token });

/** Validates a Notion integration token, returning the integration's own name. */
export const connectorNotionTest = (token: string) =>
  invoke<string>('connector_notion_test', { token });

/** Validates a Slack bot token, returning the workspace (team) name. */
export const connectorSlackTest = (token: string) =>
  invoke<string>('connector_slack_test', { token });

/** Validates a Google Drive API key. There's no user identity behind a bare
 *  key, so this only confirms the key itself is accepted by the Drive API. */
export const connectorGoogleDriveTest = (apiKey: string) =>
  invoke<string>('connector_google_drive_test', { apiKey });

// ── Native OAuth ("Connect" button) flows ───────────────────────────────────

export interface DeviceFlowStart {
  user_code: string;
  verification_uri: string;
  device_code: string;
  expires_in: number;
  interval: number;
}

/** Requests a GitHub device code and opens the browser to its verification page. */
export const connectorGithubOauthStart = () =>
  invoke<DeviceFlowStart>('connector_github_oauth_start');

/** Waits (blocks) until the device code from connectorGithubOauthStart is approved, then stores the token and returns the username. */
export const connectorGithubOauthFinish = (deviceCode: string, interval: number, expiresIn: number, profileId: number) =>
  invoke<string>('connector_github_oauth_finish', { deviceCode, interval, expiresIn, profileId });

/** Runs the full Google Drive OAuth flow (opens the browser, waits for the redirect) and returns the connected account's email. */
export const connectorGoogleDriveOauthConnect = (profileId: number) =>
  invoke<string>('connector_google_drive_oauth_connect', { profileId });

export const keyListStatusProfile = (profileId: number) =>
  invoke<KeyStatus[]>('key_list_status_profile', { profileId });

export const credMigrateToProfile = (profileId: number) =>
  invoke<void>('cred_migrate_to_profile', { profileId });

// ── Window / platform ────────────────────────────────────────────────────────

export const windowNew = (profileId: number) =>
  invoke<void>('window_new', { profileId });

export const platformInfo = () =>
  invoke<{ os: string; arch: string; apple_fm_available: boolean }>('platform_info');

export const openPath = (path: string) =>
  invoke<void>('open_path', { path });

export const diagnosticReport = () =>
  invoke<string>('diagnostic_report');

// ── Atelier skills ────────────────────────────────────────────────────────

export interface SkillInfo {
  name: string;
  preview: string;
}

/** Lists the Markdown skill files found in `<profile_root>/skills/`. */
export const skillList = (profile_root: string) =>
  invoke<SkillInfo[]>('skill_list', { profileRoot: profile_root });

/** Lists ready-made skills bundled with the app (see Settings > Skills). */
export const defaultSkillList = () =>
  invoke<SkillInfo[]>('default_skill_list');

/** Copies a bundled default skill into `<profile_root>/skills/`, activating it immediately. */
export const defaultSkillInstall = (profile_root: string, name: string) =>
  invoke<void>('default_skill_install', { profileRoot: profile_root, name });

// ── LLM Functions: permissions ──────────────────────────────────────────────

export const getPermissionConfig = () =>
  invoke<PermissionConfig>('get_permission_config');

export const getPermissionLevel = (provider: string) =>
  invoke<string>('get_permission_level', { provider });

export const setPermissionLevel = (provider: string, level: string) =>
  invoke<string[]>('set_permission_level', { provider, level });

// ── LLM Functions: undo ─────────────────────────────────────────────────────

export const undoTrigger = (conversationId: number) =>
  invoke<UndoResult>('undo_trigger', { conversationId });

// ── Plans ────────────────────────────────────────────────────────────────────

export const planList = (conversationId: number) =>
  invoke<PlanWithTasks[]>('plan_list', { conversationId });

export const planExecuteNext = (planId: number) =>
  invoke<PlanTask | null>('plan_execute_next', { planId });
