import { describe, expect, test, vi } from 'vitest';
import { createDevice, layoutNameFromUrl, memoryStorage } from './device';
import { DEFAULT_UNITS } from './units';

describe('device memory (spec section 10: browser storage)', () => {
  test('a new device has no layout, follows the player and uses default units', () => {
    const device = createDevice(memoryStorage());
    expect(device.getSnapshot()).toEqual({ layout: null, focus: null, units: DEFAULT_UNITS });
  });

  test('choices survive a reload', () => {
    const storage = memoryStorage();
    const first = createDevice(storage);
    first.setLayout('Race');
    first.setFocus({ kind: 'driver', driverId: 5 });
    first.setUnits({ speed: 'mph', temperature: 'f', pressure: 'bar' });
    const second = createDevice(storage);
    expect(second.getSnapshot()).toEqual({
      layout: 'Race',
      focus: { kind: 'driver', driverId: 5 },
      units: { speed: 'mph', temperature: 'f', pressure: 'bar' },
    });
  });

  test('units change one at a time', () => {
    const device = createDevice(memoryStorage());
    device.setUnits({ ...DEFAULT_UNITS, pressure: 'bar' });
    device.setUnits({ ...device.getSnapshot().units, speed: 'mph' });
    expect(device.getSnapshot().units).toEqual({ speed: 'mph', temperature: 'c', pressure: 'bar' });
  });

  test('Follow player clears the focus', () => {
    const device = createDevice(memoryStorage());
    device.setFocus({ kind: 'name', name: 'Max' });
    device.setFocus(null);
    expect(device.getSnapshot().focus).toBeNull();
    expect(createDevice(device.storage).getSnapshot().focus).toBeNull();
  });

  test('corrupt or unknown stored values fall back to the defaults', () => {
    const storage = memoryStorage({
      'pitwall.focus': '{oops',
      'pitwall.units': JSON.stringify({ speed: 'furlongs', temperature: 'f' }),
    });
    expect(createDevice(storage).getSnapshot()).toEqual({
      layout: null,
      focus: null,
      units: { speed: 'kmh', temperature: 'f', pressure: 'psi' },
    });
  });

  test('storage that throws (private mode, blocked) leaves the choices in memory', () => {
    const broken = {
      getItem: () => {
        throw new Error('blocked');
      },
      setItem: () => {
        throw new Error('blocked');
      },
      removeItem: () => {
        throw new Error('blocked');
      },
    };
    const device = createDevice(broken);
    device.setLayout('Race');
    expect(device.getSnapshot().layout).toBe('Race');
  });

  test('subscribers hear changes, and the snapshot is stable between them', () => {
    const device = createDevice(memoryStorage());
    const listener = vi.fn();
    const off = device.subscribe(listener);
    const before = device.getSnapshot();
    expect(device.getSnapshot()).toBe(before);
    device.setLayout('Race');
    expect(listener).toHaveBeenCalledTimes(1);
    expect(device.getSnapshot()).not.toBe(before);
    off();
    device.setLayout('Driver');
    expect(listener).toHaveBeenCalledTimes(1);
  });

  test('setting the same value again is not a change', () => {
    const device = createDevice(memoryStorage());
    device.setLayout('Race');
    const listener = vi.fn();
    device.subscribe(listener);
    device.setLayout('Race');
    expect(listener).not.toHaveBeenCalled();
  });
});

describe('?layout=<name>', () => {
  test('names a fixed layout', () => {
    expect(layoutNameFromUrl('?layout=Pitwall%20Classic')).toBe('Pitwall Classic');
    expect(layoutNameFromUrl('?foo=1&layout=Race&stream=1')).toBe('Race');
  });
  test('absent or empty means the device’s own choice', () => {
    expect(layoutNameFromUrl('')).toBeNull();
    expect(layoutNameFromUrl('?layout=')).toBeNull();
  });
});
