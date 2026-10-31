import type { Notice } from '../layout/controller';
import { CloseIcon } from './icons';

/** Multi-screen and error notices, each dismissable. */
export function Notices({ notices, onDismiss }: { notices: readonly Notice[]; onDismiss(id: number): void }) {
  if (notices.length === 0) return null;
  return (
    <div className="pw-notices" role="status">
      {notices.map((n) => (
        <div key={n.id} className="pw-notice" data-kind={n.kind}>
          <span>{n.text}</span>
          <button type="button" className="pw-icon-button" aria-label="Dismiss" onClick={() => onDismiss(n.id)}>
            <CloseIcon />
          </button>
        </div>
      ))}
    </div>
  );
}
