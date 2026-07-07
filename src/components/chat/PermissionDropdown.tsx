import { useState, useEffect, useRef, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { Shield, ChevronDown } from 'lucide-react';
import { usePermissionStore } from '../../stores/permissionStore';
import { useUIStore } from '../../stores/uiStore';

const LEVEL_COLORS: Record<string, string> = {
  chat_only: 'var(--text-muted)',
  read_preview: '#3b82f6',
  full_access: '#f59e0b',
};

export function PermissionDropdown() {
  const config = usePermissionStore(s => s.config);
  const activeLevel = usePermissionStore(s => s.activeLevel);
  const setLevel = usePermissionStore(s => s.setLevel);
  const loadConfig = usePermissionStore(s => s.loadConfig);
  const provider = useUIStore(s => s.selectedProvider);
  const getLevel = usePermissionStore(s => s.getLevel);
  const [open, setOpen] = useState(false);
  const btnRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const [menuPos, setMenuPos] = useState<{ top: number; left: number }>({ top: 0, left: 0 });

  useEffect(() => {
    if (!config) loadConfig();
  }, [config, loadConfig]);

  useEffect(() => {
    getLevel(provider);
  }, [provider, getLevel]);

  const updatePosition = useCallback(() => {
    if (!btnRef.current) return;
    const rect = btnRef.current.getBoundingClientRect();
    setMenuPos({
      top: rect.top,
      left: rect.right,
    });
  }, []);

  useEffect(() => {
    if (!open) return;
    updatePosition();
    const handleClick = (e: MouseEvent) => {
      if (
        btnRef.current?.contains(e.target as Node) ||
        menuRef.current?.contains(e.target as Node)
      ) return;
      setOpen(false);
    };
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [open, updatePosition]);

  if (!config) return null;

  const currentLevelId = activeLevel[provider] ?? 'chat_only';
  const currentLevel = config.levels[currentLevelId];
  const providerConfig = config.providers[provider];
  const availableLevels = providerConfig?.available_levels ?? Object.keys(config.levels);

  const handleSelect = async (levelId: string) => {
    setOpen(false);
    await setLevel(provider, levelId);
  };

  return (
    <>
      <button
        ref={btnRef}
        onClick={() => setOpen(o => !o)}
        title={currentLevel?.description ?? 'Permission level'}
        style={{
          background: 'none', border: 'none', borderRadius: 4,
          padding: '4px 6px', cursor: 'pointer', fontSize: 11,
          color: LEVEL_COLORS[currentLevelId] ?? 'var(--text-muted)',
          display: 'flex', alignItems: 'center', gap: 3,
          whiteSpace: 'nowrap', flexShrink: 0,
        }}
      >
        <Shield size={12} />
        {currentLevel?.label ?? 'Chat Only'}
        <ChevronDown size={10} />
      </button>
      {open && createPortal(
        <div
          ref={menuRef}
          style={{
            position: 'fixed',
            bottom: `${window.innerHeight - menuPos.top + 4}px`,
            right: `${window.innerWidth - menuPos.left}px`,
            background: 'var(--bg-surface)', border: '1px solid var(--border)',
            borderRadius: 8, boxShadow: '0 4px 16px rgba(0,0,0,0.12)',
            zIndex: 9999, minWidth: 200, overflow: 'hidden',
          }}
        >
          {availableLevels.map(levelId => {
            const level = config.levels[levelId];
            if (!level) return null;
            const isActive = levelId === currentLevelId;
            return (
              <button
                key={levelId}
                onClick={() => handleSelect(levelId)}
                style={{
                  width: '100%', padding: '8px 12px',
                  background: isActive ? 'var(--overlay)' : 'none',
                  border: 'none', cursor: 'pointer', textAlign: 'left',
                  fontSize: 12, color: 'var(--text-primary)',
                  display: 'flex', flexDirection: 'column', gap: 2,
                }}
              >
                <span style={{
                  display: 'flex', alignItems: 'center', gap: 6,
                  color: LEVEL_COLORS[levelId] ?? 'var(--text-primary)',
                  fontWeight: isActive ? 600 : 400,
                }}>
                  <Shield size={11} />
                  {level.label}
                </span>
                <span style={{ fontSize: 10, color: 'var(--text-muted)', paddingLeft: 17 }}>
                  {level.description}
                </span>
              </button>
            );
          })}
        </div>,
        document.body,
      )}
    </>
  );
}
