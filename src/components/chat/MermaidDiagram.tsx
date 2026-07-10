import { useEffect, useId, useState } from 'react';
import mermaid from 'mermaid';

function isDark(): boolean {
  const attr = document.documentElement.getAttribute('data-theme');
  if (attr === 'dark') return true;
  if (attr === 'light') return false;
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

/** Renders a fenced ```mermaid block as a real diagram (flowchart, sequence,
 * etc.) via the mermaid package — the one viewer type here that genuinely
 * needs a real rendering library rather than a hand-rolled component;
 * there's no reasonable way to reimplement Mermaid's own layout engine. */
export function MermaidDiagram({ code }: { code: string }) {
  const reactId = useId().replace(/[^a-zA-Z0-9]/g, '');
  const [svg, setSvg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Adjusting state during render (comparing against the last-seen code)
  // instead of resetting inside the effect below — a synchronous setState
  // at the top of an effect just triggers an extra render for no benefit,
  // same reasoning as the draft-key pattern in ChatInput.
  const [lastCode, setLastCode] = useState(code);
  if (code !== lastCode) {
    setLastCode(code);
    setSvg(null);
    setError(null);
  }

  useEffect(() => {
    let cancelled = false;
    mermaid.initialize({
      startOnLoad: false,
      theme: isDark() ? 'dark' : 'default',
      // Untrusted content (model output) — never let a diagram label
      // execute embedded script/click handlers.
      securityLevel: 'strict',
      // Deliberately not overriding fontFamily to the app's own font: mermaid
      // measures label widths at layout time using whatever font this says,
      // then paints with the same one — pointing this at a font the layout
      // pass didn't measure against clips every label to the wrong box.
    });
    mermaid.render(`mermaid-${reactId}`, code)
      .then(({ svg: rendered }) => { if (!cancelled) setSvg(rendered); })
      .catch((e: unknown) => { if (!cancelled) setError(e instanceof Error ? e.message : String(e)); });
    return () => { cancelled = true; };
  }, [code, reactId]);

  if (error) {
    return (
      <pre style={{
        background: 'var(--bg-surface)', border: '1px solid var(--border)',
        borderRadius: 4, padding: '10px 12px', overflow: 'auto',
        fontFamily: 'JetBrains Mono, monospace', fontSize: 12, color: 'var(--error)',
      }}>
        {code}
      </pre>
    );
  }

  if (!svg) {
    return (
      <div style={{ fontSize: 12, color: 'var(--text-muted)', padding: '8px 0' }}>
        Rendering diagram…
      </div>
    );
  }

  return (
    <div
      style={{ overflowX: 'auto', margin: '4px 0 12px' }}
      dangerouslySetInnerHTML={{ __html: svg }}
    />
  );
}
