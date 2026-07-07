import { Component, type ReactNode } from 'react';

interface Props { children: ReactNode }
interface State { error: Error | null }

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  render() {
    const { error } = this.state;
    if (error) {
      return (
        <div style={{
          display: 'flex', height: '100vh', alignItems: 'center', justifyContent: 'center',
          background: '#F8F5F0', flexDirection: 'column', gap: 16, padding: 40,
        }}>
          <div style={{ fontSize: 18, fontWeight: 600, color: '#1A1814' }}>Something went wrong</div>
          <pre style={{
            background: '#fff', border: '1px solid #E8E3DC', borderRadius: 6,
            padding: '12px 16px', fontSize: 12, fontFamily: 'monospace',
            maxWidth: 600, overflow: 'auto', color: '#C44B3A',
            whiteSpace: 'pre-wrap', wordBreak: 'break-word',
          }}>
            {error.message}
            {error.stack ? '\n\n' + error.stack : ''}
          </pre>
          <button
            onClick={() => this.setState({ error: null })}
            style={{
              padding: '8px 20px', background: '#C17B3E', border: 'none',
              borderRadius: 4, color: '#fff', fontSize: 14, cursor: 'pointer',
            }}
          >
            Try again
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
