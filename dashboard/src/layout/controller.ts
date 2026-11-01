import type { Device } from '../device/device';
import type { TileSize } from '../runtime/tile';
import type { LayoutSummary, LayoutsApi } from './api';
import {
  addTile,
  arrangementOf,
  removeTile,
  replaceArrangement,
  setTileSettings,
  toSavedBody,
  type Layout,
  type Tile,
} from './model';
import { COLUMNS, type ScreenClass } from './screen';

/** The preset a new device opens, and the fallback when its layout goes away. */
export const DEFAULT_LAYOUT = 'Pitwall Classic';

export type NoticeKind = 'moved' | 'conflict' | 'error';

export interface Notice {
  id: number;
  kind: NoticeKind;
  text: string;
}

export interface EditState {
  /** The screen class being edited: only this class changes. */
  screenClass: ScreenClass;
  /** What the editor works on. The grid shows this, not the saved layout. */
  draft: Layout;
  /** The layout as it was when editing began. */
  base: Layout;
  dirty: boolean;
  saving: boolean;
  /** Why the last Done failed. */
  error?: string;
  /** Where Done will save: the layout itself, or for a preset, the copy it will make. */
  targetName: string;
}

export interface ControllerState {
  status: 'loading' | 'ready' | 'error';
  layouts: readonly LayoutSummary[];
  current: Layout | null;
  /** `?layout=<name>`: this screen does not use or change device memory. */
  fixed: boolean;
  edit: EditState | null;
  notices: readonly Notice[];
}

