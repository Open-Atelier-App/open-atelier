export interface Dataset {
  label?: string;
  data: number[];
}

export interface ChartSpec {
  type: 'bar' | 'line' | 'pie';
  data: {
    labels: string[];
    datasets: Dataset[];
  };
}

export function parseChartSpec(raw: string): ChartSpec | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  if (typeof parsed !== 'object' || parsed === null) return null;
  const obj = parsed as Record<string, unknown>;
  const type = obj.type;
  if (type !== 'bar' && type !== 'line' && type !== 'pie') return null;
  const data = obj.data as Record<string, unknown> | undefined;
  const labels = data?.labels;
  const datasets = data?.datasets;
  if (!Array.isArray(labels) || !Array.isArray(datasets) || datasets.length === 0) return null;
  const cleanDatasets: Dataset[] = [];
  for (const ds of datasets) {
    if (typeof ds !== 'object' || ds === null || !Array.isArray((ds as Record<string, unknown>).data)) return null;
    cleanDatasets.push({
      label: typeof (ds as Record<string, unknown>).label === 'string' ? (ds as Record<string, unknown>).label as string : undefined,
      data: ((ds as Record<string, unknown>).data as unknown[]).map(Number),
    });
  }
  return { type, data: { labels: labels.map(String), datasets: cleanDatasets } };
}
