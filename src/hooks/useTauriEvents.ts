import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { message as messageDialog } from '@tauri-apps/plugin-dialog';
import type { IndexProgress, ChatToken, ChatDone, ChatContinuation, ChatError, ToolProposed, TriggerResult, TriggerParseError, Plan, PlanTask, PlanWithTasks } from '../lib/types';
import { useChatStore } from '../stores/chatStore';
import { useWorkspaceStore } from '../stores/workspaceStore';
import { useUIStore } from '../stores/uiStore';
import { usePermissionStore } from '../stores/permissionStore';
import { usePlanStore } from '../stores/planStore';
import * as api from '../lib/tauri';

const FS_MUTATING_ACTIONS = new Set([
  'CREATE', 'DELETE', 'WRITE', 'RENAME', 'INSERT', 'APPEND',
  'CREATE_DOCX', 'CREATE_XLSX', 'CREATE_PPTX', 'CREATE_MP3', 'EXPORT_PDF',
]);

export function useTauriEvents() {
  const appendToken = useChatStore(s => s.appendToken);
  const completeMessage = useChatStore(s => s.completeMessage);
  const addMessage = useChatStore(s => s.addMessage);
  const errorMessage = useChatStore(s => s.errorMessage);
  const addToolCall = useChatStore(s => s.addToolCall);
  const updateConversationTitle = useChatStore(s => s.updateConversationTitle);
  const updateConversationSummary = useChatStore(s => s.updateConversationSummary);
  const updateIndexProgress = useWorkspaceStore(s => s.updateIndexProgress);
  const updateWorkspaceStatus = useWorkspaceStore(s => s.updateWorkspaceStatus);
  const updateWorkspaceDescription = useWorkspaceStore(s => s.updateWorkspaceDescription);
  const setShowSettings = useUIStore(s => s.setShowSettings);
  const addTriggerResult = usePermissionStore(s => s.addTriggerResult);
  const addTriggerError = usePermissionStore(s => s.addTriggerError);
  const clearTriggerFeedback = usePermissionStore(s => s.clearTriggerFeedback);

  useEffect(() => {
    // listen() is async (it's an IPC round-trip), but effect cleanup runs
    // synchronously. Under React.StrictMode's dev-only double-invoke (mount
    // -> cleanup -> mount), the first mount's listen() calls resolve *after*
    // its cleanup already ran, so naively pushing into a shared array and
    // unlistening on cleanup misses them entirely — they're never
    // unregistered, and the second mount registers a fresh set on top of
    // them. Every backend event then fires twice (garbled streamed text,
    // doubled trigger-result entries, etc.), permanently, not just in dev.
    // `active` guards against this: if a registration resolves after this
    // effect instance was already cleaned up, undo it immediately instead
    // of leaving it dangling.
    let active = true;
    const unlisten: Array<() => void> = [];

    function subscribe<T>(event: string, handler: (payload: T) => void) {
      listen<T>(event, e => handler(e.payload)).then(un => {
        if (active) {
          unlisten.push(un);
        } else {
          un();
        }
      }).catch(() => {});
    }

    subscribe<ChatToken>('chat://token', payload => {
      appendToken(payload.message_id, payload.delta);
    });

    subscribe<ChatDone>('chat://done', payload => {
      completeMessage(payload.message_id, payload.citations, payload.has_more, payload.display_override);
    });

    subscribe<ChatContinuation>('chat://continuation', payload => {
      // A READ/LIST result required the model to keep going without new
      // user input — the backend already created this message and is
      // about to stream into it. Only show it if we're still looking at
      // the conversation it belongs to.
      const active = useChatStore.getState().activeConversation;
      if (active?.id === payload.message.conversation_id) {
        addMessage(payload.message);
      }
    });

    subscribe<ChatError>('chat://error', payload => {
      errorMessage(payload.message_id, payload.error.message);
    });

    subscribe<ToolProposed>('tool://proposed', payload => {
      addToolCall({
        id: payload.tool_call_id,
        message_id: 0,
        tool_name: payload.tool_name,
        arguments_json: JSON.stringify(payload.arguments),
        status: 'pending',
        result_json: null,
        created_at: Date.now(),
      });
    });

    subscribe<IndexProgress>('index://progress', payload => {
      updateIndexProgress(payload);
    });

    subscribe<{ workspace_id: number }>('index://complete', payload => {
      updateWorkspaceStatus(payload.workspace_id, 'complete');
    });

    subscribe<{ id: number; title: string }>('conversation://titled', payload => {
      updateConversationTitle(payload.id, payload.title);
    });

    subscribe<{ id: number; summary: string }>('conversation://summarized', payload => {
      updateConversationSummary(payload.id, payload.summary);
    });

    subscribe<{ workspace_id: number; description: string }>('workspace://described', payload => {
      updateWorkspaceDescription(payload.workspace_id, payload.description);
    });

    subscribe<void>('menu://preferences', () => {
      setShowSettings(true);
    });

    subscribe<void>('menu://check_updates', () => {
      setShowSettings(true);
      useUIStore.getState().setSettingsTab('About');
    });

    subscribe<void>('menu://copy_logs', () => {
      api.diagnosticReport()
        .then(report => navigator.clipboard.writeText(report))
        .then(() => messageDialog('Diagnostic report copied to clipboard.', { title: 'Copy Diagnostic Report', kind: 'info' }))
        .catch((e: unknown) => messageDialog(api.errorMessage(e), { title: 'Could not copy diagnostic report', kind: 'error' }));
    });

    subscribe<TriggerResult>('trigger://executed', payload => {
      addTriggerResult(payload);
      // Keep the sidebar file tree in sync with triggers that touch disk —
      // previously the only way to see a newly created/renamed/deleted file
      // was to leave and reopen the conversation.
      if (payload.status === 'OK' && FS_MUTATING_ACTIONS.has(payload.action)) {
        const workspace = useWorkspaceStore.getState().active;
        if (workspace) {
          useWorkspaceStore.getState().loadFileTree(workspace.id);
        }
      }
    });

    subscribe<TriggerParseError>('trigger://error', payload => {
      addTriggerError(payload);
    });

    subscribe<PlanWithTasks>('plan://created', payload => {
      const active = useChatStore.getState().activeConversation;
      if (active?.id === payload.plan.conversation_id) {
        usePlanStore.setState(s => ({ plans: [...s.plans, payload] }));
      }
    });

    subscribe<Plan>('plan://updated', payload => {
      usePlanStore.getState().upsertPlan(payload);
    });

    subscribe<PlanTask>('plan://task_updated', payload => {
      usePlanStore.getState().upsertTask(payload);
    });

    return () => {
      active = false;
      unlisten.forEach(u => u());
    };
  }, [appendToken, completeMessage, addMessage, errorMessage, addToolCall, updateConversationTitle, updateConversationSummary, updateIndexProgress, updateWorkspaceStatus, updateWorkspaceDescription, setShowSettings, addTriggerResult, addTriggerError, clearTriggerFeedback]);
}