export interface GridItem {
  i: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface LayoutController {
  subscribe(listener: () => void): () => void;
  getSnapshot(): ControllerState;
  start(): Promise<void>;
  /** Feed the layout-library revision from State; a change refetches over HTTP. */
  revisionChanged(revision: number): Promise<void>;
  select(name: string): Promise<void>;
  duplicate(name: string): Promise<void>;
  rename(name: string, newName: string): Promise<void>;
  remove(name: string): Promise<void>;
  dismissNotice(id: number): void;
  startEdit(screenClass: ScreenClass): void;
  cancelEdit(): void;
  done(): Promise<void>;
  /** Positions from the grid. Only a real change makes the draft dirty. */
  moveResize(items: readonly GridItem[]): void;
  addTile(type: string, size: TileSize): void;
  removeTile(id: string): void;
  setTileSettings(id: string, settings: Record<string, unknown>): void;
}

export interface ControllerOptions {
  api: LayoutsApi;
  device: Device;
  fixedLayout?: string | null;
  stream?: boolean;
  /** Integration seam for #48: report the missing Stream layout to the collector for
   * logging/status; null clears it when restored. No collector route exists yet. */
  onMissingStreamLayout?: (name: string | null) => void;
  /** Called with true on entering edit mode and false on leaving it. */
  onEditingChange?: (editing: boolean) => void;
}

const clone = <T,>(x: T): T => JSON.parse(JSON.stringify(x)) as T;
const sameJson = (a: unknown, b: unknown): boolean => JSON.stringify(a) === JSON.stringify(b);
const byId = (tiles: readonly Tile[]): Tile[] => [...tiles].sort((a, b) => (a.i < b.i ? -1 : a.i > b.i ? 1 : 0));
const lower = (s: string): string => s.toLowerCase();

/** The name the server gives a copy: "<name> copy", then "<name> copy 2". */
function copyNameFor(name: string, taken: readonly string[]): string {
  const used = new Set(taken.map(lower));
  let candidate = `${name} copy`;
  for (let n = 2; used.has(lower(candidate)); n++) candidate = `${name} copy ${n}`;
  return candidate;
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : 'Something went wrong';
}

/**
 * Everything about which layout this screen shows and how it is edited, apart from drawing
 * it. Talks to the layout routes, remembers the device's choice, and turns the collector's
 * revision counter into live updates and multi-screen notices.
 */
export function createLayoutController(options: ControllerOptions): LayoutController {
  const { api, device, onEditingChange } = options;
  const fixed = Boolean(options.fixedLayout) || Boolean(options.stream);
  const listeners = new Set<() => void>();
  let nextNotice = 1;
  /** Revision of the library as last fetched over HTTP. */
  let known: number | null = null;
  /** Last revision State announced; a repeat is not a change. Starts at the fetched one. */
  let signaled: number | null = null;
  /** Bumped by every change this screen makes, so a refetch begun earlier is discarded. */
  let generation = 0;
  /** A copy made for a preset whose save then failed; a retry reuses it. */
  let pendingCopy: string | null = null;

  const base: ControllerState = {
    status: 'loading',
    layouts: [],
    current: null,
    fixed,
    edit: null,
    notices: [],
  };
  let state: ControllerState = base;
  let view: ControllerState = state;

  function viewOf(s: ControllerState): ControllerState {
    if (!s.edit) return s;
    const { base: original } = s.edit;
    const targetName = original.preset
      ? s.edit.dirty
        ? copyNameFor(original.name, s.layouts.map((l) => l.name))
        : original.name
      : original.name;
    return s.edit.targetName === targetName ? s : { ...s, edit: { ...s.edit, targetName } };
  }

  function set(next: Partial<ControllerState>): void {
    state = { ...state, ...next };
    view = viewOf(state);
    for (const l of [...listeners]) l();
  }

  function notify(kind: NoticeKind, text: string): void {
    const kept = state.notices.filter((n) => !(n.kind === kind && n.text === text));
    set({ notices: [...kept, { id: nextNotice++, kind, text }] });
  }

  function remember(name: string): void {
    if (!fixed) device.setLayout(name);
  }

  async function loadLayouts(): Promise<void> {
    const list = await api.list();
    known = list.revision;
    signaled ??= list.revision;
    set({ layouts: list.layouts });
  }

  async function fallBack(text: string, started = generation): Promise<void> {
    const classic = await api.get(DEFAULT_LAYOUT);
    if (started !== generation) return;
    if (!classic) {
      set({ status: 'error', current: null });
      return;
    }
    remember(DEFAULT_LAYOUT);
    set({ current: classic, status: 'ready' });
    notify('moved', text);
  }

  async function show(name: string, started = generation): Promise<boolean> {
    const found = await api.get(name);
    if (started !== generation) return false;
    if (!found) return false;
    set({ current: found, status: 'ready' });
    return true;
  }

  let missingStreamLayout: string | null = null;
  async function showStream(started = generation): Promise<void> {
    const wanted = options.fixedLayout ?? DEFAULT_LAYOUT;
    const found = await api.get(wanted);
    if (started !== generation) return;
    set({ current: found, status: 'ready' });
    const missing = found ? null : wanted;
    if (missing !== missingStreamLayout) {
      missingStreamLayout = missing;
      options.onMissingStreamLayout?.(missing);
    }
  }

  const editing = () => state.edit !== null;

  function updateEdit(patch: Partial<EditState>): void {
    if (state.edit) set({ edit: { ...state.edit, ...patch } });
  }

  function changeDraft(change: (draft: Layout, screenClass: ScreenClass) => Layout): void {
    const edit = state.edit;
    if (!edit || edit.saving) return;
    const draft = change(edit.draft, edit.screenClass);
    const dirty = !sameJson(
      byId(arrangementOf(draft, edit.screenClass)),
      byId(arrangementOf(edit.base, edit.screenClass)),
    );
    if (sameJson(draft, edit.draft) && dirty === edit.dirty) return;
    set({ edit: { ...edit, draft, dirty, error: undefined } });
  }

  function leaveEdit(): void {
    pendingCopy = null;
    set({ edit: null });
    onEditingChange?.(false);
  }

  return {
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    getSnapshot: () => view,

    async start() {
      try {
        await loadLayouts();
        if (options.stream) {
          await showStream();
          return;
        }
        const wanted = options.fixedLayout ?? device.getSnapshot().layout ?? DEFAULT_LAYOUT;
        if (!(await show(wanted))) {
          await fallBack(`"${wanted}" no longer exists. Showing ${DEFAULT_LAYOUT}.`);
        }
      } catch {
        set({ status: 'error' });
      }
    },

    async revisionChanged(revision) {
      if (known === null || revision === signaled) return;
      signaled = revision;
      if (revision === known) return;
      const started = generation;
      let completed = false;
      try {
        const list = await api.list();
        if (started !== generation) return;
        set({ layouts: list.layouts });
        if (options.stream) {
          await showStream(started);
          if (started !== generation) return;
          known = list.revision;
          completed = true;
          return;
        }
        const here = state.current;
        if (!here) { known = list.revision; completed = true; return; }
        if (!list.layouts.some((l) => l.name === here.name)) {
          if (editing()) {
            notify(
              'conflict',
              `"${here.name}" was deleted or renamed on another screen. Done will save it again under this name.`,
            );
          } else {
            await fallBack(
              `"${here.name}" was deleted or renamed on another screen. Showing ${DEFAULT_LAYOUT}.`,
              started,
            );
          }
          if (started !== generation) return;
          known = list.revision;
          completed = true;
          return;
        }
        const latest = await api.get(here.name);
        if (started !== generation || !latest) return;
        known = list.revision;
        completed = true;
        if (sameJson(latest, here)) return;
        set({ current: latest });
        if (editing()) {
          notify('conflict', `"${here.name}" was saved on another screen. Done will replace it with your edits.`);
        }
      } catch {
        // The next State announcement retries, even at the same revision.
      } finally {
        if (!completed && signaled === revision) signaled = null;
      }
    },

    async select(name) {
      if (editing()) return;
      generation++;
      const started = generation;
      try {
        const shown = await show(name, started);
        if (started !== generation) return;
        if (shown) remember(name);
        else {
          notify('error', `"${name}" no longer exists.`);
          await loadLayouts();
        }
      } catch (error) {
        if (started !== generation) return;
        notify('error', messageOf(error));
      }
    },

    async duplicate(name) {
      if (editing()) return;
      generation++;
      try {
        const copy = await api.duplicate(name);
        generation++;
        await loadLayouts();
        set({ current: copy });
        remember(copy.name);
      } catch (error) {
        notify('error', messageOf(error));
      }
    },

    async rename(name, newName) {
      if (editing()) return;
      if (state.layouts.find((l) => l.name === name)?.preset) return;
      generation++;
      try {
        const renamed = await api.rename(name, newName);
        generation++;
        await loadLayouts();
        if (state.current?.name === name) {
          set({ current: renamed });
          remember(renamed.name);
        }
      } catch (error) {
        notify('error', messageOf(error));
      }
    },

    async remove(name) {
      if (editing()) return;
      if (state.layouts.find((l) => l.name === name)?.preset) return;
      generation++;
      try {
        await api.remove(name);
        generation++;
        await loadLayouts();
        if (state.current?.name === name) {
          const classic = await api.get(DEFAULT_LAYOUT);
          if (classic) {
            set({ current: classic });
            remember(DEFAULT_LAYOUT);
          }
        }
      } catch (error) {
        notify('error', messageOf(error));
      }
    },

    dismissNotice(id) {
      set({ notices: state.notices.filter((n) => n.id !== id) });
    },

    startEdit(screenClass) {
      const current = state.current;
      if (options.stream || !current || state.edit) return;
      set({
        edit: {
          screenClass,
          draft: clone(current),
          base: clone(current),
          dirty: false,
          saving: false,
          targetName: current.name,
        },
      });
      onEditingChange?.(true);
    },

    cancelEdit() {
      if (state.edit && !state.edit.saving) leaveEdit();
    },

    async done() {
      const edit = state.edit;
      if (!edit || edit.saving) return;
      if (!edit.dirty) {
        leaveEdit();
        return;
      }
      updateEdit({ saving: true, error: undefined });
      generation++;
      try {
        let target = edit.base.name;
        if (edit.base.preset) {
          // A preset is read-only, so the first change lands in a copy of it.
          pendingCopy ??= (await api.duplicate(edit.base.name)).name;
          target = pendingCopy;
        }
        const saved = await api.save(target, toSavedBody(edit.draft, target));
        generation++;
        try {
          await loadLayouts();
        } catch {
          // The saved layout is what matters; the list refreshes on the next revision.
        }
        remember(saved.name);
        set({ current: saved });
        leaveEdit();
      } catch (error) {
        updateEdit({ saving: false, error: messageOf(error) });
      }
    },

    moveResize(items) {
      changeDraft((draft, screenClass) => {
        const moved = new Map(items.map((item) => [item.i, item]));
        const tiles = arrangementOf(draft, screenClass).map((t) => {
          const item = moved.get(t.i);
          return item ? { ...t, x: item.x, y: item.y, w: item.w, h: item.h } : t;
        });
        return replaceArrangement(draft, screenClass, tiles);
      });
    },

    addTile(type, size) {
      changeDraft((draft, screenClass) =>
        replaceArrangement(draft, screenClass, addTile(arrangementOf(draft, screenClass), type, size, COLUMNS[screenClass]).tiles),
      );
    },

    removeTile(id) {
      changeDraft((draft, screenClass) =>
        replaceArrangement(draft, screenClass, removeTile(arrangementOf(draft, screenClass), id)),
      );
    },

    setTileSettings(id, settings) {
      changeDraft((draft, screenClass) =>
        replaceArrangement(draft, screenClass, setTileSettings(arrangementOf(draft, screenClass), id, settings)),
      );
    },
  };
}
