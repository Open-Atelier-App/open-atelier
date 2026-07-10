import { ChefHat, Clock3, MapPin, ExternalLink, Kanban as KanbanIcon, Sun, Cloud, CloudRain, CloudSnow, CloudLightning } from 'lucide-react';
import { open as openShell } from '@tauri-apps/plugin-shell';
import type { RecipeSpec, MapSpec, KanbanSpec, WeatherSpec } from '../../lib/vizSpecs';
import { mapUrl } from '../../lib/vizSpecs';

export function RecipeCard({ recipe }: { recipe: RecipeSpec }) {
  return (
    <div style={{
      border: '1px solid var(--border)', borderRadius: 8, overflow: 'hidden',
      margin: '4px 0 12px', maxWidth: 420,
    }}>
      {recipe.image && (
        <img src={recipe.image} alt={recipe.title} style={{ width: '100%', maxHeight: 180, objectFit: 'cover', display: 'block' }} />
      )}
      <div style={{ padding: '12px 16px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
          <ChefHat size={16} color="var(--accent)" style={{ flexShrink: 0 }} />
          <span style={{ fontSize: 15, fontWeight: 600, color: 'var(--text-primary)' }}>{recipe.title}</span>
        </div>
        {(recipe.prepTime || recipe.cookTime) && (
          <div style={{ display: 'flex', gap: 14, fontSize: 12, color: 'var(--text-muted)', marginBottom: 10 }}>
            {recipe.prepTime && <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}><Clock3 size={12} /> Prep {recipe.prepTime}</span>}
            {recipe.cookTime && <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}><Clock3 size={12} /> Cook {recipe.cookTime}</span>}
          </div>
        )}
        {recipe.ingredients.length > 0 && (
          <div style={{ marginBottom: 10 }}>
            <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '0.04em', marginBottom: 4 }}>
              Ingredients
            </div>
            <ul style={{ margin: 0, paddingLeft: 18, fontSize: 13, color: 'var(--text-primary)' }}>
              {recipe.ingredients.map((ing, i) => <li key={i}>{ing}</li>)}
            </ul>
          </div>
        )}
        {recipe.steps.length > 0 && (
          <div>
            <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '0.04em', marginBottom: 4 }}>
              Steps
            </div>
            <ol style={{ margin: 0, paddingLeft: 18, fontSize: 13, color: 'var(--text-primary)' }}>
              {recipe.steps.map((step, i) => <li key={i} style={{ marginBottom: 4 }}>{step}</li>)}
            </ol>
          </div>
        )}
        {recipe.notes && (
          <div style={{ marginTop: 10, fontSize: 12, color: 'var(--text-muted)', fontStyle: 'italic' }}>
            {recipe.notes}
          </div>
        )}
      </div>
    </div>
  );
}

export function MapCard({ spec }: { spec: MapSpec }) {
  const label = spec.label ?? spec.address ?? (spec.lat !== undefined ? `${spec.lat.toFixed(4)}, ${spec.lng?.toFixed(4)}` : 'Location');
  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 12,
      border: '1px solid var(--border)', borderRadius: 8, padding: '12px 16px',
      margin: '4px 0 12px', maxWidth: 420,
    }}>
      <div style={{
        width: 36, height: 36, borderRadius: '50%', background: 'var(--overlay)', flexShrink: 0,
        display: 'flex', alignItems: 'center', justifyContent: 'center',
      }}>
        <MapPin size={18} color="var(--accent)" />
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 14, fontWeight: 600, color: 'var(--text-primary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {label}
        </div>
        {spec.label && spec.address && (
          <div style={{ fontSize: 12, color: 'var(--text-muted)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {spec.address}
          </div>
        )}
      </div>
      <button
        onClick={() => openShell(mapUrl(spec)).catch((e: unknown) => console.error('Failed to open map', e))}
        style={{
          display: 'flex', alignItems: 'center', gap: 6, flexShrink: 0,
          padding: '6px 12px', borderRadius: 6, fontSize: 12, fontWeight: 500,
          border: '1px solid var(--border)', background: 'var(--overlay)', color: 'var(--text-primary)', cursor: 'pointer',
        }}
      >
        Open in Maps <ExternalLink size={12} />
      </button>
    </div>
  );
}

export function KanbanBoard({ board }: { board: KanbanSpec }) {
  return (
    <div style={{ margin: '4px 0 12px' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 8, fontSize: 12, color: 'var(--text-muted)' }}>
        <KanbanIcon size={13} />
        Board
      </div>
      <div style={{ display: 'flex', gap: 10, overflowX: 'auto', paddingBottom: 4 }}>
        {board.columns.map((col, i) => (
          <div key={i} style={{
            flexShrink: 0, width: 180, background: 'var(--overlay)', borderRadius: 8, padding: 10,
          }}>
            <div style={{
              fontSize: 11, fontWeight: 600, color: 'var(--text-muted)', textTransform: 'uppercase',
              letterSpacing: '0.04em', marginBottom: 8, display: 'flex', justifyContent: 'space-between',
            }}>
              <span>{col.title}</span>
              <span>{col.cards.length}</span>
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              {col.cards.map((card, ci) => (
                <div key={ci} style={{
                  background: 'var(--bg-surface)', border: '1px solid var(--border)', borderRadius: 6,
                  padding: '6px 8px', fontSize: 12.5, color: 'var(--text-primary)',
                }}>
                  {card}
                </div>
              ))}
              {col.cards.length === 0 && (
                <div style={{ fontSize: 11, color: 'var(--text-muted)', fontStyle: 'italic' }}>Empty</div>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

const CONDITION_ICON: Record<string, typeof Sun> = {
  sunny: Sun, clear: Sun, rain: CloudRain, rainy: CloudRain,
  snow: CloudSnow, snowy: CloudSnow, storm: CloudLightning, thunderstorm: CloudLightning,
};

export function WeatherCard({ weather }: { weather: WeatherSpec }) {
  const Icon = CONDITION_ICON[weather.condition.toLowerCase()] ?? Cloud;
  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 14,
      border: '1px solid var(--border)', borderRadius: 8, padding: '14px 18px',
      margin: '4px 0 12px', maxWidth: 320,
    }}>
      <Icon size={32} color="var(--accent)" style={{ flexShrink: 0 }} />
      <div>
        <div style={{ fontSize: 14, fontWeight: 600, color: 'var(--text-primary)' }}>{weather.city}</div>
        <div style={{ fontSize: 12, color: 'var(--text-muted)', marginBottom: 2 }}>{weather.condition}</div>
        <div style={{ display: 'flex', gap: 10, fontSize: 13, fontVariantNumeric: 'tabular-nums' }}>
          {weather.tempC !== undefined && (
            <span style={{ fontWeight: 700, color: 'var(--text-primary)' }}>{Math.round(weather.tempC)}°C</span>
          )}
          {weather.high !== undefined && weather.low !== undefined && (
            <span style={{ color: 'var(--text-muted)' }}>H:{Math.round(weather.high)}° L:{Math.round(weather.low)}°</span>
          )}
        </div>
      </div>
    </div>
  );
}
