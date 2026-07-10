function asRecord(v: unknown): Record<string, unknown> | null {
  return typeof v === 'object' && v !== null ? v as Record<string, unknown> : null;
}

function asStringArray(v: unknown): string[] {
  return Array.isArray(v) ? v.map(String) : [];
}

export interface RecipeSpec {
  title: string;
  image?: string;
  prepTime?: string;
  cookTime?: string;
  ingredients: string[];
  steps: string[];
  notes?: string;
}

export function parseRecipeSpec(raw: string): RecipeSpec | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  const obj = asRecord(parsed);
  if (!obj || typeof obj.title !== 'string') return null;
  const ingredients = asStringArray(obj.ingredients);
  const steps = asStringArray(obj.steps);
  if (ingredients.length === 0 && steps.length === 0) return null;
  return {
    title: obj.title,
    image: typeof obj.image === 'string' ? obj.image : undefined,
    prepTime: typeof obj.prepTime === 'string' ? obj.prepTime : undefined,
    cookTime: typeof obj.cookTime === 'string' ? obj.cookTime : undefined,
    ingredients,
    steps,
    notes: typeof obj.notes === 'string' ? obj.notes : undefined,
  };
}

export interface MapSpec {
  lat?: number;
  lng?: number;
  address?: string;
  label?: string;
}

export function parseMapSpec(raw: string): MapSpec | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  const obj = asRecord(parsed);
  if (!obj) return null;
  const hasLatLng = typeof obj.lat === 'number' && typeof obj.lng === 'number';
  const hasAddress = typeof obj.address === 'string' && obj.address.trim().length > 0;
  if (!hasLatLng && !hasAddress) return null;
  return {
    lat: typeof obj.lat === 'number' ? obj.lat : undefined,
    lng: typeof obj.lng === 'number' ? obj.lng : undefined,
    address: typeof obj.address === 'string' ? obj.address : undefined,
    label: typeof obj.label === 'string' ? obj.label : undefined,
  };
}

/** A openstreetmap.org URL — no API key needed, unlike Google Maps — used
 * for the "Open in Maps" button; the system browser handles the rest. */
export function mapUrl(spec: MapSpec): string {
  if (spec.lat !== undefined && spec.lng !== undefined) {
    return `https://www.openstreetmap.org/?mlat=${spec.lat}&mlon=${spec.lng}#map=15/${spec.lat}/${spec.lng}`;
  }
  return `https://www.openstreetmap.org/search?query=${encodeURIComponent(spec.address ?? '')}`;
}

export interface KanbanColumn {
  title: string;
  cards: string[];
}

export interface KanbanSpec {
  columns: KanbanColumn[];
}

export function parseKanbanSpec(raw: string): KanbanSpec | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  const obj = asRecord(parsed);
  const columnsRaw = obj?.columns;
  if (!Array.isArray(columnsRaw) || columnsRaw.length === 0) return null;
  const columns: KanbanColumn[] = [];
  for (const col of columnsRaw) {
    const colObj = asRecord(col);
    if (!colObj || typeof colObj.title !== 'string') return null;
    columns.push({ title: colObj.title, cards: asStringArray(colObj.cards) });
  }
  return { columns };
}

export interface WeatherSpec {
  city: string;
  condition: string;
  tempC?: number;
  high?: number;
  low?: number;
}

export function parseWeatherSpec(raw: string): WeatherSpec | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  const obj = asRecord(parsed);
  if (!obj || typeof obj.city !== 'string' || typeof obj.condition !== 'string') return null;
  return {
    city: obj.city,
    condition: obj.condition,
    tempC: typeof obj.tempC === 'number' ? obj.tempC : undefined,
    high: typeof obj.high === 'number' ? obj.high : undefined,
    low: typeof obj.low === 'number' ? obj.low : undefined,
  };
}
