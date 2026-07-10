import { useState, useEffect, type ReactNode } from 'react';
import { X, Eye, Check, AlertCircle, FolderOpen, Pencil, Trash2, Plus, ExternalLink, Loader, RefreshCw } from 'lucide-react';
import { open as openDialog, confirm as confirmDialog } from '@tauri-apps/plugin-dialog';
import { open as openShell } from '@tauri-apps/plugin-shell';
import * as api from '../../lib/tauri';
import type { KeyStatus, Conversation, Message } from '../../lib/types';
import { useUIStore } from '../../stores/uiStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useProfileStore } from '../../stores/profileStore';
import type { SkillInfo } from '../../lib/tauri';
import { GithubConnectButton, GoogleDriveConnectButton } from './OAuthConnectButtons';
import { checkForUpdates } from '../../lib/updater';

interface Props {
  onClose: () => void;
}

type CredType = 'api_key' | 'bearer_token' | 'azure' | 'ollama_url';

interface ProviderConfig {
  id: string;
  label: string;
  credTypes: { id: CredType; label: string; placeholder: string; multiline?: boolean }[];
  note?: string;
  keyUrl?: string;
}

const PROVIDERS: ProviderConfig[] = [
  {
    id: 'anthropic',
    label: 'Anthropic',
    credTypes: [
      { id: 'api_key', label: 'API Key', placeholder: 'sk-ant-...' },
    ],
    keyUrl: 'https://console.anthropic.com/settings/keys',
  },
  {
    id: 'openai',
    label: 'OpenAI',
    credTypes: [
      { id: 'api_key', label: 'API Key', placeholder: 'sk-...' },
      { id: 'bearer_token', label: 'Bearer Token (org/project)', placeholder: 'Bearer sk-proj-...' },
    ],
    keyUrl: 'https://platform.openai.com/api-keys',
  },
  {
    id: 'google',
    label: 'Google Gemini',
    credTypes: [
      { id: 'api_key', label: 'API Key', placeholder: 'AIza...' },
      { id: 'bearer_token', label: 'OAuth Bearer Token', placeholder: 'ya29...' },
    ],
    keyUrl: 'https://aistudio.google.com/apikey',
  },
  {
    id: 'openai-azure',
    label: 'Azure OpenAI',
    credTypes: [
      {
        id: 'azure',
        label: 'Endpoint + Key (JSON)',
        placeholder: '{"endpoint":"https://…","key":"…","deployment":"gpt-4o"}',
        multiline: true,
      },
    ],
    keyUrl: 'https://portal.azure.com',
  },
  {
    id: 'ollama',
    label: 'Ollama (local)',
    credTypes: [
      { id: 'ollama_url', label: 'Base URL', placeholder: 'http://localhost:11434' },
    ],
    keyUrl: 'https://ollama.ai',
  },
  {
    id: 'groq',
    label: 'Groq',
    credTypes: [
      { id: 'api_key', label: 'API Key', placeholder: 'gsk_...' },
    ],
    keyUrl: 'https://console.groq.com/keys',
  },
  {
    id: 'openrouter',
    label: 'OpenRouter',
    credTypes: [
      { id: 'api_key', label: 'API Key', placeholder: 'sk-or-...' },
    ],
    keyUrl: 'https://openrouter.ai/keys',
  },
  {
    id: 'mistral',
    label: 'Mistral AI',
    credTypes: [
      { id: 'api_key', label: 'API Key', placeholder: '...' },
    ],
    keyUrl: 'https://console.mistral.ai',
  },
  {
    id: 'together',
    label: 'Together AI',
    credTypes: [
      { id: 'api_key', label: 'API Key', placeholder: '...' },
    ],
    keyUrl: 'https://api.together.ai/settings/api-keys',
  },
  {
    id: 'deepseek',
    label: 'DeepSeek',
    credTypes: [
      { id: 'api_key', label: 'API Key', placeholder: 'sk-...' },
    ],
    keyUrl: 'https://platform.deepseek.com/api_keys',
  },
  {
    id: 'xai',
    label: 'xAI (Grok)',
    credTypes: [
      { id: 'api_key', label: 'API Key', placeholder: 'xai-...' },
    ],
    keyUrl: 'https://console.x.ai',
  },
];

const PRICING_PER_MILLION: Record<string, { input: number; output: number }> = {
  anthropic: { input: 3, output: 15 },
  openai: { input: 2.5, output: 10 },
  'openai-codex': { input: 1.5, output: 6 },
  google: { input: 0.075, output: 0.3 },
  ollama: { input: 0, output: 0 },
  groq: { input: 0.59, output: 0.79 },
  openrouter: { input: 0.15, output: 0.6 },
  mistral: { input: 2, output: 6 },
  together: { input: 0.88, output: 0.88 },
  deepseek: { input: 0.27, output: 1.1 },
  xai: { input: 3, output: 15 },
};

function estimateCost(provider: string | null, inputTokens: number, outputTokens: number): number {
  const rates = (provider && PRICING_PER_MILLION[provider]) || { input: 0, output: 0 };
  return (inputTokens / 1_000_000) * rates.input + (outputTokens / 1_000_000) * rates.output;
}

type Tab = 'Profiles' | 'LLMs' | 'Skills' | 'Connectors' | 'Appearance' | 'Usage' | 'About';

