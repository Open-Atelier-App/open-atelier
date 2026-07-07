import { PROVIDER_BADGE } from '../../lib/types';

interface Props {
  provider: string;
  size?: number;
}

/** Small colored monogram standing in for the provider's brand. */
export function ProviderBadge({ provider, size = 14 }: Props) {
  const badge = PROVIDER_BADGE[provider] ?? { letter: '?', color: 'var(--text-muted)' };
  return (
    <span
      title={provider}
      style={{
        width: size, height: size, borderRadius: '50%', flexShrink: 0,
        background: badge.color, color: '#fff',
        fontSize: size * 0.6, fontWeight: 700, lineHeight: 1,
        display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
      }}
    >
      {badge.letter}
    </span>
  );
}
