import { describe, expect, test, vi } from 'vitest';
import { BuildGuard } from './build-guard';

function make(own = 'new') {
  const reload = vi.fn();
  const mem = new Map<string, string>();
  const storage = { getItem: (k: string) => mem.get(k) ?? null, setItem: (k: string, v: string) => void mem.set(k, v) };
  const guard = new BuildGuard({ ownBuildId: own, reload, storage });
  return { guard, reload, mem };
}

describe('BuildGuard', () => {
  test('matching build ID does nothing', () => {
    const { guard, reload } = make('abc');
    guard.onHello('abc');
    expect(reload).not.toHaveBeenCalled();
    expect(guard.getSnapshot()).toBe(false);
  });

  test('mismatch reloads at once outside edit mode', () => {
    const { guard, reload } = make('old');
    guard.onHello('new');
    expect(reload).toHaveBeenCalledTimes(1);
  });

  test('in edit mode it shows a prompt instead, so unsaved edits survive', () => {
    const { guard, reload } = make('old');
    guard.setEditing(true);
    guard.onHello('new');
    expect(reload).not.toHaveBeenCalled();
    expect(guard.getSnapshot()).toBe(true);
  });

  test('the prompt reloads on request', () => {
    const { guard, reload } = make('old');
    guard.setEditing(true);
    guard.onHello('new');
    guard.reloadNow();
    expect(reload).toHaveBeenCalledTimes(1);
  });

  test('leaving edit mode with a prompt pending reloads, since nothing is unsaved', () => {
    const { guard, reload } = make('old');
    guard.setEditing(true);
    guard.onHello('new');
    guard.setEditing(false);
    expect(reload).toHaveBeenCalledTimes(1);
  });

  test('never reloads twice for the same collector build (stale cache, dev server)', () => {
    const { guard, reload } = make('old');
    guard.onHello('new');
    guard.onHello('new');
    expect(reload).toHaveBeenCalledTimes(1);
    // a fresh page load after that reload shares storage
    const again = new BuildGuard({
      ownBuildId: 'old',
      reload,
      storage: { getItem: () => 'new', setItem: () => {} },
    });
    again.onHello('new');
    expect(reload).toHaveBeenCalledTimes(1);
    expect(again.getSnapshot()).toBe(false);
  });

  test('notifies subscribers when the prompt appears', () => {
    const { guard } = make('old');
    const l = vi.fn();
    guard.subscribe(l);
    guard.setEditing(true);
    guard.onHello('new');
    expect(l).toHaveBeenCalledTimes(1);
  });
});
