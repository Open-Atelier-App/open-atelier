import { Shield } from 'lucide-react';
import { usePermissionStore } from '../../stores/permissionStore';
import { useUIStore } from '../../stores/uiStore';

const LEVEL_COLORS: Record<string, string> = {
  chat_only: 'var(--text-muted)',
  read_preview: '#3b82f6',
  full_access: '#f59e0b',
};

export function PermissionBadge() {
  const config = usePermissionStore(s => s.config);
  const activeLevel = usePermissionStore(s => s.activeLevel);
  const provider = useUIStore(s => s.selectedProvider);

  const levelId = activeLevel[provider] ?? 'chat_only';
  const level = config?.levels[levelId];

  return (
    <span
      title={level?.description ?? 'Permission level'}
      style={{
        display: 'inline-flex', alignItems: 'center', gap: 3,
        fontSize: 10, color: LEVEL_COLORS[levelId] ?? 'var(--text-muted)',
        padding: '2px 6px', borderRadius: 4,
        background: 'var(--overlay)',
      }}
    >
      <Shield size={10} />
      {level?.label ?? 'Chat Only'}
    </span>
  );
}
