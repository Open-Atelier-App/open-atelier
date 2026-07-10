import { create } from 'zustand';

export type WidgetKind = 'timer' | 'countdown';

export interface ConversationWidget {
  id: number;
  kind: WidgetKind;
}

interface WidgetState {
  // Ephemeral, per-conversation scratch widgets (stopwatch/countdown) — not
  // messages, not persisted, not sent to the LLM. Lost on reload, same as
  // the report's own "native JS" framing implies.
  widgets: Record<number, ConversationWidget[]>;
  addWidget: (conversationId: number, kind: WidgetKind) => void;
  removeWidget: (conversationId: number, id: number) => void;
}

let nextWidgetId = 1;

export const useWidgetStore = create<WidgetState>((set) => ({
  widgets: {},

  addWidget: (conversationId, kind) => set(s => ({
    widgets: {
      ...s.widgets,
      [conversationId]: [...(s.widgets[conversationId] ?? []), { id: nextWidgetId++, kind }],
    },
  })),

  removeWidget: (conversationId, id) => set(s => ({
    widgets: {
      ...s.widgets,
      [conversationId]: (s.widgets[conversationId] ?? []).filter(w => w.id !== id),
    },
  })),
}));
