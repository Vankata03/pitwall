import { useEffect, useRef, type ReactNode } from 'react';
import { useEscape } from './hooks';

/** A modal panel: focus moves in on open, Tab stays inside, Escape closes, focus goes back. */
export function Dialog({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  const panel = useRef<HTMLDivElement>(null);
  useEscape(true, onClose);

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const first = panel.current?.querySelector<HTMLElement>('select, input, button, [href], [tabindex]');
    (first ?? panel.current)?.focus();
    return () => previous?.focus?.();
  }, []);

  const trap = (e: React.KeyboardEvent) => {
    if (e.key !== 'Tab' || !panel.current) return;
    const items = [...panel.current.querySelectorAll<HTMLElement>('select, input, button:not([disabled]), [href], [tabindex]:not([tabindex="-1"])')];
    if (items.length === 0) return;
    const first = items[0]!;
    const last = items[items.length - 1]!;
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  };

  return (
    <div className="pw-scrim" onPointerDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={panel} className="pw-dialog" role="dialog" aria-modal="true" aria-label={title} tabIndex={-1} onKeyDown={trap}>
        <h2 className="pw-dialog-title">{title}</h2>
        {children}
      </div>
    </div>
  );
}
