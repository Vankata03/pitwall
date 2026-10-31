import { describe, expect, test, vi } from 'vitest';
import { createDevice, memoryStorage } from '../device/device';
import { createLayoutController } from './controller';
import { fakeServer, layoutOf, tile } from './fake-server';
import { arrangementOf } from './model';

function setup(options: { extra?: ReturnType<typeof layoutOf>[]; stored?: Record<string, string>; fixed?: string | null } = {}) {
  const server = fakeServer(options.extra);
  const device = createDevice(memoryStorage(options.stored));
  const editing = vi.fn();
  const controller = createLayoutController({
    api: server,
    device,
    fixedLayout: options.fixed ?? null,
    onEditingChange: editing,
  });
  return { server, device, controller, editing };
}

const classic = 'Pitwall Classic';
const mine = layoutOf('Mine', false, ['tablet', 'phone']);

describe('opening', () => {
  test('a new device opens Pitwall Classic and writes nothing to storage yet', async () => {
    const { controller, device } = setup();
    await controller.start();
    expect(controller.getSnapshot().current?.name).toBe(classic);
    expect(device.getSnapshot().layout).toBeNull();
    expect(controller.getSnapshot().layouts.map((l) => l.name)).toEqual([classic, 'Driver', 'Race']);
  });

  test('the device’s remembered layout opens', async () => {
    const { controller } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    expect(controller.getSnapshot().current?.name).toBe('Mine');
    expect(controller.getSnapshot().notices).toEqual([]);
  });

  test('a remembered layout that no longer exists costs one re-pick, with a notice', async () => {
    const { controller, device } = setup({ stored: { 'pitwall.layout': 'Gone' } });
    await controller.start();
    expect(controller.getSnapshot().current?.name).toBe(classic);
    expect(controller.getSnapshot().notices.map((n) => n.kind)).toEqual(['moved']);
    expect(controller.getSnapshot().notices[0]!.text).toContain('Gone');
    expect(device.getSnapshot().layout).toBe(classic);
  });

  test('?layout=<name> opens that layout and leaves device memory alone', async () => {
    const { controller, device } = setup({ extra: [mine], fixed: 'Mine', stored: { 'pitwall.layout': 'Race' } });
    await controller.start();
    expect(controller.getSnapshot().current?.name).toBe('Mine');
    expect(controller.getSnapshot().fixed).toBe(true);
    controller.select('Driver');
    await vi.waitFor(() => expect(controller.getSnapshot().current?.name).toBe('Driver'));
    expect(device.getSnapshot().layout).toBe('Race');
  });

  test('an unreachable collector is an error state, not a crash', async () => {
    const { controller, server } = setup();
    server.list = async () => {
      throw new Error('down');
    };
    await controller.start();
    expect(controller.getSnapshot().status).toBe('error');
  });
});

