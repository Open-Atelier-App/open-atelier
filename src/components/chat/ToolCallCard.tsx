import { Wrench, Check, X } from 'lucide-react';
import { useChatStore } from '../../stores/chatStore';
import type { ToolCall } from '../../lib/types';

interface Props {
  toolCall: ToolCall;
}

// Keys match the trigger action names emitted by the backend's
// >>>[ACTION]<<< protocol (src-tauri/src/triggers/parser.rs), not the
// tool_name strings from a traditional function-calling API.
const TOOL_LABELS: Record<string, string> = {
  CREATE: 'Create file',
  DELETE: 'Delete file',
  WRITE: 'Write file',
  INSERT: 'Insert into file',
  APPEND: 'Append to file',
  PREVIEW: 'Preview file',
  READ: 'Read file',
  RENAME: 'Rename file',
  LIST: 'List files',
};

export function ToolCallCard({ toolCall }: Props) {
  const approveToolCall = useChatStore(s => s.approveToolCall);
  const rejectToolCall = useChatStore(s => s.rejectToolCall);

  let args: Record<string, unknown> = {};
  try {
    args = JSON.parse(toolCall.arguments_json);
  } catch {
    // ignore
  }

  const isPending = toolCall.status === 'pending';
  const isApproved = toolCall.status === 'approved' || toolCall.status === 'executed';
  const isRejected = toolCall.status === 'rejected';

  const pathArg = (args.rel_path ?? args.old_rel_path ?? args.query ?? '') as string;

  return (
    <div style={{
      margin: '6px 24px 6px 40px',
      background: 'var(--bg-surface)', border: '1px solid var(--border)',
      borderRadius: 6, padding: '10px 12px',
      display: 'flex', alignItems: 'center', gap: 10,
      opacity: isRejected ? 0.5 : 1,
    }}>
      <Wrench size={14} color="var(--accent)" style={{ flexShrink: 0 }} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--text-primary)' }}>
          {TOOL_LABELS[toolCall.tool_name] ?? toolCall.tool_name}
        </div>
        {pathArg && (
          <div style={{
            fontSize: 11, color: 'var(--text-muted)',
            fontFamily: 'JetBrains Mono, monospace',
            overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
          }}>
            {pathArg}
          </div>
        )}
        {typeof args.content === 'string' && args.content && (
          <div style={{
            fontSize: 11, color: 'var(--text-muted)', marginTop: 4,
            background: 'var(--overlay)', padding: '4px 6px', borderRadius: 3,
            fontFamily: 'JetBrains Mono, monospace',
            maxHeight: 60, overflow: 'hidden',
          }}>
            {(args.content as string).slice(0, 200)}
          </div>
        )}
      </div>

      {isPending && (
        <div style={{ display: 'flex', gap: 6, flexShrink: 0 }}>
          <button
            onClick={() => approveToolCall(toolCall.id)}
            style={{
              padding: '4px 10px', background: 'var(--success)', border: 'none',
              borderRadius: 4, color: '#fff', fontSize: 12, cursor: 'pointer',
              display: 'flex', alignItems: 'center', gap: 4,
            }}
          >
            <Check size={11} />
            Approve
          </button>
          <button
            onClick={() => rejectToolCall(toolCall.id)}
            style={{
              padding: '4px 10px', background: 'none', border: '1px solid var(--border)',
              borderRadius: 4, color: 'var(--text-muted)', fontSize: 12, cursor: 'pointer',
              display: 'flex', alignItems: 'center', gap: 4,
            }}
          >
            <X size={11} />
            Reject
          </button>
        </div>
      )}
      {isApproved && (
        <span style={{ fontSize: 11, color: 'var(--success)', flexShrink: 0 }}>✓ Approved</span>
      )}
      {isRejected && (
        <span style={{ fontSize: 11, color: 'var(--error)', flexShrink: 0 }}>✕ Rejected</span>
      )}
    </div>
  );
}
