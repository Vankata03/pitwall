import { useState } from 'react';
import type { LayoutSummary } from '../layout/api';
import { CopyIcon, RenameIcon, TrashIcon } from './icons';
import { Popover } from './popover';

export interface LayoutSwitcherProps {
  layouts: readonly LayoutSummary[];
  current: string;
  disabled: boolean;
  onSelect(name: string): void;
  onDuplicate(name: string): void;
  onRename(name: string, newName: string): void;
  onDelete(name: string): void;
}

type Row = { mode: 'idle' } | { mode: 'rename'; name: string } | { mode: 'delete'; name: string };

/** Choose, rename, duplicate and delete layouts. Presets can be chosen and copied only. */
export function LayoutSwitcher(props: LayoutSwitcherProps) {
  return (
    <Popover label={<LabelText current={props.current} />} panelLabel="Layouts" disabled={props.disabled} className="pw-switcher">
      {(close) => <Panel {...props} close={close} />}
    </Popover>
  );
}

function LabelText({ current }: { current: string }) {
  return (
    <>
      <span className="pw-label">Layout</span>{' '}
      <span className="pw-button-value">{current}</span>
    </>
  );
}

function Panel({ layouts, current, onSelect, onDuplicate, onRename, onDelete, close }: LayoutSwitcherProps & { close: () => void }) {
  const [row, setRow] = useState<Row>({ mode: 'idle' });
  const [draft, setDraft] = useState('');

  return (
    <ul className="pw-menu-list">
      {layouts.map((layout) => {
        if (row.mode === 'rename' && row.name === layout.name) {
          return (
            <li key={layout.name} className="pw-menu-row">
              <form
                className="pw-inline-form"
                onSubmit={(e) => {
                  e.preventDefault();
                  const next = draft.trim();
                  if (next && next !== layout.name) onRename(layout.name, next);
                  setRow({ mode: 'idle' });
                }}
              >
                <label className="pw-label" htmlFor="pw-rename">
                  New name for {layout.name}
                </label>
                <input id="pw-rename" className="pw-input" value={draft} maxLength={64} autoFocus onChange={(e) => setDraft(e.target.value)} />
                <button type="submit" className="pw-button">
                  Save
                </button>
                <button type="button" className="pw-button" onClick={() => setRow({ mode: 'idle' })}>
                  Cancel
                </button>
              </form>
            </li>
          );
        }
        if (row.mode === 'delete' && row.name === layout.name) {
          return (
            <li key={layout.name} className="pw-menu-row">
              <div className="pw-inline-form" role="alertdialog" aria-label={`Delete ${layout.name}`}>
                <span className="pw-value">Delete “{layout.name}”?</span>
                <button
                  type="button"
                  className="pw-button pw-danger"
                  onClick={() => {
                    onDelete(layout.name);
                    setRow({ mode: 'idle' });
                  }}
                >
                  Delete
                </button>
                <button type="button" className="pw-button" autoFocus onClick={() => setRow({ mode: 'idle' })}>
                  Keep
                </button>
              </div>
            </li>
          );
        }
        const selected = layout.name === current;
        return (
          <li key={layout.name} className="pw-menu-row">
            <button
              type="button"
              className="pw-button pw-menu-choice"
              aria-current={selected ? 'true' : undefined}
              data-selected={selected}
              onClick={() => {
                onSelect(layout.name);
                close();
              }}
            >
              <span>{layout.name}</span>{' '}
              {layout.preset ? <span className="pw-label">Preset</span> : null}
            </button>
            <button
              type="button"
              className="pw-icon-button"
              aria-label={`Duplicate ${layout.name}`}
              onClick={() => {
                onDuplicate(layout.name);
                close();
              }}
            >
              <CopyIcon />
            </button>
            {layout.preset ? null : (
              <>
                <button
                  type="button"
                  className="pw-icon-button"
                  aria-label={`Rename ${layout.name}`}
                  onClick={() => {
                    setDraft(layout.name);
                    setRow({ mode: 'rename', name: layout.name });
                  }}
                >
                  <RenameIcon />
                </button>
                <button type="button" className="pw-icon-button" aria-label={`Delete ${layout.name}`} onClick={() => setRow({ mode: 'delete', name: layout.name })}>
                  <TrashIcon />
                </button>
              </>
            )}
          </li>
        );
      })}
    </ul>
  );
}
