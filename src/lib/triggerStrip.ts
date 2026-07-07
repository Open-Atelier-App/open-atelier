const START = '>>>[';
const END = ']<<<';

// Mirrors KNOWN_ACTIONS in src-tauri/src/triggers/parser.rs.
const KNOWN_ACTIONS = [
  'MESSAGE', 'CREATE', 'DELETE', 'WRITE', 'INSERT',
  'APPEND', 'PREVIEW', 'READ', 'RENAME', 'LIST',
  'CREATE_DOCX', 'CREATE_XLSX', 'CREATE_PPTX', 'CREATE_MP3', 'EXPORT_PDF', 'PLAN',
];

/**
 * Strips >>>[ACTION ...]<<< protocol markers from assistant text before it's
 * rendered. Deltas from chat://token are the model's raw output, and the
 * backend only strips triggers once the full response has finished
 * streaming — without this, users watch raw trigger syntax (including
 * escaped file content) flash through the chat bubble as it streams in.
 *
 * Mirrors the quote-aware trigger-end scanning in
 * src-tauri/src/triggers/parser.rs, including its lenient fallback for a
 * bare `[ACTION ...]<<<` (weaker models sometimes drop the ">>>" prefix) so
 * the display hides exactly what the backend will actually execute. A
 * trigger that hasn't closed yet (still streaming in) is hidden entirely
 * rather than shown half-formed.
 */
export function stripTriggers(text: string): string {
  let result = '';
  let i = 0;
  const len = text.length;

  while (i < len) {
    const open = findTriggerOpen(text, i);
    if (!open) {
      result += text.slice(i);
      break;
    }
    result += text.slice(i, open.index);
    const endPos = findTriggerEnd(text, open.index + open.openLen);
    if (endPos === -1) {
      // Unterminated: either still streaming or malformed. Either way,
      // don't show the raw marker.
      break;
    }
    i = endPos;
  }

  return result.trim();
}

export interface InFlightAction {
  action: string;
  path?: string;
}

/**
 * Looks at the tail of a still-streaming response for a trigger that has
 * opened but not yet closed, and reports what it's doing (action + path,
 * once its path parameter has fully arrived) so the UI can show a live
 * "Creating coucou.md…" status instead of a bare blinking cursor while the
 * model is mid-trigger.
 */
export function getInFlightAction(text: string): InFlightAction | null {
  let i = 0;
  let open: { index: number; openLen: number } | null = null;

  while (i < text.length) {
    const found = findTriggerOpen(text, i);
    if (!found) return null;
    const end = findTriggerEnd(text, found.index + found.openLen);
    if (end === -1) {
      open = found;
      break;
    }
    i = end;
  }

  if (!open) return null;

  const inner = text.slice(open.index + open.openLen);
  const actionMatch = /^([A-Z]+)/.exec(inner);
  if (!actionMatch) return null;
  const action = actionMatch[1];
  if (!KNOWN_ACTIONS.includes(action)) return null;

  const pathMatch = /^[A-Z]+\s+"([^"]*)"/.exec(inner);
  return { action, path: pathMatch?.[1] };
}

/** Finds the next trigger opener at or after `from`: the canonical ">>>["
 * or a bare "[ACTION" (missing the ">>>" prefix). */
function findTriggerOpen(text: string, from: number): { index: number; openLen: number } | null {
  for (let i = from; i < text.length; i++) {
    if (text.startsWith(START, i)) {
      return { index: i, openLen: START.length };
    }
    if (text[i] === '[' && bareActionLenAt(text, i + 1)) {
      return { index: i, openLen: 1 };
    }
  }
  return null;
}

function bareActionLenAt(text: string, from: number): boolean {
  for (const action of KNOWN_ACTIONS) {
    if (text.startsWith(action, from)) {
      const next = text[from + action.length];
      if (next === ' ' || next === '"' || next === ']') return true;
    }
  }
  return false;
}

function findTriggerEnd(s: string, from: number): number {
  let i = from;
  let inQuotes = false;
  let escaped = false;
  const len = s.length;

  while (i < len) {
    if (escaped) {
      escaped = false;
      i += 1;
      continue;
    }
    const c = s[i];
    if (c === '\\' && inQuotes) {
      escaped = true;
      i += 1;
      continue;
    }
    if (c === '"') {
      inQuotes = !inQuotes;
      i += 1;
      continue;
    }
    if (!inQuotes && s.startsWith(END, i)) {
      return i + END.length;
    }
    i += 1;
  }
  return -1;
}