export function SettingsView({ onClose }: Props) {
  const settingsTab = useUIStore(s => s.settingsTab);
  const setSettingsTab = useUIStore(s => s.setSettingsTab);
  const [activeTab, setActiveTab] = useState<Tab>((settingsTab as Tab) || 'Profiles');
  const theme = useUIStore(s => s.theme);
  const setTheme = useUIStore(s => s.setTheme);
  const activeProfile = useProfileStore(s => s.active);

  // Follow external requests to jump to a specific tab (e.g. "Check for
  // updates" opening straight to About) even while already mounted.
  // Adjusted during render rather than in an effect for the same reason as
  // ProvidersTab above.
  const [lastSettingsTab, setLastSettingsTab] = useState(settingsTab);
  if (settingsTab !== lastSettingsTab) {
    setLastSettingsTab(settingsTab);
    if (settingsTab) {
      setActiveTab(settingsTab as Tab);
    }
  }

  const handleTabChange = (tab: Tab) => {
    setActiveTab(tab);
    setSettingsTab(tab);
  };

  const tabs: Tab[] = ['Profiles', 'LLMs', 'Skills', 'Connectors', 'Appearance', 'Usage', 'About'];

  return (
    <div style={{
      position: 'fixed', inset: 0, zIndex: 500,
      background: 'rgba(0,0,0,0.4)', display: 'flex', alignItems: 'center', justifyContent: 'center',
    }}
      onClick={e => e.target === e.currentTarget && onClose()}
    >
      <div style={{
        // Fixed height (not just a cap) so the dialog doesn't grow or
        // shrink as the user switches between tabs with very different
        // amounts of content (e.g. "About" vs. "Usage") — each tab's own
        // content area scrolls internally instead.
        width: 700, height: '82vh', background: 'var(--bg-surface)',
        borderRadius: 8, border: '1px solid var(--border)',
        display: 'flex', flexDirection: 'column', overflow: 'hidden',
        boxShadow: '0 8px 32px rgba(0,0,0,0.2)',
      }}>
        {/* Header */}
        <div style={{
          padding: '18px 24px 14px', borderBottom: '1px solid var(--border)',
          display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexShrink: 0,
        }}>
          <h2 style={{ margin: 0, fontSize: 16, fontWeight: 600, color: 'var(--text-primary)' }}>
            Settings
          </h2>
          <button onClick={onClose} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', padding: 4 }}>
            <X size={18} />
          </button>
        </div>

        {/* Body: left sidebar + right content */}
        <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
          {/* Left tab sidebar */}
          <div style={{ width: 140, borderRight: '1px solid var(--border)', padding: '12px 8px', flexShrink: 0 }}>
            {tabs.map(tab => (
              <button
                key={tab}
                onClick={() => handleTabChange(tab)}
                style={{
                  padding: '8px 12px', width: '100%', textAlign: 'left',
                  border: 'none', cursor: 'pointer', fontSize: 13, borderRadius: 4,
                  marginBottom: 2,
                  background: activeTab === tab ? 'var(--overlay)' : 'none',
                  color: activeTab === tab ? 'var(--text-primary)' : 'var(--text-muted)',
                  fontWeight: activeTab === tab ? 500 : 400,
                }}
              >
                {tab}
              </button>
            ))}
          </div>

          {/* Right content */}
          <div style={{ flex: 1, overflow: 'auto', padding: '20px 24px 24px' }}>
            {activeTab === 'Profiles' && <ProfilesTab />}
            {activeTab === 'LLMs' && <ProvidersTab profileId={activeProfile?.id ?? null} />}

            {activeTab === 'Skills' && <SkillsTab />}

            {activeTab === 'Connectors' && <ConnectorsTab profileId={activeProfile?.id ?? null} />}

            {activeTab === 'Appearance' && (
              <div>
                <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--text-primary)', marginBottom: 10 }}>
                  Theme
                </div>
                <div style={{ display: 'flex', gap: 8 }}>
                  {(['system', 'light', 'dark'] as const).map(t => (
                    <button
                      key={t}
                      onClick={() => setTheme(t)}
                      style={{
                        padding: '7px 16px', borderRadius: 4, fontSize: 13,
                        border: 'none', cursor: 'pointer',
                        background: theme === t ? 'var(--accent)' : 'var(--overlay)',
                        color: theme === t ? '#fff' : 'var(--text-muted)',
                        fontWeight: theme === t ? 500 : 400,
                      }}
                    >
                      {t.charAt(0).toUpperCase() + t.slice(1)}
                    </button>
                  ))}
                </div>
              </div>
            )}

            {activeTab === 'Usage' && <UsageTab />}

            {activeTab === 'About' && <AboutTab />}
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Providers Tab ──────────────────────────────────────────────────────────

interface ConnectedProvider {
  providerId: string;
  label: string;
  credType: CredType;
  maskedKey: string | null;
}

async function loadProviderLabels(): Promise<Record<string, string>> {
  const v = await api.settingsGet('provider_labels').catch(() => null);
  if (v && typeof v === 'object' && !Array.isArray(v)) return v as Record<string, string>;
  return {};
}

// Pure data fetch with no setState calls, so effects can drive it through a
// .then() callback without an eslint(react-hooks/set-state-in-effect)
// violation (the rule flags effects that synchronously call a function
// which itself sets state).
async function fetchConnectedProviders(profileId: number | null): Promise<ConnectedProvider[]> {
  const statuses: KeyStatus[] = profileId != null
    ? await api.keyListStatusProfile(profileId).catch(() => [])
    : await api.keyListStatus().catch(() => []);

  const existingProviders = statuses.filter(s => s.exists);
  const labels = await loadProviderLabels();

  const items: ConnectedProvider[] = [];
  for (const st of existingProviders) {
    const providerDef = PROVIDERS.find(p => p.id === st.provider);
    if (!providerDef) continue;

    const masked = await api.credGetMasked(st.provider, 'api_key', profileId ?? undefined).catch(() => null);
    items.push({
      providerId: st.provider,
      label: labels[st.provider] || providerDef.label,
      credType: providerDef.credTypes[0].id,
      maskedKey: masked,
    });
  }
  return items;
}

function ProvidersTab({ profileId }: { profileId: number | null }) {
  const [connected, setConnected] = useState<ConnectedProvider[]>([]);
  const [loading, setLoading] = useState(true);
  const [showConnect, setShowConnect] = useState(false);
  const [editingLabel, setEditingLabel] = useState<string | null>(null);
  const [editLabelValue, setEditLabelValue] = useState('');
  const [revealedId, setRevealedId] = useState<string | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<Record<string, boolean | null>>({});

  // Flip back to the loading state as soon as we're about to fetch for a
  // different profile. Adjusted during render rather than in the effect
  // below, since setting state synchronously at the top of an effect (or a
  // function it calls) causes an extra cascading render.
  const [loadedForProfileId, setLoadedForProfileId] = useState<number | null | undefined>(undefined);
  if (loadedForProfileId !== profileId) {
    setLoadedForProfileId(profileId);
    setLoading(true);
  }

  const loadConnected = async () => {
    const items = await fetchConnectedProviders(profileId);
    setConnected(items);
    setLoading(false);
  };

  const saveProviderLabel = async (providerId: string, label: string) => {
    const labels = await loadProviderLabels();
    labels[providerId] = label;
    await api.settingsSet('provider_labels', labels);
  };

  useEffect(() => {
    let ignore = false;
    fetchConnectedProviders(profileId).then(items => {
      if (!ignore) {
        setConnected(items);
        setLoading(false);
      }
    });
    return () => { ignore = true; };
  }, [profileId]);

  const handleDelete = async (providerId: string) => {
    const providerDef = PROVIDERS.find(p => p.id === providerId);
    if (!providerDef) return;
    const credType = providerDef.credTypes[0].id;
    try {
      if (profileId != null) {
        await api.credDeleteProfile(providerId, credType, profileId);
      } else {
        await api.credDelete(providerId, credType);
      }
      await api.keyDelete(providerId).catch(() => {});
      setConnected(c => c.filter(p => p.providerId !== providerId));
      setTestResults(r => { const next = { ...r }; delete next[providerId]; return next; });
    } catch (e) {
      console.error('Failed to delete provider', e);
    }
  };

  const handleTest = async (providerId: string) => {
    setTestingId(providerId);
    setTestResults(r => ({ ...r, [providerId]: null }));
    try {
      const ok = profileId != null
        ? await api.keyTestProfile(providerId, profileId)
        : await api.keyTest(providerId);
      setTestResults(r => ({ ...r, [providerId]: ok }));
    } catch {
      setTestResults(r => ({ ...r, [providerId]: false }));
    }
    setTestingId(null);
  };

  const handleRenameSubmit = async (providerId: string) => {
    if (editLabelValue.trim()) {
      await saveProviderLabel(providerId, editLabelValue.trim());
      setConnected(c => c.map(p =>
        p.providerId === providerId ? { ...p, label: editLabelValue.trim() } : p
      ));
    }
    setEditingLabel(null);
  };

  const connectedIds = new Set(connected.map(c => c.providerId));

  return (
    <div>
      <div style={{ marginBottom: 16, padding: '10px 14px', background: 'var(--overlay)', borderRadius: 6, fontSize: 12, color: 'var(--text-muted)', lineHeight: 1.6 }}>
        Your API keys are stored locally in an encrypted file on your device. Only you can access them.
      </div>

      {/* Connected providers table */}
      {loading ? (
        <div style={{ fontSize: 12, color: 'var(--text-muted)' }}>Loading...</div>
      ) : connected.length === 0 ? (
        <div style={{ fontSize: 12, color: 'var(--text-muted)', marginBottom: 16 }}>
          No LLMs connected yet. Click "Connect LLM" to add one.
        </div>
      ) : (
        <table style={{ width: '100%', borderCollapse: 'collapse', marginBottom: 16, fontSize: 12 }}>
          <thead>
            <tr style={{ background: 'var(--overlay)' }}>
              <th style={{ textAlign: 'left', padding: '8px 10px', border: '1px solid var(--border)', color: 'var(--text-primary)', fontWeight: 500 }}>Name</th>
              <th style={{ textAlign: 'left', padding: '8px 10px', border: '1px solid var(--border)', color: 'var(--text-primary)', fontWeight: 500 }}>Provider</th>
              <th style={{ textAlign: 'left', padding: '8px 10px', border: '1px solid var(--border)', color: 'var(--text-primary)', fontWeight: 500 }}>Key</th>
              <th style={{ textAlign: 'center', padding: '8px 10px', border: '1px solid var(--border)', color: 'var(--text-primary)', fontWeight: 500, width: 80 }}>Status</th>
              <th style={{ textAlign: 'center', padding: '8px 10px', border: '1px solid var(--border)', color: 'var(--text-primary)', fontWeight: 500, width: 100 }}>Actions</th>
            </tr>
          </thead>
          <tbody>
            {connected.map(c => {
              const providerDef = PROVIDERS.find(p => p.id === c.providerId);
              const testResult = testResults[c.providerId];
              return (
                <tr key={c.providerId}>
                  <td style={{ padding: '8px 10px', border: '1px solid var(--border)', color: 'var(--text-primary)' }}>
                    {editingLabel === c.providerId ? (
                      <input
                        autoFocus
                        value={editLabelValue}
                        onChange={e => setEditLabelValue(e.target.value)}
                        onBlur={() => handleRenameSubmit(c.providerId)}
                        onKeyDown={e => { if (e.key === 'Enter') handleRenameSubmit(c.providerId); if (e.key === 'Escape') setEditingLabel(null); }}
                        style={{
                          width: '100%', padding: '2px 4px', fontSize: 12,
                          background: 'var(--bg-surface)', border: '1px solid var(--accent)',
                          borderRadius: 3, color: 'var(--text-primary)', outline: 'none',
                        }}
                      />
                    ) : (
                      <span>{c.label}</span>
                    )}
                  </td>
                  <td style={{ padding: '8px 10px', border: '1px solid var(--border)', color: 'var(--text-muted)', fontSize: 11 }}>
                    {providerDef?.label ?? c.providerId}
                  </td>
                  <td style={{ padding: '8px 10px', border: '1px solid var(--border)', fontFamily: 'JetBrains Mono, monospace', fontSize: 11, color: 'var(--text-muted)' }}>
                    {revealedId === c.providerId && c.maskedKey ? c.maskedKey : '••••••••'}
                  </td>
                  <td style={{ padding: '8px 10px', border: '1px solid var(--border)', textAlign: 'center' }}>
                    {testingId === c.providerId ? (
                      <Loader size={12} style={{ animation: 'spin 1s linear infinite' }} color="var(--text-muted)" />
                    ) : testResult === true ? (
                      <span style={{ fontSize: 11, color: 'var(--success)', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 3 }}>
                        <Check size={11} /> Valid
                      </span>
                    ) : testResult === false ? (
                      <span style={{ fontSize: 11, color: 'var(--error)', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 3 }}>
                        <AlertCircle size={11} /> Invalid
                      </span>
                    ) : (
                      <span style={{ fontSize: 11, color: 'var(--success)', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 3 }}>
                        <Check size={11} /> Connected
                      </span>
                    )}
                  </td>
                  <td style={{ padding: '8px 10px', border: '1px solid var(--border)', textAlign: 'center' }}>
                    <div style={{ display: 'flex', gap: 4, justifyContent: 'center' }}>
                      <button
                        onClick={() => {
                          if (revealedId === c.providerId) {
                            setRevealedId(null);
                          } else {
                            setRevealedId(c.providerId);
                          }
                        }}
                        title="Show last 3 characters"
                        style={{
                          background: 'none', border: '1px solid var(--border)', borderRadius: 3,
                          padding: '3px 5px', cursor: 'pointer', color: 'var(--text-muted)',
                          display: 'flex', alignItems: 'center',
                        }}
                      >
                        <Eye size={12} />
                      </button>
                      <button
                        onClick={() => handleTest(c.providerId)}
                        disabled={testingId === c.providerId}
                        title="Test connection"
                        style={{
                          background: 'none', border: '1px solid var(--border)', borderRadius: 3,
                          padding: '3px 5px', cursor: 'pointer', color: 'var(--text-muted)',
                          display: 'flex', alignItems: 'center',
                        }}
                      >
                        <RefreshCw size={12} />
                      </button>
                      <button
                        onClick={() => { setEditingLabel(c.providerId); setEditLabelValue(c.label); }}
                        title="Rename"
                        style={{
                          background: 'none', border: '1px solid var(--border)', borderRadius: 3,
                          padding: '3px 5px', cursor: 'pointer', color: 'var(--text-muted)',
                          display: 'flex', alignItems: 'center',
                        }}
                      >
                        <Pencil size={12} />
                      </button>
                      <button
                        onClick={() => handleDelete(c.providerId)}
                        title="Delete"
                        style={{
                          background: 'none', border: '1px solid var(--error)', borderRadius: 3,
                          padding: '3px 5px', cursor: 'pointer', color: 'var(--error)',
                          display: 'flex', alignItems: 'center',
                        }}
                      >
                        <Trash2 size={12} />
                      </button>
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}

      {/* Connect LLM button */}
      {!showConnect && (
        <button
          onClick={() => setShowConnect(true)}
          style={{
            padding: '8px 16px', background: 'var(--accent)', border: 'none',
            borderRadius: 4, color: '#fff', fontSize: 12, cursor: 'pointer',
            display: 'flex', alignItems: 'center', gap: 6,
          }}
        >
          <Plus size={14} /> Connect LLM
        </button>
      )}

      {/* Connect provider form */}
      {showConnect && (
        <ConnectProviderForm
          profileId={profileId}
          excludeProviders={connectedIds}
          onDone={() => { setShowConnect(false); loadConnected(); }}
          onCancel={() => setShowConnect(false)}
        />
      )}
    </div>
  );
}

function ConnectProviderForm({ profileId, excludeProviders, onDone, onCancel }: {
  profileId: number | null;
  excludeProviders: Set<string>;
  onDone: () => void;
  onCancel: () => void;
}) {
  const [selectedProvider, setSelectedProvider] = useState('');
  const [label, setLabel] = useState('');
  const [credType, setCredType] = useState<CredType>('api_key');
  const [apiKey, setApiKey] = useState('');
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);

  const availableProviders = PROVIDERS.filter(p => !excludeProviders.has(p.id));
  const providerDef = PROVIDERS.find(p => p.id === selectedProvider);

  const handleProviderChange = (id: string) => {
    setSelectedProvider(id);
    const def = PROVIDERS.find(p => p.id === id);
    if (def) {
      setLabel(def.label);
      setCredType(def.credTypes[0].id);
    }
    setApiKey('');
    setTestResult(null);
    setError(null);
  };

  const handleTest = async () => {
    if (!apiKey.trim() || !selectedProvider) return;
    setTesting(true);
    setTestResult(null);
    try {
      const ok = await api.keyTestWithValue(selectedProvider, apiKey.trim());
      setTestResult(ok);
    } catch {
      setTestResult(false);
    }
    setTesting(false);
  };

  const handleSave = async () => {
    if (!apiKey.trim() || !selectedProvider) return;
    setSaving(true);
    setError(null);
    try {
      if (profileId != null) {
        await api.credSaveProfile(selectedProvider, credType, apiKey.trim(), profileId);
      } else {
        await api.credSave(selectedProvider, credType, apiKey.trim());
      }
      if (credType === 'api_key') {
        await api.keySave(selectedProvider, apiKey.trim());
      }
      if (label.trim()) {
        // settingsGet resolves to null (not a rejection) when the setting
        // has never been saved before — e.g. the very first LLM ever
        // connected in a profile — so `?? {}` is needed in addition to the
        // `.catch()` fallback, or `labels` stays null and the assignment
        // below throws.
        const existing = await api.settingsGet('provider_labels').catch(() => null) as Record<string, string> | null;
        const labels = existing ?? {};
        labels[selectedProvider] = label.trim();
        await api.settingsSet('provider_labels', labels);
      }
      onDone();
    } catch (e) {
      setError(api.errorMessage(e));
      setSaving(false);
    }
  };

  const currentCredDef = providerDef?.credTypes.find(ct => ct.id === credType) ?? providerDef?.credTypes[0];

  return (
    <div style={{
      padding: '16px', background: 'var(--bg-app)', border: '1px solid var(--accent)',
      borderRadius: 6, marginTop: 8,
    }}>
      <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-primary)', marginBottom: 12 }}>
        Connect an LLM
      </div>

      {/* Provider selector */}
      <div style={{ marginBottom: 12 }}>
        <label style={{ fontSize: 11, color: 'var(--text-muted)', display: 'block', marginBottom: 4 }}>
          Provider
        </label>
        <select
          value={selectedProvider}
          onChange={e => handleProviderChange(e.target.value)}
          style={{
            width: '100%', padding: '7px 10px', fontSize: 12,
            background: 'var(--bg-surface)', border: '1px solid var(--border)',
            borderRadius: 4, color: 'var(--text-primary)', outline: 'none',
          }}
        >
          <option value="">Select a provider...</option>
          {availableProviders.map(p => (
            <option key={p.id} value={p.id}>{p.label}</option>
          ))}
        </select>
      </div>

      {selectedProvider && providerDef && (
        <>
          {/* Label */}
          <div style={{ marginBottom: 12 }}>
            <label style={{ fontSize: 11, color: 'var(--text-muted)', display: 'block', marginBottom: 4 }}>
              Name (how you want to identify this connection)
            </label>
            <input
              value={label}
              onChange={e => setLabel(e.target.value)}
              placeholder={providerDef.label}
              style={{
                width: '100%', padding: '7px 10px', fontSize: 12,
                background: 'var(--bg-surface)', border: '1px solid var(--border)',
                borderRadius: 4, color: 'var(--text-primary)', outline: 'none',
                boxSizing: 'border-box',
              }}
            />
          </div>

          {/* Cred type selector if multiple */}
          {providerDef.credTypes.length > 1 && (
            <div style={{ marginBottom: 12 }}>
              <label style={{ fontSize: 11, color: 'var(--text-muted)', display: 'block', marginBottom: 4 }}>
                Auth method
              </label>
              <div style={{ display: 'flex', gap: 4 }}>
                {providerDef.credTypes.map(ct => (
                  <button
                    key={ct.id}
                    onClick={() => { setCredType(ct.id); setApiKey(''); setTestResult(null); }}
                    style={{
                      padding: '4px 10px', borderRadius: 4, fontSize: 11,
                      background: credType === ct.id ? 'var(--accent)' : 'var(--overlay)',
                      color: credType === ct.id ? '#fff' : 'var(--text-muted)',
                      border: 'none', cursor: 'pointer',
                    }}
                  >
                    {ct.label}
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* API key input */}
          <div style={{ marginBottom: 12 }}>
            <label style={{ fontSize: 11, color: 'var(--text-muted)', display: 'block', marginBottom: 4 }}>
              {currentCredDef?.label ?? 'API Key'}
            </label>
            {currentCredDef?.multiline ? (
              <textarea
                value={apiKey}
                onChange={e => { setApiKey(e.target.value); setTestResult(null); }}
                placeholder={currentCredDef.placeholder}
                rows={3}
                style={{
                  width: '100%', padding: '7px 10px',
                  background: 'var(--bg-surface)', border: '1px solid var(--border)',
                  borderRadius: 4, fontSize: 12, color: 'var(--text-primary)',
                  outline: 'none', resize: 'vertical', boxSizing: 'border-box',
                  fontFamily: 'JetBrains Mono, monospace',
                }}
              />
            ) : (
              <input
                type="password"
                value={apiKey}
                onChange={e => { setApiKey(e.target.value); setTestResult(null); }}
                placeholder={currentCredDef?.placeholder ?? ''}
                style={{
                  width: '100%', padding: '7px 10px',
                  background: 'var(--bg-surface)', border: '1px solid var(--border)',
                  borderRadius: 4, fontSize: 12, color: 'var(--text-primary)',
                  outline: 'none', boxSizing: 'border-box',
                  fontFamily: 'JetBrains Mono, monospace',
                }}
              />
            )}
          </div>

          {error && <div style={{ marginBottom: 8, fontSize: 11, color: 'var(--error)' }}>{error}</div>}

          {testResult === true && (
            <div style={{ marginBottom: 8, fontSize: 11, color: 'var(--success)', display: 'flex', alignItems: 'center', gap: 4 }}>
              <Check size={12} /> Key is valid
            </div>
          )}
          {testResult === false && (
            <div style={{ marginBottom: 8, fontSize: 11, color: 'var(--error)', display: 'flex', alignItems: 'center', gap: 4 }}>
              <AlertCircle size={12} /> Key is invalid or provider unreachable
            </div>
          )}

          {/* Actions */}
          <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
            {providerDef.keyUrl && (
              <a
                href={providerDef.keyUrl}
                target="_blank"
                rel="noopener noreferrer"
                style={{
                  padding: '5px 10px', fontSize: 12, color: 'var(--accent)',
                  textDecoration: 'none', display: 'flex', alignItems: 'center', gap: 4,
                  border: '1px solid var(--accent)', borderRadius: 4,
                }}
              >
                <ExternalLink size={11} /> Get API Key
              </a>
            )}
            <button
              onClick={handleTest}
              disabled={!apiKey.trim() || testing}
              style={{
                padding: '5px 12px', background: 'none', border: '1px solid var(--border)',
                borderRadius: 4, color: 'var(--text-muted)', fontSize: 12, cursor: 'pointer',
                opacity: !apiKey.trim() ? 0.5 : 1,
              }}
            >
              {testing ? 'Testing...' : 'Test'}
            </button>
            <button
              onClick={handleSave}
              disabled={!apiKey.trim() || saving}
              style={{
                padding: '5px 12px', background: 'var(--accent)', border: 'none',
                borderRadius: 4, color: '#fff', fontSize: 12, cursor: 'pointer',
                opacity: !apiKey.trim() ? 0.5 : 1,
              }}
            >
              {saving ? 'Saving...' : 'Save'}
            </button>
            <button
              onClick={onCancel}
              style={{
                padding: '5px 12px', background: 'none', border: '1px solid var(--border)',
                borderRadius: 4, color: 'var(--text-muted)', fontSize: 12, cursor: 'pointer',
              }}
            >
              Cancel
            </button>
          </div>
        </>
      )}
    </div>
  );
}

// ── Skills Tab ──────────────────────────────────────────────────────────

async function fetchSkillsFor(profileRoot: string | undefined): Promise<{ installed: SkillInfo[]; defaults: SkillInfo[] }> {
  const [installed, defaults] = await Promise.all([
    profileRoot ? api.skillList(profileRoot).catch(() => [] as SkillInfo[]) : Promise.resolve([] as SkillInfo[]),
    api.defaultSkillList().catch(() => [] as SkillInfo[]),
  ]);
  return { installed, defaults };
}

function SkillsTab() {
  const activeProfile = useProfileStore(s => s.active);
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [defaultSkills, setDefaultSkills] = useState<SkillInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetchSkillsFor(activeProfile?.root_path).then(({ installed, defaults }) => {
      if (cancelled) return;
      setSkills(installed);
      setDefaultSkills(defaults);
      setLoading(false);
    });
    return () => { cancelled = true; };
  }, [activeProfile]);

  const installedNames = new Set(skills.map(s => s.name));
  const suggestions = defaultSkills.filter(s => !installedNames.has(s.name));

  const handleInstall = async (name: string) => {
    if (!activeProfile) return;
    setInstalling(name);
    try {
      await api.defaultSkillInstall(activeProfile.root_path, name);
      const { installed, defaults } = await fetchSkillsFor(activeProfile.root_path);
      setSkills(installed);
      setDefaultSkills(defaults);
    } catch (err: unknown) {
      console.error('Failed to install default skill', err);
    } finally {
      setInstalling(null);
    }
  };

  return (
    <div>
      <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--text-primary)', marginBottom: 6 }}>
        Atelier skills
      </div>
      <div style={{ marginBottom: 12, fontSize: 11, color: 'var(--text-muted)', lineHeight: 1.5 }}>
        Skills are Markdown files placed in <code>skills/</code> inside your active profile's folder
        ({activeProfile?.root_path ?? 'no active profile'}). Each file's name becomes the skill's
        name, and its full contents are automatically prepended as instructions to every AI request
        for this profile — no restart needed, just add or edit a file and start a new message.
      </div>
      <button
        onClick={() => openShell('https://skills.open-atelier.app').catch((err: unknown) => console.error('Failed to open skill store', err))}
        style={{
          display: 'flex', alignItems: 'center', gap: 6, marginBottom: 12,
          padding: '6px 10px', background: 'var(--overlay)', border: '1px solid var(--border)',
          borderRadius: 4, color: 'var(--accent)', fontSize: 12, cursor: 'pointer',
        }}
      >
        <ExternalLink size={12} />
        Browse the skill store (skills.open-atelier.app)
      </button>
      {loading && <div style={{ fontSize: 12, color: 'var(--text-muted)' }}>Loading...</div>}
      {!loading && skills.length === 0 && (
        <div style={{ fontSize: 12, color: 'var(--text-muted)' }}>
          No skills found. Create a <code>.md</code> file under <code>skills/</code> in your profile folder to add one.
        </div>
      )}
      {!loading && skills.map(s => (
        <div key={s.name} style={{ padding: '8px 10px', borderRadius: 4, background: 'var(--overlay)', marginBottom: 6 }}>
          <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-primary)' }}>{s.name}</div>
          <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 2 }}>{s.preview}</div>
        </div>
      ))}

      {!loading && suggestions.length > 0 && (
        <>
          <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--text-primary)', margin: '18px 0 6px' }}>
            Recommended skills
          </div>
          <div style={{ marginBottom: 10, fontSize: 11, color: 'var(--text-muted)', lineHeight: 1.5 }}>
            Bundled with Atelier. Adding one copies it into your profile's <code>skills/</code> folder,
            where you can edit or remove it like any other skill.
          </div>
          {suggestions.map(s => (
            <div key={s.name} style={{
              display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 10,
              padding: '8px 10px', borderRadius: 4, background: 'var(--overlay)', marginBottom: 6,
            }}>
              <div style={{ minWidth: 0 }}>
                <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-primary)' }}>{s.name}</div>
                <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 2 }}>{s.preview}</div>
              </div>
              <button
                onClick={() => handleInstall(s.name)}
                disabled={installing === s.name || !activeProfile}
                style={{
                  flexShrink: 0, padding: '3px 10px', borderRadius: 4, fontSize: 11, border: 'none',
                  cursor: installing === s.name ? 'default' : 'pointer',
                  background: 'var(--accent)', color: '#fff',
                }}
              >
                {installing === s.name ? 'Adding…' : 'Add'}
              </button>
            </div>
          ))}
        </>
      )}
    </div>
  );
}

