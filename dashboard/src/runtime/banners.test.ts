import { describe, expect, test } from 'vitest';
import { deriveBanners } from './banners';

const base = { connection: 'open', lifecycle: 'live', reloadPrompt: false } as const;

describe('deriveBanners', () => {
  test('nothing while connected and the game is sending', () => {
    expect(deriveBanners(base)).toEqual([]);
    expect(deriveBanners({ ...base, lifecycle: 'ended' })).toEqual([]);
  });

  test('first connection attempt says connecting', () => {
    const [b] = deriveBanners({ ...base, connection: 'connecting', lifecycle: undefined });
    expect(b!.id).toBe('connecting');
    expect(b!.text).toBe('Connecting to the collector');
  });

  test('lost connection says unreachable and promises reconnect', () => {
    const [b] = deriveBanners({ ...base, connection: 'reconnecting' });
    expect(b!.id).toBe('collector-unreachable');
    expect(b!.text).toBe('Collector unreachable. Reconnecting');
  });

  test('collector trouble hides game banners: one cause at a time', () => {
    expect(deriveBanners({ ...base, connection: 'reconnecting', lifecycle: 'waiting' })).toHaveLength(1);
  });

  test('waiting shows a thin banner with the setup hint', () => {
    const [b] = deriveBanners({ ...base, lifecycle: 'waiting' });
    expect(b!.id).toBe('game-not-sending');
    expect(b!.text).toBe('Waiting for the game');
    expect(b!.hint).toMatch(/UDP/);
    expect(b!.hint).toMatch(/20777/);
  });

  test('lost reads as the spec words it', () => {
    const [b] = deriveBanners({ ...base, lifecycle: 'lost' });
    expect(b!.text).toBe('No data: game paused or closed');
  });

  test('reload prompt is its own banner with an action', () => {
    const b = deriveBanners({ ...base, reloadPrompt: true });
    expect(b.map((x) => x.id)).toEqual(['reload']);
    expect(b[0]!.action).toBe('reload');
  });
});
