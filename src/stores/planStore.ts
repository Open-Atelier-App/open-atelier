import { create } from 'zustand';
import type { PlanWithTasks, Plan, PlanTask } from '../lib/types';
import * as api from '../lib/tauri';

interface PlanState {
  plans: PlanWithTasks[];
  // Plan ids currently auto-running (calling planExecuteNext in a loop
  // until done/failed) — tracked so the "Run all" button can show a
  // stop control and so a second click doesn't start a duplicate loop.
  autoRunning: Set<number>;

  loadForConversation: (conversationId: number) => Promise<void>;
  clear: () => void;
  upsertPlan: (plan: Plan) => void;
  upsertTask: (task: PlanTask) => void;
  runNext: (planId: number) => Promise<void>;
  runAll: (planId: number) => Promise<void>;
  stopAutoRun: (planId: number) => void;
}

export const usePlanStore = create<PlanState>((set, get) => ({
  plans: [],
  autoRunning: new Set(),

  loadForConversation: async (conversationId) => {
    try {
      const plans = await api.planList(conversationId);
      set({ plans });
    } catch {
      set({ plans: [] });
    }
  },

  clear: () => {
    set({ plans: [], autoRunning: new Set() });
  },

  upsertPlan: (plan) => {
    set(s => {
      const idx = s.plans.findIndex(p => p.plan.id === plan.id);
      if (idx === -1) {
        // A brand-new plan (from a PLAN trigger) arrives via plan://created
        // with its tasks already attached — plan://updated alone (status-only)
        // for a plan we don't know about yet has nothing useful to show.
        return s;
      }
      const plans = [...s.plans];
      plans[idx] = { ...plans[idx], plan };
      return { plans };
    });
  },

  upsertTask: (task) => {
    set(s => ({
      plans: s.plans.map(p => {
        if (p.plan.id !== task.plan_id) return p;
        const tasks = p.tasks.some(t => t.id === task.id)
          ? p.tasks.map(t => t.id === task.id ? task : t)
          : [...p.tasks, task].sort((a, b) => a.seq - b.seq);
        return { ...p, tasks };
      }),
    }));
  },

  runNext: async (planId) => {
    await api.planExecuteNext(planId);
  },

  runAll: async (planId) => {
    if (get().autoRunning.has(planId)) return;
    set(s => ({ autoRunning: new Set(s.autoRunning).add(planId) }));

    try {
      while (true) {
        if (!get().autoRunning.has(planId)) break; // stopped by the user
        const entry = get().plans.find(p => p.plan.id === planId);
        if (!entry || entry.plan.status === 'done' || entry.plan.status === 'failed') break;

        const started = await api.planExecuteNext(planId);
        if (!started) break; // no pending task was found

        // Wait for this task to settle (its status stops being 'running')
        // via the plan://task_updated events landing in the store, rather
        // than polling — check the store on a short interval since we have
        // no promise to await for "the backend turn finished".
        await waitForTaskSettled(get, planId, started.id);
      }
    } finally {
      set(s => {
        const next = new Set(s.autoRunning);
        next.delete(planId);
        return { autoRunning: next };
      });
    }
  },

  stopAutoRun: (planId) => {
    set(s => {
      const next = new Set(s.autoRunning);
      next.delete(planId);
      return { autoRunning: next };
    });
  },
}));

function waitForTaskSettled(get: () => PlanState, planId: number, taskId: number): Promise<void> {
  return new Promise((resolve) => {
    const check = () => {
      const entry = get().plans.find(p => p.plan.id === planId);
      const task = entry?.tasks.find(t => t.id === taskId);
      if (!task || task.status !== 'running') {
        resolve();
        return;
      }
      setTimeout(check, 300);
    };
    check();
  });
}
