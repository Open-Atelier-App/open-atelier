import { useState } from 'react';
import type { Dataset, ChartSpec } from '../../lib/chartSpec';

// Fixed-order categorical palette, validated for CVD-safe adjacent contrast
// (see dataviz skill's reference palette) — used once a chart has more than
// one series. A single-series chart uses the app's own accent color instead,
// so a lone bar/line chart still reads as "this app's," not a generic one.
const CATEGORICAL: { light: string; dark: string }[] = [
  { light: '#2a78d6', dark: '#3987e5' }, // blue
  { light: '#1baf7a', dark: '#199e70' }, // aqua
  { light: '#eda100', dark: '#c98500' }, // yellow
  { light: '#4a3aa7', dark: '#9085e9' }, // violet
  { light: '#e34948', dark: '#e66767' }, // red
  { light: '#e87ba4', dark: '#d55181' }, // magenta
];

/** "Is dark mode active" — same test the app's own theme toggle drives (see uiStore.setTheme). */
function isDark(): boolean {
  const attr = document.documentElement.getAttribute('data-theme');
  if (attr === 'dark') return true;
  if (attr === 'light') return false;
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

function seriesColor(index: number, seriesCount: number, dark: boolean): string {
  if (seriesCount <= 1) return 'var(--accent)';
  const slot = CATEGORICAL[index % CATEGORICAL.length];
  return dark ? slot.dark : slot.light;
}

/** Rounds a max value up to a "clean" tick ceiling (1/2/5 * 10^n) so axis labels read as round numbers. */
function niceCeiling(max: number): number {
  if (max <= 0) return 1;
  const pow = Math.pow(10, Math.floor(Math.log10(max)));
  const fraction = max / pow;
  const step = fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 5 ? 5 : 10;
  return step * pow;
}

const WIDTH = 520;
const HEIGHT = 260;
const PAD = { top: 16, right: 16, bottom: 28, left: 44 };
const PLOT_W = WIDTH - PAD.left - PAD.right;
const PLOT_H = HEIGHT - PAD.top - PAD.bottom;

export function ChartBlock({ spec }: { spec: ChartSpec }) {
  const [hover, setHover] = useState<{ x: number; y: number; label: string; value: number; series?: string } | null>(null);
  const dark = isDark();
  const { labels, datasets } = spec.data;
  const multiSeries = datasets.length > 1;

  if (spec.type === 'pie') {
    return <PieChart labels={labels} dataset={datasets[0]} dark={dark} hover={hover} setHover={setHover} />;
  }

  const allValues = datasets.flatMap(d => d.data);
  const maxValue = niceCeiling(Math.max(0, ...allValues));
  const ticks = [0, 0.25, 0.5, 0.75, 1].map(f => Math.round(maxValue * f));

  const xFor = (i: number) => PAD.left + (labels.length <= 1 ? PLOT_W / 2 : (i / (labels.length - 1)) * PLOT_W);
  const yFor = (v: number) => PAD.top + PLOT_H - (v / maxValue) * PLOT_H;

  return (
    <div style={{ margin: '4px 0 12px' }}>
      <div style={{ overflowX: 'auto' }}>
        <svg
          viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
          width="100%"
          style={{ maxWidth: WIDTH, display: 'block', fontFamily: 'inherit' }}
          onMouseLeave={() => setHover(null)}
        >
          {/* Gridlines — recessive hairlines, one step off the surface */}
          {ticks.map(t => (
            <line
              key={t}
              x1={PAD.left} x2={WIDTH - PAD.right}
              y1={yFor(t)} y2={yFor(t)}
              stroke="var(--border)" strokeWidth={1}
            />
          ))}
          {/* Y-axis tick labels */}
          {ticks.map(t => (
            <text
              key={t}
              x={PAD.left - 8} y={yFor(t)}
              textAnchor="end" dominantBaseline="middle"
              fontSize={10} fill="var(--text-muted)"
              style={{ fontVariantNumeric: 'tabular-nums' }}
            >
              {t.toLocaleString()}
            </text>
          ))}
          {/* X-axis category labels */}
          {labels.map((label, i) => (
            <text
              key={label + i}
              x={xFor(i)} y={HEIGHT - 8}
              textAnchor="middle"
              fontSize={10} fill="var(--text-muted)"
            >
              {label}
            </text>
          ))}

          {spec.type === 'bar' && datasets.map((ds, dsIndex) => {
            const groupW = PLOT_W / labels.length;
            const barW = Math.min(24, (groupW * 0.6) / datasets.length);
            const color = seriesColor(dsIndex, datasets.length, dark);
            return ds.data.map((v, i) => {
              const groupStart = PAD.left + i * groupW + groupW / 2 - (barW * datasets.length) / 2;
              const x = groupStart + dsIndex * barW;
              const y = yFor(Math.max(0, v));
              const h = PLOT_H - (y - PAD.top);
              return (
                <rect
                  key={i}
                  x={x} y={y} width={Math.max(1, barW - 2)} height={Math.max(0, h)}
                  rx={4} ry={4}
                  fill={color}
                  onMouseEnter={() => setHover({ x: x + barW / 2, y, label: labels[i], value: v, series: ds.label })}
                />
              );
            });
          })}

          {spec.type === 'line' && datasets.map((ds, dsIndex) => {
            const color = seriesColor(dsIndex, datasets.length, dark);
            const points = ds.data.map((v, i) => `${xFor(i)},${yFor(v)}`).join(' ');
            return (
              <g key={dsIndex}>
                <polyline points={points} fill="none" stroke={color} strokeWidth={2} strokeLinejoin="round" strokeLinecap="round" />
                {ds.data.map((v, i) => (
                  <circle
                    key={i}
                    cx={xFor(i)} cy={yFor(v)} r={4}
                    fill={color} stroke="var(--bg-surface)" strokeWidth={2}
                    onMouseEnter={() => setHover({ x: xFor(i), y: yFor(v), label: labels[i], value: v, series: ds.label })}
                  />
                ))}
              </g>
            );
          })}

          {hover && (
            <g pointerEvents="none">
              <line x1={hover.x} x2={hover.x} y1={PAD.top} y2={HEIGHT - PAD.bottom} stroke="var(--text-muted)" strokeWidth={1} strokeDasharray="2,2" opacity={0.5} />
            </g>
          )}
        </svg>
      </div>

      {hover && (
        <div style={{
          fontSize: 12, color: 'var(--text-primary)', background: 'var(--overlay)',
          borderRadius: 4, padding: '4px 8px', display: 'inline-flex', gap: 6, marginTop: 4,
        }}>
          <span style={{ color: 'var(--text-muted)' }}>{hover.label}{hover.series ? ` · ${hover.series}` : ''}</span>
          <span style={{ fontVariantNumeric: 'tabular-nums', fontWeight: 600 }}>{hover.value.toLocaleString()}</span>
        </div>
      )}

      {multiSeries && (
        <div style={{ display: 'flex', gap: 14, flexWrap: 'wrap', marginTop: 8 }}>
          {datasets.map((ds, i) => (
            <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 5, fontSize: 11, color: 'var(--text-muted)' }}>
              <span style={{ width: 8, height: 8, borderRadius: '50%', background: seriesColor(i, datasets.length, dark), flexShrink: 0 }} />
              {ds.label ?? `Series ${i + 1}`}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function PieChart({
  labels, dataset, dark, hover, setHover,
}: {
  labels: string[];
  dataset: Dataset;
  dark: boolean;
  hover: { x: number; y: number; label: string; value: number; series?: string } | null;
  setHover: (h: { x: number; y: number; label: string; value: number; series?: string } | null) => void;
}) {
  const size = 220;
  const r = size / 2 - 4;
  const cx = size / 2;
  const cy = size / 2;
  const total = dataset.data.reduce((a, b) => a + b, 0) || 1;

  // Cumulative start angle per slice, computed up front rather than mutating
  // a running counter inside the render map below.
  const startAngles = dataset.data.reduce<number[]>((acc) => {
    const prevEnd = acc.length > 0 ? acc[acc.length - 1] + (dataset.data[acc.length - 1] / total) * 360 : -90;
    acc.push(prevEnd);
    return acc;
  }, []);

  const slices = dataset.data.map((v, i) => {
    const fraction = v / total;
    const startAngle = startAngles[i];
    const endAngle = startAngle + fraction * 360;
    const large = fraction > 0.5 ? 1 : 0;
    const toXY = (a: number) => {
      const rad = (a * Math.PI) / 180;
      return [cx + r * Math.cos(rad), cy + r * Math.sin(rad)];
    };
    const [x1, y1] = toXY(startAngle);
    const [x2, y2] = toXY(endAngle);
    const path = `M ${cx} ${cy} L ${x1} ${y1} A ${r} ${r} 0 ${large} 1 ${x2} ${y2} Z`;
    const midAngle = (startAngle + endAngle) / 2;
    const midRad = (midAngle * Math.PI) / 180;
    const labelPos: [number, number] = [cx + r * 0.68 * Math.cos(midRad), cy + r * 0.68 * Math.sin(midRad)];
    return { path, fraction, label: labels[i], value: v, color: seriesColor(i, dataset.data.length, dark), labelPos };
  });

  return (
    <div style={{ margin: '4px 0 12px' }}>
      <div style={{ display: 'flex', gap: 20, flexWrap: 'wrap', alignItems: 'center' }}>
        <svg
          viewBox={`0 0 ${size} ${size}`} width={size} height={size}
          onMouseLeave={() => setHover(null)}
        >
          {slices.map((s, i) => (
            <path
              key={i}
              d={s.path}
              fill={s.color}
              stroke="var(--bg-surface)"
              strokeWidth={2}
              onMouseEnter={() => setHover({ x: s.labelPos[0], y: s.labelPos[1], label: s.label, value: s.value })}
            />
          ))}
          {slices.filter(s => s.fraction > 0.08).map((s, i) => (
            <text
              key={i}
              x={s.labelPos[0]} y={s.labelPos[1]}
              textAnchor="middle" dominantBaseline="middle"
              fontSize={11} fontWeight={600} fill="#fff"
              style={{ fontVariantNumeric: 'tabular-nums', pointerEvents: 'none' }}
            >
              {Math.round(s.fraction * 100)}%
            </text>
          ))}
        </svg>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          {slices.map((s, i) => (
            <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12, color: 'var(--text-muted)' }}>
              <span style={{ width: 9, height: 9, borderRadius: '50%', background: s.color, flexShrink: 0 }} />
              {s.label}
              <span style={{ fontVariantNumeric: 'tabular-nums', color: 'var(--text-primary)', fontWeight: 600 }}>{s.value.toLocaleString()}</span>
            </div>
          ))}
        </div>
      </div>
      {hover && (
        <div style={{
          fontSize: 12, color: 'var(--text-primary)', background: 'var(--overlay)',
          borderRadius: 4, padding: '4px 8px', display: 'inline-flex', gap: 6, marginTop: 8,
        }}>
          <span style={{ color: 'var(--text-muted)' }}>{hover.label}</span>
          <span style={{ fontVariantNumeric: 'tabular-nums', fontWeight: 600 }}>{hover.value.toLocaleString()}</span>
        </div>
      )}
    </div>
  );
}
