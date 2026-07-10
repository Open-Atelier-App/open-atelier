import { useRef } from 'react';

/**
 * A thin draggable strip that reports raw pointer-movement deltas via
 * `onDrag` — callers decide the sign (a handle on a panel's right edge
 * grows the panel by +deltaX; a handle on a panel's left edge, like
 * RightBar's or the file viewer's, grows it by -deltaX).
 */
export function ResizeHandle({ onDrag }: { onDrag: (deltaX: number) => void }) {
  const lastX = useRef(0);

  const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    lastX.current = e.clientX;
    const target = e.currentTarget;
    target.setPointerCapture(e.pointerId);

    const handleMove = (moveEvent: PointerEvent) => {
      const deltaX = moveEvent.clientX - lastX.current;
      lastX.current = moveEvent.clientX;
      onDrag(deltaX);
    };
    const handleUp = () => {
      target.removeEventListener('pointermove', handleMove);
      target.removeEventListener('pointerup', handleUp);
    };
    target.addEventListener('pointermove', handleMove);
    target.addEventListener('pointerup', handleUp);
  };

  return (
    <div
      onPointerDown={handlePointerDown}
      title="Drag to resize"
      style={{
        width: 5, flexShrink: 0, cursor: 'col-resize',
        background: 'transparent',
      }}
      onMouseEnter={e => { (e.currentTarget as HTMLDivElement).style.background = 'var(--accent)'; }}
      onMouseLeave={e => { (e.currentTarget as HTMLDivElement).style.background = 'transparent'; }}
    />
  );
}
