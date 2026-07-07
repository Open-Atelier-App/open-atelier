# API Reference

Open Atelier's API surface is Tauri commands plus Tauri events. The frontend wrapper lives in [src/lib/tauri.ts](../../src/lib/tauri.ts#L10). Commands are registered in [src-tauri/src/lib.rs](../../src-tauri/src/lib.rs#L77).

## Profile Commands

| Command | Frontend Wrapper | Backend |
|---|---|---|
| `profile_list` | `profileList()` | [profile.rs](../../src-tauri/src/commands/profile.rs#L19) |
| `profile_create` | `profileCreate(name, dirName, rootPath)` | [profile.rs](../../src-tauri/src/commands/profile.rs#L29) |
| `profile_update` | `profileUpdate(id, name?, dirName?, rootPath?)` | [profile.rs](../../src-tauri/src/commands/profile.rs#L50) |
| `profile_delete` | `profileDelete(id)` | [profile.rs](../../src-tauri/src/commands/profile.rs#L76) |
| `profile_switch` | `profileSwitch(id)` | [profile.rs](../../src-tauri/src/commands/profile.rs#L83) |
| `profile_get_active` | `profileGetActive()` | [profile.rs](../../src-tauri/src/commands/profile.rs#L110) |
| `profile_recreate_dir` | `profileRecreateDir(id)` | [profile.rs](../../src-tauri/src/commands/profile.rs#L97) |

## Workspace Commands

| Command | Frontend Wrapper | Backend |
|---|---|---|
| `workspace_open` | `workspaceOpen(path)` | [workspace.rs](../../src-tauri/src/commands/workspace.rs#L28) |
| `workspace_list` | `workspaceList(profileId)` | [workspace.rs](../../src-tauri/src/commands/workspace.rs#L75) |
| `workspace_close` | `workspaceClose(id)` | [workspace.rs](../../src-tauri/src/commands/workspace.rs#L86) |
| `workspace_rename` | `workspaceRename(id, name)` | [workspace.rs](../../src-tauri/src/commands/workspace.rs#L92) |
| `workspace_delete` | `workspaceDelete(id)` | [workspace.rs](../../src-tauri/src/commands/workspace.rs#L104) |
| `workspace_relocate` | `workspaceRelocate(id, newPath)` | [workspace.rs](../../src-tauri/src/commands/workspace.rs#L111) |

## File And Index Commands

| Command | Frontend Wrapper | Backend |
|---|---|---|
| `file_list_tree` | `fileListTree(workspaceId)` | [files.rs](../../src-tauri/src/commands/files.rs#L62) |
| `file_create` | `fileCreate(workspaceId, relPath, content)` | [files.rs](../../src-tauri/src/commands/files.rs#L72) |
| `file_rename` | `fileRename(workspaceId, oldRelPath, newRelPath)` | [files.rs](../../src-tauri/src/commands/files.rs#L106) |
| `file_delete` | `fileDelete(workspaceId, relPath)` | [files.rs](../../src-tauri/src/commands/files.rs#L139) |
| `file_read_raw` | `fileReadRaw(workspaceId, relPath)` | [files.rs](../../src-tauri/src/commands/files.rs#L159) |
| `index_start` | `indexStart(workspaceId)` | [files.rs](../../src-tauri/src/commands/files.rs#L175) |
| `index_cancel` | `indexCancel(workspaceId)` | [files.rs](../../src-tauri/src/commands/files.rs#L196) |
| `index_status` | `indexStatus(workspaceId)` | [files.rs](../../src-tauri/src/commands/files.rs#L206) |

## Chat, Search, And Tool Commands

| Command | Frontend Wrapper | Backend |
|---|---|---|
| `conversation_list` | `conversationList(workspaceId)` | [chat.rs](../../src-tauri/src/commands/chat.rs#L36) |
| `conversation_create` | `conversationCreate(workspaceId, title?)` | [chat.rs](../../src-tauri/src/commands/chat.rs#L47) |
| `conversation_rename` | `conversationRename(id, title)` | [chat.rs](../../src-tauri/src/commands/chat.rs#L64) |
| `conversation_delete` | `conversationDelete(id)` | [chat.rs](../../src-tauri/src/commands/chat.rs#L75) |
| `conversation_get` | `conversationGet(id)` | [chat.rs](../../src-tauri/src/commands/chat.rs#L82) |
| `ask` | `ask(conversationId, content, provider, model)` | [chat.rs](../../src-tauri/src/commands/chat.rs#L101) |
| `search_hybrid` | `searchHybrid(workspaceId, query, limit?)` | [chat.rs](../../src-tauri/src/commands/chat.rs#L327) |
| `tool_list` | `toolList(messageId)` | [chat.rs](../../src-tauri/src/commands/chat.rs#L269) |
| `tool_approve` | `toolApprove(toolCallId)` | [chat.rs](../../src-tauri/src/commands/chat.rs#L287) |
| `tool_reject` | `toolReject(toolCallId)` | [chat.rs](../../src-tauri/src/commands/chat.rs#L307) |

## Settings And Credential Commands

| Command | Frontend Wrapper | Backend |
|---|---|---|
| `key_save` | `keySave(provider, key)` | [settings.rs](../../src-tauri/src/commands/settings.rs#L48) |
| `key_delete` | `keyDelete(provider)` | [settings.rs](../../src-tauri/src/commands/settings.rs#L55) |
| `key_get` | `keyGet(provider)` | [settings.rs](../../src-tauri/src/commands/settings.rs#L60) |
| `key_test` | `keyTest(provider)` | [settings.rs](../../src-tauri/src/commands/settings.rs#L65) |
| `key_list_status` | `keyListStatus()` | [settings.rs](../../src-tauri/src/commands/settings.rs#L111) |
| `settings_get` | `settingsGet(key)` | [settings.rs](../../src-tauri/src/commands/settings.rs#L123) |
| `settings_set` | `settingsSet(key, value)` | [settings.rs](../../src-tauri/src/commands/settings.rs#L140) |
| `cred_save` | `credSave(provider, credType, value)` | [settings.rs](../../src-tauri/src/commands/settings.rs#L20) |
| `cred_delete` | `credDelete(provider, credType)` | [settings.rs](../../src-tauri/src/commands/settings.rs#L27) |
| `cred_get` | `credGet(provider, credType)` | [settings.rs](../../src-tauri/src/commands/settings.rs#L33) |
| `cred_get_with_backend` | `credGetWithBackend(provider, credType)` | [settings.rs](../../src-tauri/src/commands/settings.rs#L42) |

## Platform, CLI, And Skills Commands

| Command | Frontend Wrapper | Backend |
|---|---|---|
| `platform_info` | `platformInfo()` | [window.rs](../../src-tauri/src/commands/window.rs#L6) |
| `detect_cli_credentials` | `detectCliCredentials()` | [cli_detect.rs](../../src-tauri/src/commands/cli_detect.rs#L28) |
| `anthropic_oauth_status` | `anthropicOauthStatus()` | [anthropic_oauth.rs](../../src-tauri/src/llm/anthropic_oauth.rs#L136) |
| `check_macos_keychain_session` | `checkMacosKeychainSession()` | [anthropic_oauth.rs](../../src-tauri/src/llm/anthropic_oauth.rs#L109) |
| `skill_list` | `skillList(profileRoot)` | [skills.rs](../../src-tauri/src/llm/skills.rs#L55) |

## Events

| Event | Producer | Consumer |
|---|---|---|
| `chat://token` | [router.rs](../../src-tauri/src/llm/router.rs#L97) | [useTauriEvents.ts](../../src/hooks/useTauriEvents.ts#L21) |
| `chat://done` | [chat.rs](../../src-tauri/src/commands/chat.rs#L210) | [useTauriEvents.ts](../../src/hooks/useTauriEvents.ts#L25) |
| `chat://error` | [chat.rs](../../src-tauri/src/commands/chat.rs#L250) | [useTauriEvents.ts](../../src/hooks/useTauriEvents.ts#L29) |
| `index://progress` | [indexer/mod.rs](../../src-tauri/src/indexer/mod.rs#L100) | [useTauriEvents.ts](../../src/hooks/useTauriEvents.ts#L45) |
| `index://complete` | [indexer/mod.rs](../../src-tauri/src/indexer/mod.rs#L87) | [useTauriEvents.ts](../../src/hooks/useTauriEvents.ts#L49) |
| `conversation://titled` | [chat.rs](../../src-tauri/src/commands/chat.rs#L235) | [useTauriEvents.ts](../../src/hooks/useTauriEvents.ts#L53) |
| `menu://preferences` | [lib.rs](../../src-tauri/src/lib.rs#L69) | [useTauriEvents.ts](../../src/hooks/useTauriEvents.ts#L57) |
