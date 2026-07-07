import { useState } from 'react';
import { ChevronDown, ChevronRight, Circle, CircleCheck, CircleX, ListTodo, Loader2, Play, Square } from 'lucide-react';
import type { PlanWithTasks, PlanTask } from '../../lib/types';
import { usePlanStore } from '../../stores/planStore';

function TaskRow({ task }: { task: PlanTask }) {
  const icon = {
    pending: <Circle size={14} color="var(--text-muted)" />,
    running: <Loader2 size={14} color="var(--accent)" style={{ animation: 'spin 1s linear infinite' }} />,
    done: <CircleCheck size={14} color="var(--success)" />,
    failed: <CircleX size={14} color="var(--error)" />,
  }[task.status];

  return (
    <div style={{ display: 'flex', gap: 8, padding: '5px 0', alignItems: 'flex-start' }}>
      <div style={{ marginTop: 2, flexShrink: 0 }}>{icon}</div>
      <div style={{ minWidth: 0, flex: 1 }}>
        <div style={{
          fontSize: 13,
          color: task.status === 'pending' ? 'var(--text-muted)' : 'var(--text-primary)',
          textDecoration: task.status === 'done' ? 'line-through' : 'none',
          opacity: task.status === 'done' ? 0.75 : 1,
        }}>
          {task.description}
        </div>
        {task.summary && (
          <div style={{ fontSize: 12, color: 'var(--text-muted)', marginTop: 2 }}>
            {task.summary}
          </div>
        )}
      </div>
    </div>
  );
}

function PlanCard({ entry }: { entry: PlanWithTasks }) {
  const runNext = usePlanStore(s => s.runNext);
  const runAll = usePlanStore(s => s.runAll);
  const stopAutoRun = usePlanStore(s => s.stopAutoRun);
  const autoRunning = usePlanStore(s => s.autoRunning.has(entry.plan.id));
  const [collapsed, setCollapsed] = useState(false);

  const hasPending = entry.tasks.some(t => t.status === 'pending');
  const isBusy = entry.plan.status === 'running';
  const doneCount = entry.tasks.filter(t => t.status === 'done').length;
  const statusLabel = {
    pending: 'Not started',
    running: 'Running',
    done: 'Done',
    failed: 'Stopped — a step failed',
  }[entry.plan.status];
  const statusColor = {
    pending: 'var(--text-muted)',
    running: 'var(--accent)',
    done: 'var(--success)',
    failed: 'var(--error)',
  }[entry.plan.status];

  return (
    <div style={{
      border: '1px solid var(--border)', borderRadius: 6, padding: '12px 14px',
      margin: '0 12px 12px', background: 'var(--bg-surface)',
    }}>
      <button
        onClick={() => setCollapsed(v => !v)}
        style={{
          display: 'flex', alignItems: 'center', gap: 8, marginBottom: collapsed ? 0 : 8,
          width: '100%', background: 'none', border: 'none', cursor: 'pointer', padding: 0, font: 'inherit',
          textAlign: 'left',
        }}
      >
        {collapsed ? <ChevronRight size={13} color="var(--text-muted)" /> : <ChevronDown size={13} color="var(--text-muted)" />}
        <ListTodo size={14} color="var(--text-muted)" />
        <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-primary)', flex: 1 }}>
          {entry.plan.title}
        </span>
        {collapsed && (
          <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{doneCount}/{entry.tasks.length}</span>
        )}
        <span style={{ fontSize: 11, color: statusColor }}>{statusLabel}</span>
      </button>

      {!collapsed && (
        <div style={{ maxHeight: '32vh', overflowY: 'auto' }}>
          {entry.tasks.map(task => <TaskRow key={task.id} task={task} />)}
        </div>
      )}

      {!collapsed && hasPending && (
        <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
          <button
            onClick={() => runNext(entry.plan.id)}
            disabled={isBusy}
            style={{
              display: 'flex', alignItems: 'center', gap: 4,
              padding: '4px 10px', borderRadius: 4, fontSize: 12, border: 'none',
              background: 'var(--overlay)', color: isBusy ? 'var(--text-muted)' : 'var(--text-primary)',
              cursor: isBusy ? 'default' : 'pointer',
            }}
          >
            <Play size={12} /> Run next step
          </button>
          <button
            onClick={() => autoRunning ? stopAutoRun(entry.plan.id) : runAll(entry.plan.id)}
            style={{
              display: 'flex', alignItems: 'center', gap: 4,
              padding: '4px 10px', borderRadius: 4, fontSize: 12, border: 'none',
              background: autoRunning ? 'var(--error)' : 'var(--accent)', color: '#fff', cursor: 'pointer',
            }}
          >
            {autoRunning ? <><Square size={12} /> Stop</> : <><Play size={12} /> Run all</>}
          </button>
        </div>
      )}
    </div>
  );
}

export function PlanPanel() {
  const plans = usePlanStore(s => s.plans);

  return (
    // Bounded and independently scrollable so a plan with many steps (or
    // several plans at once) can never squeeze whatever's above it (the
    // files panel) down to nothing — each PlanCard also caps its own task
    // list height.
    <div style={{ flexShrink: 0, paddingTop: 12, borderTop: '1px solid var(--border)', maxHeight: '45vh', overflowY: 'auto' }}>
      {plans.length === 0 ? (
        <div style={{
          margin: '0 12px 12px', padding: '10px 12px', borderRadius: 6,
          border: '1px dashed var(--border)', display: 'flex', alignItems: 'center', gap: 8,
          color: 'var(--text-muted)', fontSize: 12,
        }}>
          <ListTodo size={14} style={{ flexShrink: 0 }} />
          Ask me to plan something to start plan mode.
        </div>
      ) : (
        plans.map(entry => <PlanCard key={entry.plan.id} entry={entry} />)
      )}
    </div>
  );
}
