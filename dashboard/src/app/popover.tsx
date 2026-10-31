import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { useEscape } from './hooks';
import { ChevronIcon } from './icons';

export interface PopoverProps {
  /** Button text, which also names the panel. */
  label: ReactNode;
  /** Accessible name of the panel; defaults to nothing extra when `label` is text. */
  panelLabel: string;
  disabled?: boolean;
  className?: string;
  children: (close: () => void) => ReactNode;
}

/**
 * A button that opens a panel under it. Keyboard: Enter or Space opens, Escape closes and
 * returns focus to the button, Tab walks the panel. Pressing outside closes it.
 */
export function Popover({ label, panelLabel, disabled, className, children }: PopoverProps) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const id = useId();

  const close = () => setOpen(false);
  useEscape(open, () => {
    setOpen(false);
    button.current?.focus();
  });

  useEffect(() => {
    if (!open) return;
    const onPointer = (e: PointerEvent) => {
      if (root.current && !root.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', onPointer);
    return () => document.removeEventListener('pointerdown', onPointer);
  }, [open]);

  useEffect(() => {
    if (disabled) setOpen(false);
  }, [disabled]);

  return (
    <div className={className ? `pw-popover ${className}` : 'pw-popover'} ref={root}>
      <button
        ref={button}
        type="button"
        className="pw-button pw-popover-button"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
      >
        <span>{label}</span>
        <ChevronIcon />
      </button>
      {open ? (
        <div className="pw-popover-panel" id={id} role="group" aria-label={panelLabel}>
          {children(close)}
        </div>
      ) : null}
    </div>
  );
}
