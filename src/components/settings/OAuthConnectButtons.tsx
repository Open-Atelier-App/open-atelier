import { useState } from 'react';
import { Github } from 'lucide-react';
import { open as openShell } from '@tauri-apps/plugin-shell';
import * as api from '../../lib/tauri';
import type { DeviceFlowStart } from '../../lib/tauri';

// The two connectors with a real native "Connect" flow instead of
// paste-a-token — see connector_github_oauth_* / connector_google_drive_oauth_*
// in commands::connectors for why only these two (no client secret needed
// to keep confidential, unlike Notion/Slack's OAuth).

export function GithubConnectButton({ profileId, onConnected }: { profileId: number | null; onConnected: () => void }) {
  const [code, setCode] = useState<DeviceFlowStart | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState('');

  const handleConnect = async () => {
    if (!profileId || connecting) return;
    setError('');
    setConnecting(true);
    setCode(null);
    try {
      const start = await api.connectorGithubOauthStart();
      setCode(start);
      await api.connectorGithubOauthFinish(start.device_code, start.interval, start.expires_in, profileId);
      onConnected();
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setConnecting(false);
      setCode(null);
    }
  };

  return (
    <div style={{ marginBottom: 10 }}>
      <button
        onClick={handleConnect}
        disabled={connecting}
        style={{
          display: 'flex', alignItems: 'center', gap: 6, padding: '6px 12px',
          borderRadius: 4, fontSize: 12, border: '1px solid var(--border)',
          background: 'var(--overlay)', color: 'var(--text-primary)',
          cursor: connecting ? 'default' : 'pointer', opacity: connecting ? 0.7 : 1,
        }}
      >
        <Github size={13} />
        {connecting ? 'Waiting for you to approve…' : 'Connect with GitHub'}
      </button>
      {code && (
        <div style={{ fontSize: 12, padding: '8px 10px', marginTop: 6, background: 'var(--overlay)', borderRadius: 4 }}>
          <div style={{ color: 'var(--text-muted)' }}>
            Enter this code at{' '}
            <a
              href="#"
              onClick={e => { e.preventDefault(); openShell(code.verification_uri).catch(() => {}); }}
              style={{ color: 'var(--accent)' }}
            >
              {code.verification_uri}
            </a>:
          </div>
          <div style={{ fontFamily: 'JetBrains Mono, monospace', fontSize: 16, fontWeight: 700, letterSpacing: 2, margin: '4px 0', color: 'var(--text-primary)' }}>
            {code.user_code}
          </div>
        </div>
      )}
      {error && <div style={{ color: 'var(--error)', fontSize: 11, marginTop: 4 }}>{error}</div>}
    </div>
  );
}

export function GoogleDriveConnectButton({ profileId, onConnected }: { profileId: number | null; onConnected: () => void }) {
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState('');

  const handleConnect = async () => {
    if (!profileId || connecting) return;
    setError('');
    setConnecting(true);
    try {
      await api.connectorGoogleDriveOauthConnect(profileId);
      onConnected();
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setConnecting(false);
    }
  };

  return (
    <div style={{ marginBottom: 10 }}>
      <button
        onClick={handleConnect}
        disabled={connecting}
        style={{
          padding: '6px 12px', borderRadius: 4, fontSize: 12, border: '1px solid var(--border)',
          background: 'var(--overlay)', color: 'var(--text-primary)',
          cursor: connecting ? 'default' : 'pointer', opacity: connecting ? 0.7 : 1,
        }}
      >
        {connecting ? 'Waiting for you to finish signing in…' : 'Connect with Google (full Drive access)'}
      </button>
      {error && <div style={{ color: 'var(--error)', fontSize: 11, marginTop: 4 }}>{error}</div>}
    </div>
  );
}