// ── Connectors Tab ───────────────────────────────────────────────────────

// A connector is "enabled" the same way an LLM provider is "available": by
// whether a credential is stored for it, not a separate on/off flag. This
// mirrors the existing provider-availability pattern instead of adding a
// parallel settings concept.
function ConnectorsTab({ profileId }: { profileId: number | null }) {
  return (
    <div>
      <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--text-primary)', marginBottom: 6 }}>
        Connectors
      </div>
      <div style={{
        marginBottom: 14, fontSize: 11, color: 'var(--text-muted)', lineHeight: 1.5,
        padding: '8px 10px', background: 'var(--overlay)', borderRadius: 4,
      }}>
        Off by default. Enabling a connector lets the assistant send data (file paths, content) directly
        to that service using your own token — this is separate from, and in addition to, the local-only
        file operations Atelier normally uses.
      </div>

      <ConnectorCard
        profileId={profileId}
        provider="github"
        label="GitHub"
        placeholder="ghp_…"
        description={
          <>
            Lets the assistant read files from your GitHub repositories via GITHUB_READ. Connect your
            account directly, or paste a{' '}
            <a
              href="#"
              onClick={e => { e.preventDefault(); openShell('https://github.com/settings/tokens').catch(() => {}); }}
              style={{ color: 'var(--accent)' }}
            >
              personal access token
            </a>{' '}
            yourself (or leave both empty, for public repos only — subject to GitHub's low unauthenticated rate limit).
          </>
        }
        onTest={async token => `Connected as ${await api.connectorGithubTest(token)}`}
        renderExtra={reload => <GithubConnectButton profileId={profileId} onConnected={reload} />}
      />

      <ConnectorCard
        profileId={profileId}
        provider="notion"
        label="Notion"
        placeholder="ntn_… / secret_…"
        description={
          <>
            Lets the assistant read Notion pages via NOTION_READ. Needs an{' '}
            <a
              href="#"
              onClick={e => { e.preventDefault(); openShell('https://www.notion.so/my-integrations').catch(() => {}); }}
              style={{ color: 'var(--accent)' }}
            >
              internal integration token
            </a>
            {' '}— and Notion requires sharing that integration onto each individual page it should read
            (there's no "read everything" scope).
          </>
        }
        onTest={async token => `Connected as ${await api.connectorNotionTest(token)}`}
      />

      <ConnectorCard
        profileId={profileId}
        provider="slack"
        label="Slack"
        placeholder="xoxb-…"
        description={
          <>
            Lets the assistant read recent channel messages via SLACK_READ. Needs a bot token from a{' '}
            <a
              href="#"
              onClick={e => { e.preventDefault(); openShell('https://api.slack.com/apps').catch(() => {}); }}
              style={{ color: 'var(--accent)' }}
            >
              Slack app
            </a>
            {' '}with the channels:history and channels:read scopes — the bot must also be invited into
            each channel it should read.
          </>
        }
        onTest={async token => `Connected to ${await api.connectorSlackTest(token)}`}
      />

      <ConnectorCard
        profileId={profileId}
        provider="google_drive"
        label="Google Drive"
        placeholder="AIza…"
        extraCredTypeForEnabled="bearer_token"
        description={
          <>
            Lets the assistant read files via GDRIVE_READ. "Connect with Google" below signs you in
            directly and can read your own private files too. Or paste a{' '}
            <a
              href="#"
              onClick={e => { e.preventDefault(); openShell('https://console.cloud.google.com/apis/credentials').catch(() => {}); }}
              style={{ color: 'var(--accent)' }}
            >
              Google Cloud API key
            </a>
            {' '}instead — a bare key has no user identity, so it only works on files shared as "Anyone
            with the link", and never on native Google Docs/Sheets/Slides (only plain files like .txt/.csv/.md).
          </>
        }
        onTest={async key => await api.connectorGoogleDriveTest(key)}
        renderExtra={reload => <GoogleDriveConnectButton profileId={profileId} onConnected={reload} />}
      />
    </div>
  );
}

function ConnectorCard({
  profileId, provider, label, description, placeholder, onTest, extraCredTypeForEnabled, renderExtra,
}: {
  profileId: number | null;
  provider: string;
  label: string;
  description: ReactNode;
  placeholder: string;
  onTest: (value: string) => Promise<string>;
  // A second credential slot (e.g. "bearer_token" for an OAuth-derived
  // token stored separately from a pasted API key) that also counts
  // toward this card's "Enabled" status — see the Google Drive card,
  // which can be connected via either method.
  extraCredTypeForEnabled?: string;
  // Renders extra UI (e.g. a native "Connect with X" button) between the
  // description and the paste-a-token form. Receives this card's own
  // reload function so a successful native connect can refresh the
  // masked-token display immediately.
  renderExtra?: (reload: () => Promise<void>) => ReactNode;
}) {
  const [maskedToken, setMaskedToken] = useState<string | null>(null);
  const [maskedExtra, setMaskedExtra] = useState<string | null>(null);
  const [tokenDraft, setTokenDraft] = useState('');
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(null);

  const reload = async () => {
    const masked = profileId != null
      ? await api.credGetMasked(provider, 'api_key', profileId).catch(() => null)
      : null;
    setMaskedToken(masked);
    if (extraCredTypeForEnabled) {
      const maskedE = profileId != null
        ? await api.credGetMasked(provider, extraCredTypeForEnabled, profileId).catch(() => null)
        : null;
      setMaskedExtra(maskedE);
    }
    setLoading(false);
  };

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const masked = profileId != null
        ? await api.credGetMasked(provider, 'api_key', profileId).catch(() => null)
        : null;
      const maskedE = (extraCredTypeForEnabled && profileId != null)
        ? await api.credGetMasked(provider, extraCredTypeForEnabled, profileId).catch(() => null)
        : null;
      if (!cancelled) {
        setMaskedToken(masked);
        setMaskedExtra(maskedE);
        setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, [profileId, provider, extraCredTypeForEnabled]);

  const handleSave = async () => {
    if (!profileId || !tokenDraft.trim() || saving) return;
    setSaving(true);
    setTestResult(null);
    try {
      await api.credSaveProfile(provider, 'api_key', tokenDraft.trim(), profileId);
      setTokenDraft('');
      await reload();
    } catch (e) {
      console.error(`Failed to save ${label} token`, e);
    } finally {
      setSaving(false);
    }
  };

  const handleRemove = async () => {
    if (!profileId) return;
    try {
      await api.credDeleteProfile(provider, 'api_key', profileId);
      if (extraCredTypeForEnabled) {
        await api.credDeleteProfile(provider, extraCredTypeForEnabled, profileId);
        // The OAuth refresh token (if any) rides along with the access
        // token slot above — clear it too so a stale one can't be reused.
        await api.credDeleteProfile(provider, 'oauth_refresh_token', profileId).catch(() => {});
      }
      setTestResult(null);
      await reload();
    } catch (e) {
      console.error(`Failed to remove ${label} token`, e);
    }
  };

  const handleTest = async () => {
    if (!profileId || testing) return;
    setTesting(true);
    setTestResult(null);
    try {
      const token = tokenDraft.trim() || await api.credGetProfile(provider, 'api_key', profileId).catch(() => null) || '';
      if (!token) {
        setTestResult({ ok: false, message: 'Nothing to test — paste a value first.' });
        return;
      }
      const message = await onTest(token);
      setTestResult({ ok: true, message });
    } catch (e: unknown) {
      setTestResult({ ok: false, message: api.errorMessage(e) });
    } finally {
      setTesting(false);
    }
  };

  const enabled = !!maskedToken || !!maskedExtra;

  return (
    <div style={{
      border: '1px solid var(--border)', borderRadius: 6, padding: '12px 14px',
      background: 'var(--bg-surface)', marginBottom: 10,
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
        <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-primary)', flex: 1 }}>{label}</span>
        <span style={{
          fontSize: 10, fontWeight: 600, padding: '2px 8px', borderRadius: 10,
          background: enabled ? 'rgba(61, 122, 90, 0.15)' : 'var(--overlay)',
          color: enabled ? 'var(--success)' : 'var(--text-muted)',
        }}>
          {enabled ? 'Enabled' : 'Disabled'}
        </span>
      </div>
      <div style={{ fontSize: 11, color: 'var(--text-muted)', marginBottom: 10 }}>
        {description}
      </div>

      {renderExtra?.(reload)}

      {!loading && maskedExtra && (
        <div style={{ fontSize: 12, color: 'var(--success)', marginBottom: 8, fontFamily: 'JetBrains Mono, monospace' }}>
          Connected: {maskedExtra}
        </div>
      )}

      {!loading && maskedToken && (
        <div style={{ fontSize: 12, color: 'var(--text-muted)', marginBottom: 8, fontFamily: 'JetBrains Mono, monospace' }}>
          Value: {maskedToken}
        </div>
      )}

      {renderExtra && (
        <div style={{ fontSize: 11, color: 'var(--text-muted)', margin: '2px 0 6px' }}>Or paste a token manually:</div>
      )}

      <div style={{ display: 'flex', gap: 6, marginBottom: 8 }}>
        <input
          type="password"
          value={tokenDraft}
          onChange={e => setTokenDraft(e.target.value)}
          placeholder={enabled ? 'Paste a new value to replace it…' : placeholder}
          style={{
            flex: 1, padding: '6px 10px', fontSize: 12,
            background: 'var(--bg-app)', border: '1px solid var(--border)',
            borderRadius: 4, color: 'var(--text-primary)', outline: 'none',
          }}
        />
        <button
          onClick={handleSave}
          disabled={!tokenDraft.trim() || saving}
          style={{
            padding: '6px 12px', borderRadius: 4, fontSize: 12, border: 'none',
            background: 'var(--accent)', color: '#fff', cursor: 'pointer',
            opacity: (!tokenDraft.trim() || saving) ? 0.5 : 1,
          }}
        >
          {saving ? 'Saving…' : 'Save'}
        </button>
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <button
          onClick={handleTest}
          disabled={testing}
          style={{
            padding: '4px 10px', borderRadius: 4, fontSize: 12, border: '1px solid var(--border)',
            background: 'none', color: 'var(--text-primary)', cursor: testing ? 'default' : 'pointer',
          }}
        >
          {testing ? 'Testing…' : 'Test connection'}
        </button>
        {enabled && (
          <button
            onClick={handleRemove}
            style={{
              padding: '4px 10px', borderRadius: 4, fontSize: 12, border: 'none',
              background: 'none', color: 'var(--error)', cursor: 'pointer',
            }}
          >
            Remove
          </button>
        )}
        {testResult && (
          <span style={{ fontSize: 11, color: testResult.ok ? 'var(--success)' : 'var(--error)' }}>
            {testResult.message}
          </span>
        )}
      </div>
    </div>
  );
}

// ── Usage Tab ──────────────────────────────────────────────────────────

function UsageTab() {
  const workspaces = useWorkspaceStore(s => s.workspaces);
  const [rows, setRows] = useState<{
    conversation: Conversation;
    inputTokens: number;
    outputTokens: number;
    cost: number;
  }[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const allRows: typeof rows = [];
      for (const ws of workspaces) {
        const convs = await api.conversationList(ws.id).catch(() => [] as Conversation[]);
        for (const conv of convs) {
          const result = await api.conversationGet(conv.id).catch(() => null);
          if (!result) continue;
          const messages: Message[] = result.messages;
          const inputTokens = messages.reduce((sum, m) => sum + (m.input_tokens ?? 0), 0);
          const outputTokens = messages.reduce((sum, m) => sum + (m.output_tokens ?? 0), 0);
          if (inputTokens === 0 && outputTokens === 0) continue;
          const cost = estimateCost(conv.provider, inputTokens, outputTokens);
          allRows.push({ conversation: conv, inputTokens, outputTokens, cost });
        }
      }
      if (!cancelled) {
        setRows(allRows);
        setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, [workspaces]);

  const totalCost = rows.reduce((sum, r) => sum + r.cost, 0);
  const totalTokens = rows.reduce((sum, r) => sum + r.inputTokens + r.outputTokens, 0);

  return (
    <div>
      <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--text-primary)', marginBottom: 6 }}>
        Estimated usage &amp; cost
      </div>
      <div style={{ marginBottom: 14, padding: '10px 14px', background: 'var(--overlay)', borderRadius: 6, fontSize: 12, color: 'var(--text-muted)', lineHeight: 1.6 }}>
        Approximate only — based on hardcoded public list prices captured at one point in time. Check each provider's pricing page for current rates. Local providers (Ollama) are not billed and shown as $0.
      </div>

      {loading && <div style={{ fontSize: 12, color: 'var(--text-muted)' }}>Loading...</div>}

      {!loading && rows.length === 0 && (
        <div style={{ fontSize: 12, color: 'var(--text-muted)' }}>No token usage recorded yet for conversations in this profile's workspaces.</div>
      )}

      {!loading && rows.length > 0 && (
        <>
          <div style={{ overflow: 'auto', marginBottom: 14 }}>
            <table style={{ borderCollapse: 'collapse', width: '100%', fontSize: 12 }}>
              <thead>
                <tr style={{ background: 'var(--overlay)' }}>
                  <th style={{ textAlign: 'left', padding: '6px 10px', border: '1px solid var(--border)', color: 'var(--text-primary)' }}>Conversation</th>
                  <th style={{ textAlign: 'left', padding: '6px 10px', border: '1px solid var(--border)', color: 'var(--text-primary)' }}>Provider / Model</th>
                  <th style={{ textAlign: 'right', padding: '6px 10px', border: '1px solid var(--border)', color: 'var(--text-primary)' }}>Est. tokens</th>
                  <th style={{ textAlign: 'right', padding: '6px 10px', border: '1px solid var(--border)', color: 'var(--text-primary)' }}>Est. cost</th>
                </tr>
              </thead>
              <tbody>
                {rows.map(r => (
                  <tr key={r.conversation.id}>
                    <td style={{ padding: '6px 10px', border: '1px solid var(--border)', color: 'var(--text-primary)' }}>{r.conversation.title}</td>
                    <td style={{ padding: '6px 10px', border: '1px solid var(--border)', color: 'var(--text-muted)' }}>
                      {r.conversation.provider ?? '—'} {r.conversation.model ? `/ ${r.conversation.model}` : ''}
                    </td>
                    <td style={{ padding: '6px 10px', border: '1px solid var(--border)', textAlign: 'right', color: 'var(--text-primary)' }}>
                      {(r.inputTokens + r.outputTokens).toLocaleString()}
                    </td>
                    <td style={{ padding: '6px 10px', border: '1px solid var(--border)', textAlign: 'right', color: 'var(--text-primary)' }}>
                      ${r.cost.toFixed(4)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div style={{ display: 'flex', gap: 16, fontSize: 12, color: 'var(--text-muted)' }}>
            <div><strong style={{ color: 'var(--text-primary)' }}>{totalTokens.toLocaleString()}</strong> total est. tokens</div>
            <div><strong style={{ color: 'var(--text-primary)' }}>${totalCost.toFixed(4)}</strong> total est. cost (active profile's workspaces)</div>
          </div>
        </>
      )}
    </div>
  );
}

// ── About Tab with styled factory reset confirmation ─────────────────────

function AboutTab() {
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [resetConfirmText, setResetConfirmText] = useState('');
  const [updateStatus, setUpdateStatus] = useState<'idle' | 'checking' | 'available' | 'downloading' | 'up-to-date' | 'error'>('idle');
  const [updateError, setUpdateError] = useState('');
  const [appVersion, setAppVersion] = useState('');

  useEffect(() => {
    import('@tauri-apps/api/app').then(({ getVersion }) => getVersion()).then(setAppVersion).catch(() => {});
  }, []);

  const handleCheckUpdates = async () => {
    setUpdateError('');
    const result = await checkForUpdates(setUpdateStatus);
    switch (result.status) {
      case 'up-to-date':
        setUpdateStatus('up-to-date');
        break;
      case 'declined':
        setUpdateStatus('idle');
        break;
      case 'installed':
        // App relaunches itself momentarily; leave the status as-is.
        break;
      case 'error':
        setUpdateStatus('error');
        if (result.message.includes('release JSON') || result.message.includes('endpoint') || result.message.includes('pubkey')) {
          setUpdateError('Auto-update is not available yet. Check open-atelier.app/update for the latest release.');
        } else {
          setUpdateError(result.message);
        }
        break;
    }
  };

  const handleFactoryReset = async () => {
    try {
      await api.factoryReset();
      window.location.reload();
    } catch (e) {
      console.error('Factory reset failed', e);
    }
  };

  return (
    <div>
      <div style={{ fontSize: 18, fontWeight: 600, color: 'var(--text-primary)', marginBottom: 6 }}>
        Open Atelier
      </div>
      <div style={{ fontSize: 12, color: 'var(--text-muted)', marginBottom: 4 }}>
        Version {appVersion || '…'}
      </div>
      <div style={{ fontSize: 11, color: 'var(--text-muted)', marginBottom: 12 }}>
        &copy; 2025 Atelier. All rights reserved.
      </div>
      <div style={{ fontSize: 13, color: 'var(--text-muted)', lineHeight: 1.6, marginBottom: 12 }}>
        Local-first AI workspace. BYOK. No telemetry.
      </div>
      <div style={{ fontSize: 12, color: 'var(--text-muted)', lineHeight: 1.6, padding: '10px 14px', background: 'var(--overlay)', borderRadius: 6, marginBottom: 16 }}>
        All LLM calls happen on your machine via your API keys.
      </div>
      <div style={{ marginBottom: 24 }}>
        <button
          onClick={handleCheckUpdates}
          disabled={updateStatus === 'checking' || updateStatus === 'downloading'}
          style={{
            padding: '8px 16px', background: 'var(--accent)',
            border: 'none', borderRadius: 4,
            color: '#fff', fontSize: 12, cursor: 'pointer',
            opacity: (updateStatus === 'checking' || updateStatus === 'downloading') ? 0.6 : 1,
          }}
        >
          {updateStatus === 'checking' ? 'Checking…' :
           updateStatus === 'downloading' ? 'Downloading…' :
           'Check for Updates'}
        </button>
        {updateStatus === 'up-to-date' && (
          <span style={{ marginLeft: 10, fontSize: 12, color: 'var(--success)' }}>You're up to date!</span>
        )}
        {updateStatus === 'error' && (
          <span style={{ marginLeft: 10, fontSize: 12, color: 'var(--error)' }}>{updateError || 'Update check failed'}</span>
        )}
      </div>

      <div style={{ borderTop: '1px solid var(--border)', paddingTop: 20 }}>
        <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--text-primary)', marginBottom: 8 }}>
          Advanced
        </div>

        {!showResetConfirm ? (
          <>
            <button
              onClick={() => setShowResetConfirm(true)}
              style={{
                padding: '8px 16px', background: 'none',
                border: '1px solid var(--error)', borderRadius: 4,
                color: 'var(--error)', fontSize: 12, cursor: 'pointer',
              }}
            >
              Factory Reset
            </button>
            <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 6 }}>
              Deletes all app data (database, credentials, preferences). Profile directories on disk are not removed.
            </div>
          </>
        ) : (
          <div style={{
            padding: '16px', background: 'var(--bg-app)', border: '2px solid var(--error)',
            borderRadius: 6,
          }}>
            <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--error)', marginBottom: 8 }}>
              Are you sure?
            </div>
            <div style={{ fontSize: 12, color: 'var(--text-muted)', lineHeight: 1.6, marginBottom: 12 }}>
              This will permanently delete all profiles, projects, conversations, API keys, and preferences.
              Your files on disk will not be deleted. <strong style={{ color: 'var(--error)' }}>This action cannot be undone.</strong>
            </div>
            <div style={{ marginBottom: 12 }}>
              <label style={{ fontSize: 11, color: 'var(--text-muted)', display: 'block', marginBottom: 4 }}>
                Type <strong>RESET</strong> to confirm
              </label>
              <input
                value={resetConfirmText}
                onChange={e => setResetConfirmText(e.target.value)}
                placeholder="RESET"
                autoFocus
                style={{
                  width: 200, padding: '6px 10px', fontSize: 12,
                  background: 'var(--bg-surface)', border: '1px solid var(--border)',
                  borderRadius: 4, color: 'var(--text-primary)', outline: 'none',
                }}
              />
            </div>
            <div style={{ display: 'flex', gap: 8 }}>
              <button
                onClick={handleFactoryReset}
                disabled={resetConfirmText !== 'RESET'}
                style={{
                  padding: '8px 16px', background: resetConfirmText === 'RESET' ? 'var(--error)' : 'var(--overlay)',
                  border: 'none', borderRadius: 4,
                  color: resetConfirmText === 'RESET' ? '#fff' : 'var(--text-muted)',
                  fontSize: 12, cursor: resetConfirmText === 'RESET' ? 'pointer' : 'not-allowed',
                  fontWeight: 500,
                }}
              >
                Factory Reset
              </button>
              <button
                onClick={() => { setShowResetConfirm(false); setResetConfirmText(''); }}
                style={{
                  padding: '8px 16px', background: 'none',
                  border: '1px solid var(--border)', borderRadius: 4,
                  color: 'var(--text-muted)', fontSize: 12, cursor: 'pointer',
                }}
              >
                Cancel
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

// ── Profiles Tab (only shows active profile) ─────────────────────────────

function ProfilesTab() {
  const profiles = useProfileStore(s => s.profiles);
  const activeProfile = useProfileStore(s => s.active);
  const updateProfile = useProfileStore(s => s.update);
  const deleteProfile = useProfileStore(s => s.delete);
  const loadProfiles = useProfileStore(s => s.load);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editName, setEditName] = useState('');

  const handleRename = async (id: number) => {
    if (!editName.trim()) { setEditingId(null); return; }
    await updateProfile(id, { name: editName.trim() });
    setEditingId(null);
  };

  const handleChangeLocation = async (id: number) => {
    const selected = await openDialog({ directory: true, multiple: false });
    if (selected && typeof selected === 'string') {
      await updateProfile(id, { root_path: selected });
    }
  };

  const handleOpenFolder = async (path: string) => {
    try { await api.openPath(path); } catch (e) { console.error(e); }
  };

  const handleDelete = async (p: { id: number; name: string; root_path: string }) => {
    if (profiles.length <= 1) return;
    const confirmed = await confirmDialog(
      `This will permanently delete the profile directory at ${p.root_path} and all its contents. This action cannot be undone.`,
      { title: `Delete profile "${p.name}"?`, kind: 'warning' },
    );
    if (!confirmed) return;
    try {
      await deleteProfile(p.id);
      await loadProfiles();
    } catch (e) {
      console.error('Failed to delete profile', e);
    }
  };

  const displayProfile = activeProfile ?? profiles[0];
  if (!displayProfile) {
    return <div style={{ fontSize: 12, color: 'var(--text-muted)' }}>No profile found.</div>;
  }

  const p = displayProfile;

  return (
    <div>
      <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--text-primary)', marginBottom: 12 }}>
        Current Profile
      </div>
      <div style={{
        padding: '12px 14px', marginBottom: 8, borderRadius: 6,
        background: 'var(--bg-app)',
        border: '1px solid var(--accent)',
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
          <div style={{
            width: 24, height: 24, borderRadius: '50%', background: 'var(--accent)',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            fontSize: 11, fontWeight: 600, color: '#fff', flexShrink: 0,
          }}>
            {p.name[0].toUpperCase()}
          </div>
          {editingId === p.id ? (
            <input
              value={editName}
              onChange={e => setEditName(e.target.value)}
              onBlur={() => handleRename(p.id)}
              onKeyDown={e => { if (e.key === 'Enter') handleRename(p.id); if (e.key === 'Escape') setEditingId(null); }}
              autoFocus
              style={{
                flex: 1, padding: '3px 6px', fontSize: 13, fontWeight: 500,
                background: 'var(--bg-surface)', border: '1px solid var(--border)',
                borderRadius: 4, color: 'var(--text-primary)', outline: 'none',
              }}
            />
          ) : (
            <span style={{ flex: 1, fontSize: 13, fontWeight: 500, color: 'var(--text-primary)' }}>
              {p.name}
            </span>
          )}
          <span style={{ fontSize: 10, color: 'var(--accent)', fontWeight: 500 }}>Active</span>
        </div>
        <div style={{ fontSize: 11, color: 'var(--text-muted)', fontFamily: 'JetBrains Mono, monospace', marginBottom: 8, wordBreak: 'break-all' }}>
          {p.root_path}
        </div>
        <div style={{ display: 'flex', gap: 6 }}>
          <button
            onClick={() => { setEditingId(p.id); setEditName(p.name); }}
            style={{
              padding: '4px 10px', background: 'none', border: '1px solid var(--border)',
              borderRadius: 4, color: 'var(--text-muted)', fontSize: 11, cursor: 'pointer',
              display: 'flex', alignItems: 'center', gap: 4,
            }}
          >
            <Pencil size={11} /> Rename
          </button>
          <button
            onClick={() => handleOpenFolder(p.root_path)}
            style={{
              padding: '4px 10px', background: 'none', border: '1px solid var(--border)',
              borderRadius: 4, color: 'var(--text-muted)', fontSize: 11, cursor: 'pointer',
              display: 'flex', alignItems: 'center', gap: 4,
            }}
          >
            <FolderOpen size={11} /> Open Folder
          </button>
          <button
            onClick={() => handleChangeLocation(p.id)}
            style={{
              padding: '4px 10px', background: 'none', border: '1px solid var(--border)',
              borderRadius: 4, color: 'var(--text-muted)', fontSize: 11, cursor: 'pointer',
            }}
          >
            Change Location
          </button>
          <button
            onClick={() => handleDelete(p)}
            disabled={profiles.length <= 1}
            style={{
              padding: '4px 10px', background: 'none',
              border: `1px solid ${profiles.length <= 1 ? 'var(--border)' : 'var(--error)'}`,
              borderRadius: 4, fontSize: 11, cursor: profiles.length <= 1 ? 'not-allowed' : 'pointer',
              color: profiles.length <= 1 ? 'var(--text-muted)' : 'var(--error)',
              display: 'flex', alignItems: 'center', gap: 4,
              opacity: profiles.length <= 1 ? 0.5 : 1,
            }}
          >
            <Trash2 size={11} /> Delete
          </button>
        </div>
      </div>
    </div>
  );
}