describe('switcher actions', () => {
  test('choosing a layout shows it and the device remembers it', async () => {
    const { controller, device } = setup();
    await controller.start();
    await controller.select('Race');
    expect(controller.getSnapshot().current?.name).toBe('Race');
    expect(device.getSnapshot().layout).toBe('Race');
  });

  test('duplicate makes a copy and switches to it', async () => {
    const { controller, device, server } = setup();
    await controller.start();
    await controller.duplicate('Race');
    expect(controller.getSnapshot().current?.name).toBe('Race copy');
    expect(controller.getSnapshot().current?.preset).toBe(false);
    expect(device.getSnapshot().layout).toBe('Race copy');
    expect(server.log).toEqual(['POST duplicate Race']);
  });

  test('rename follows the layout this device shows', async () => {
    const { controller, device } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    await controller.rename('Mine', 'Mine 2');
    expect(controller.getSnapshot().current?.name).toBe('Mine 2');
    expect(device.getSnapshot().layout).toBe('Mine 2');
    expect(controller.getSnapshot().layouts.map((l) => l.name)).toContain('Mine 2');
  });

  test('a refused rename reports why and changes nothing', async () => {
    const { controller, server } = setup({ extra: [mine, layoutOf('Taken')], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    await controller.rename('Mine', 'Taken');
    expect(controller.getSnapshot().current?.name).toBe('Mine');
    expect(controller.getSnapshot().notices.at(-1)).toMatchObject({ kind: 'error', text: 'name already in use' });
    expect(server.layouts.has('Mine')).toBe(true);
  });

  test('deleting the layout this device shows falls back to Pitwall Classic', async () => {
    const { controller, device, server } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    await controller.remove('Mine');
    expect(server.layouts.has('Mine')).toBe(false);
    expect(controller.getSnapshot().current?.name).toBe(classic);
    expect(device.getSnapshot().layout).toBe(classic);
  });

  test('presets cannot be renamed or deleted, and the client does not even ask', async () => {
    const { controller, server } = setup();
    await controller.start();
    await controller.remove('Race');
    await controller.rename('Race', 'Mine');
    expect(server.log).toEqual([]);
  });
});

describe('several screens', () => {
  test('a layout deleted on another screen moves this screen to Pitwall Classic with a notice', async () => {
    const { controller, server, device } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    server.layouts.delete('Mine');
    server.revision++;
    await controller.revisionChanged(server.revision);
    expect(controller.getSnapshot().current?.name).toBe(classic);
    expect(device.getSnapshot().layout).toBe(classic);
    expect(controller.getSnapshot().notices[0]).toMatchObject({ kind: 'moved' });
    expect(controller.getSnapshot().notices[0]!.text).toMatch(/Mine.*(deleted|renamed).*Pitwall Classic/);
  });

  test('a layout saved on another screen updates this one live, with no notice', async () => {
    const { controller, server } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    const edited = layoutOf('Mine', false, []);
    edited.arrangements.wide = [tile('notice', 'notice', 0, 4, 24, 1)];
    server.layouts.set('Mine', edited);
    server.revision++;
    await controller.revisionChanged(server.revision);
    expect(arrangementOf(controller.getSnapshot().current!, 'wide')[0]!.y).toBe(4);
    expect(controller.getSnapshot().notices).toEqual([]);
  });

  test('the first revision seen is a baseline, not a change', async () => {
    const { controller, server } = setup();
    await controller.start();
    const before = controller.getSnapshot();
    await controller.revisionChanged(server.revision);
    expect(controller.getSnapshot()).toBe(before);
  });

  test('an unchanged revision does nothing', async () => {
    const { controller, server } = setup();
    await controller.start();
    await controller.revisionChanged(5);
    const calls = vi.spyOn(server, 'list');
    await controller.revisionChanged(5);
    expect(calls).not.toHaveBeenCalled();
  });

  test('while editing, a save elsewhere leaves the draft and warns that Done replaces it', async () => {
    const { controller, server } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    controller.startEdit('wide');
    controller.moveResize([{ i: 'future-tile', x: 6, y: 1, w: 6, h: 2 }]);
    const theirs = layoutOf('Mine', false, []);
    theirs.arrangements.wide = [tile('notice', 'notice', 0, 9, 24, 1)];
    server.layouts.set('Mine', theirs);
    server.revision++;
    await controller.revisionChanged(server.revision);
    const snap = controller.getSnapshot();
    expect(snap.edit).not.toBeNull();
    expect(arrangementOf(snap.edit!.draft, 'wide').find((t) => t.i === 'future-tile')!.x).toBe(6);
    expect(snap.notices.at(-1)).toMatchObject({ kind: 'conflict' });
    expect(snap.notices.at(-1)!.text).toMatch(/Done will replace/);
  });

  test('while editing, a layout deleted elsewhere keeps the draft and says so', async () => {
    const { controller, server } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    controller.startEdit('wide');
    server.layouts.delete('Mine');
    server.revision++;
    await controller.revisionChanged(server.revision);
    expect(controller.getSnapshot().edit).not.toBeNull();
    expect(controller.getSnapshot().notices.at(-1)).toMatchObject({ kind: 'conflict' });
    expect(controller.getSnapshot().notices.at(-1)!.text).toMatch(/deleted or renamed/);
  });

  test('this screen’s own save does not warn itself', async () => {
    const { controller, server } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    controller.startEdit('wide');
    controller.moveResize([{ i: 'future-tile', x: 6, y: 1, w: 6, h: 2 }]);
    await controller.done();
    await controller.revisionChanged(server.revision);
    expect(controller.getSnapshot().notices).toEqual([]);
    expect(arrangementOf(controller.getSnapshot().current!, 'wide').find((t) => t.i === 'future-tile')!.x).toBe(6);
  });
});

describe('edit mode', () => {
  test('Edit tells the runtime, and Done and Cancel tell it again', async () => {
    const { controller, editing } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    controller.startEdit('wide');
    expect(editing).toHaveBeenLastCalledWith(true);
    controller.cancelEdit();
    expect(editing).toHaveBeenLastCalledWith(false);
    controller.startEdit('wide');
    await controller.done();
    expect(editing).toHaveBeenLastCalledWith(false);
  });

  test('Cancel discards every change and sends nothing', async () => {
    const { controller, server } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    controller.startEdit('wide');
    controller.moveResize([{ i: 'future-tile', x: 12, y: 1, w: 6, h: 2 }]);
    controller.removeTile('notice');
    controller.cancelEdit();
    expect(controller.getSnapshot().edit).toBeNull();
    expect(arrangementOf(controller.getSnapshot().current!, 'wide')).toHaveLength(2);
    expect(server.log).toEqual([]);
  });

  test('Done without a change sends nothing and makes no copy of a preset', async () => {
    const { controller, server } = setup();
    await controller.start();
    controller.startEdit('wide');
    controller.moveResize(arrangementOf(controller.getSnapshot().current!, 'wide').map(({ i, x, y, w, h }) => ({ i, x, y, w, h })));
    await controller.done();
    expect(server.log).toEqual([]);
    expect(controller.getSnapshot().edit).toBeNull();
  });

  test('moving and resizing change the draft, and Done saves it over the layout', async () => {
    const { controller, server } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    controller.startEdit('wide');
    controller.moveResize([
      { i: 'notice', x: 0, y: 0, w: 24, h: 1 },
      { i: 'future-tile', x: 18, y: 1, w: 6, h: 4 },
    ]);
    expect(controller.getSnapshot().edit!.dirty).toBe(true);
    await controller.done();
    expect(server.log).toEqual(['PUT Mine']);
    const saved = server.layouts.get('Mine')!;
    expect(saved.arrangements.wide.find((t) => t.i === 'future-tile')).toMatchObject({ x: 18, h: 4 });
    expect(controller.getSnapshot().edit).toBeNull();
  });

  test('editing changes only the current screen class, and that class stops being generated', async () => {
    const { controller, server } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    controller.startEdit('tablet');
    controller.moveResize([
      { i: 'notice', x: 0, y: 0, w: 12, h: 1 },
      { i: 'future-tile', x: 6, y: 1, w: 6, h: 2 },
    ]);
    await controller.done();
    const saved = server.layouts.get('Mine')!;
    expect(saved.generated).toEqual(['phone']);
    expect(saved.arrangements.wide).toEqual(mine.arrangements.wide);
    expect(saved.arrangements.phone).toEqual(mine.arrangements.phone);
    expect(saved.arrangements.tablet.find((t) => t.i === 'future-tile')!.x).toBe(6);
  });

  test('the first change to a preset is saved as a copy, and the device moves to it', async () => {
    const { controller, server, device } = setup();
    await controller.start();
    controller.startEdit('wide');
    expect(controller.getSnapshot().edit!.targetName).toBe(classic);
    controller.removeTile('notice');
    expect(controller.getSnapshot().edit!.targetName).toBe('Pitwall Classic copy');
    await controller.done();
    expect(server.log).toEqual([`POST duplicate ${classic}`, 'PUT Pitwall Classic copy']);
    expect(server.layouts.get(classic)!.arrangements.wide).toHaveLength(2);
    expect(server.layouts.get('Pitwall Classic copy')!.arrangements.wide.map((t) => t.i)).toEqual(['future-tile']);
    expect(controller.getSnapshot().current?.name).toBe('Pitwall Classic copy');
    expect(device.getSnapshot().layout).toBe('Pitwall Classic copy');
  });

  test('the predicted copy name skips names already taken', async () => {
    const { controller } = setup({ extra: [layoutOf('Pitwall Classic copy')] });
    await controller.start();
    controller.startEdit('wide');
    controller.removeTile('notice');
    expect(controller.getSnapshot().edit!.targetName).toBe('Pitwall Classic copy 2');
  });

  test('add puts a tile under the others at its default size, with its own id', async () => {
    const { controller } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    controller.startEdit('wide');
    controller.addTile('future-tile', { w: 6, h: 2 });
    const tiles = arrangementOf(controller.getSnapshot().edit!.draft, 'wide');
    expect(tiles.at(-1)).toMatchObject({ i: 'future-tile-2', type: 'future-tile', x: 0, y: 3, w: 6, h: 2 });
    expect(controller.getSnapshot().edit!.dirty).toBe(true);
  });

  test('tile settings are saved with the tile instance', async () => {
    const { controller, server } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    controller.startEdit('wide');
    controller.setTileSettings('future-tile', { car: { target: 'focus' } });
    await controller.done();
    expect(server.layouts.get('Mine')!.arrangements.wide.find((t) => t.i === 'future-tile')!.settings).toEqual({
      car: { target: 'focus' },
    });
  });

  test('an unknown tile type survives moving, resizing and saving untouched', async () => {
    const odd = layoutOf('Odd');
    odd.arrangements.wide.push({ ...tile('future-1', 'warp-core', 0, 3, 4, 2), settings: { flux: 3 }, colour: 'red' });
    const { controller, server } = setup({ extra: [odd], stored: { 'pitwall.layout': 'Odd' } });
    await controller.start();
    controller.startEdit('wide');
    controller.moveResize([{ i: 'future-1', x: 4, y: 3, w: 4, h: 2 }]);
    await controller.done();
    expect(server.layouts.get('Odd')!.arrangements.wide.find((t) => t.i === 'future-1')).toEqual({
      i: 'future-1',
      type: 'warp-core',
      x: 4,
      y: 3,
      w: 4,
      h: 2,
      settings: { flux: 3 },
      colour: 'red',
    });
  });

  test('a failed save keeps the editor open with the draft and says why', async () => {
    const { controller, server } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    controller.startEdit('wide');
    controller.removeTile('notice');
    server.failNext(500, 'disk full');
    await controller.done();
    const snap = controller.getSnapshot();
    expect(snap.edit).not.toBeNull();
    expect(snap.edit!.error).toBe('disk full');
    expect(snap.edit!.saving).toBe(false);
    await controller.done();
    expect(controller.getSnapshot().edit).toBeNull();
  });

  test('a retry after the copy was made but the save failed reuses the copy', async () => {
    const { controller, server } = setup();
    await controller.start();
    controller.startEdit('wide');
    controller.removeTile('notice');
    server.log.length = 0;
    // Fail the PUT: the duplicate succeeds first, so fail on the second mutation.
    const realSave = server.save.bind(server);
    let failures = 1;
    server.save = async (name, body) => {
      if (failures-- > 0) throw new Error('offline');
      return realSave(name, body);
    };
    await controller.done();
    expect(controller.getSnapshot().edit).not.toBeNull();
    await controller.done();
    expect(server.log.filter((l) => l.startsWith('POST duplicate'))).toHaveLength(1);
    expect(controller.getSnapshot().edit).toBeNull();
  });

  test('the layout cannot be switched or renamed mid-edit', async () => {
    const { controller } = setup({ extra: [mine], stored: { 'pitwall.layout': 'Mine' } });
    await controller.start();
    controller.startEdit('wide');
    await controller.select('Race');
    expect(controller.getSnapshot().current?.name).toBe('Mine');
  });
});

test('Stream view stays blank for a missing named layout and recovers on a library revision', async () => {
  const server = fakeServer();
  const device = createDevice(memoryStorage({ 'pitwall.layout': 'Race' }));
  const missing = vi.fn();
  const controller = createLayoutController({ api: server, device, fixedLayout: 'Air', stream: true, onMissingStreamLayout: missing });
  await controller.start();
  expect(controller.getSnapshot().current).toBeNull();
  expect(controller.getSnapshot().notices).toEqual([]);
  expect(missing).toHaveBeenLastCalledWith('Air');
  expect(device.getSnapshot().layout).toBe('Race');
  server.layouts.set('Air', layoutOf('Air'));
  server.revision++;
  await controller.revisionChanged(server.revision);
  expect(controller.getSnapshot().current?.name).toBe('Air');
  expect(missing).toHaveBeenLastCalledWith(null);
  server.layouts.delete('Air');
  server.revision++;
  await controller.revisionChanged(server.revision);
  expect(controller.getSnapshot().current).toBeNull();
  expect(missing).toHaveBeenLastCalledWith('Air');
});
