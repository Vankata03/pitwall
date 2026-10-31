import { describe, expect, test } from 'vitest';
import { RingBuffer } from './ring';

describe('RingBuffer', () => {
  test('keeps pushes in order, oldest first', () => {
    const r = new RingBuffer(4);
    r.push(1);
    r.push(2);
    expect(r.length).toBe(2);
    expect([r.at(0), r.at(1)]).toEqual([1, 2]);
  });

  test('overwrites the oldest once full', () => {
    const r = new RingBuffer(3);
    for (const v of [1, 2, 3, 4, 5]) r.push(v);
    expect(r.length).toBe(3);
    expect(r.toArray()).toEqual([3, 4, 5]);
    expect(r.last()).toBe(5);
  });

  test('out-of-range reads give NaN, never a stale slot', () => {
    const r = new RingBuffer(3);
    r.push(9);
    expect(r.at(1)).toBeNaN();
    expect(r.at(-1)).toBeNaN();
    expect(new RingBuffer(2).last()).toBeNaN();
  });

  test('clear empties it and bumps version on every change', () => {
    const r = new RingBuffer(3);
    const v0 = r.version;
    r.push(1);
    const v1 = r.version;
    r.clear();
    expect(v1).toBeGreaterThan(v0);
    expect(r.version).toBeGreaterThan(v1);
    expect(r.length).toBe(0);
    expect(r.toArray()).toEqual([]);
  });

  test('rejects a non-positive capacity', () => {
    expect(() => new RingBuffer(0)).toThrow();
  });
});
