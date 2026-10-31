// @vitest-environment jsdom
import { act, cleanup, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, test } from 'vitest';
import { layoutOf, tile } from '../layout/fake-server';
import { SHORT_LINE } from '../shared/short-line';
import { createRegistry, mount } from './harness';

afterEach(cleanup);

async function ready(view: ReturnType<typeof mount>) {
  await act(async () => {
    await view.controller.start();
  });
}

describe('chrome', () => {
  test('every screen has the switcher, focus picker, display menu, Edit, status link and the short line', async () => {
    const view = mount();
    await ready(view);
    expect(screen.getByRole('button', { name: /Layout\s*Pitwall Classic/ })).toBeTruthy();
    expect(screen.getByLabelText('Focus car')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Display' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Edit' })).toBeTruthy();
    expect(screen.getByRole('link', { name: 'Status' }).getAttribute('href')).toBe('/status');
    const footer = document.querySelector('footer')!;
    expect(within(footer).getByRole('link', { name: SHORT_LINE }).getAttribute('href')).toBe('/status#notice');
  });

  test('connection banners show in the chrome', async () => {
    const view = mount();
    await ready(view);
    expect(screen.getByText('Connecting to the collector')).toBeTruthy();
  });
});

describe('the grid', () => {
  test('shows the layout’s tiles; types this build lacks get the placeholder and keep their place', async () => {
    const view = mount();
    await ready(view);
    // The fake preset holds Notice and a type this build lacks.
    expect(document.querySelectorAll('.pw-tile')).toHaveLength(2);
    expect(screen.getAllByTestId('unknown-tile')).toHaveLength(1);
    expect(screen.getByTestId('unknown-tile').textContent).toContain('future-tile');
    expect(screen.getAllByText(SHORT_LINE).length).toBe(2); // the Notice tile and the footer
  });

  test('the page class comes from the viewport width', async () => {
    const view = mount({ viewport: 800 });
    await ready(view);
    expect(document.querySelector('.react-grid-layout')).toBeTruthy();
  });
});

const user = () => userEvent.setup();

describe('edit mode', () => {
  test('Edit shows Done and Cancel, and tells the runtime so a build mismatch prompts instead of reloading', async () => {
    const view = mount({ extra: [layoutOf('Mine')], stored: { 'pitwall.layout': 'Mine' } });
    await ready(view);
    await user().click(screen.getByRole('button', { name: 'Edit' }));
    expect(screen.getByRole('button', { name: 'Done' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Edit' })).toBeNull();
    expect(view.setEditing).toHaveBeenLastCalledWith(true);
    await user().click(screen.getByRole('button', { name: 'Cancel' }));
    expect(view.setEditing).toHaveBeenLastCalledWith(false);
    expect(screen.getByRole('button', { name: 'Edit' })).toBeTruthy();
  });

  test('tiles get a settings control and a remove control in edit mode, and not otherwise', async () => {
    const view = mount({ extra: [layoutOf('Mine')], stored: { 'pitwall.layout': 'Mine' } });
    await ready(view);
    expect(screen.queryByRole('button', { name: /^Remove/ })).toBeNull();
    await user().click(screen.getByRole('button', { name: 'Edit' }));
    expect(screen.getByRole('button', { name: 'Remove Notice' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Settings for Notice' })).toBeTruthy();
    // An unknown tile can be removed but has no settings to open.
    expect(screen.getByRole('button', { name: 'Remove future-tile' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Settings for future-tile' })).toBeNull();
  });

  test('removing a tile and pressing Done saves the layout and leaves edit mode', async () => {
    const view = mount({ extra: [layoutOf('Mine')], stored: { 'pitwall.layout': 'Mine' } });
    await ready(view);
    const u = user();
    await u.click(screen.getByRole('button', { name: 'Edit' }));
    await u.click(screen.getByRole('button', { name: 'Remove future-tile' }));
    await u.click(screen.getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Edit' })).toBeTruthy());
    expect(view.server.layouts.get('Mine')!.arrangements.wide.map((t) => t.i)).toEqual(['notice']);
  });

  test('Done with nothing changed saves nothing, so the grid’s own re-layout never makes a copy of a preset', async () => {
    const view = mount();
    await ready(view);
    const u = user();
    await u.click(screen.getByRole('button', { name: 'Edit' }));
    await u.click(screen.getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Edit' })).toBeTruthy());
    expect(view.server.log).toEqual([]);
  });

  test('on a preset the first change is saved as a copy, and the screen says so', async () => {
    const view = mount();
    await ready(view);
    const u = user();
    await u.click(screen.getByRole('button', { name: 'Edit' }));
    expect(screen.getByText(/Pitwall Classic is read-only/)).toBeTruthy();
    await u.click(screen.getByRole('button', { name: 'Remove future-tile' }));
    expect(screen.getByText(/Done saves your changes as “Pitwall Classic copy”/)).toBeTruthy();
    await u.click(screen.getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(screen.getByRole('button', { name: /Layout\s*Pitwall Classic copy/ })).toBeTruthy());
    expect(view.server.layouts.get('Pitwall Classic')!.arrangements.wide).toHaveLength(2);
    expect(view.device.getSnapshot().layout).toBe('Pitwall Classic copy');
  });

  test('Add tile lists the registry and adds a tile under the others', async () => {
    const view = mount({ extra: [layoutOf('Mine')], stored: { 'pitwall.layout': 'Mine' } });
    await ready(view);
    const u = user();
    await u.click(screen.getByRole('button', { name: 'Edit' }));
    await u.click(screen.getByRole('button', { name: 'Add tile' }));
    await u.click(screen.getByRole('button', { name: /Notice\s*6×1/ }));
    expect(screen.getAllByRole('region', { name: 'Notice' })).toHaveLength(2);
    await u.click(screen.getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Edit' })).toBeTruthy());
    const saved = view.server.layouts.get('Mine')!.arrangements.wide;
    expect(saved.at(-1)).toMatchObject({ i: 'notice-2', type: 'notice', w: 6, h: 1 });
  });

  test('a failed save keeps edit mode open and says why', async () => {
    const view = mount({ extra: [layoutOf('Mine')], stored: { 'pitwall.layout': 'Mine' } });
    await ready(view);
    const u = user();
    await u.click(screen.getByRole('button', { name: 'Edit' }));
    await u.click(screen.getByRole('button', { name: 'Remove future-tile' }));
    view.server.failNext(500, 'disk full');
    await u.click(screen.getByRole('button', { name: 'Done' }));
    expect((await screen.findByRole('alert')).textContent).toContain('disk full');
    expect(screen.getByRole('button', { name: 'Done' })).toBeTruthy();
  });
});

// A follows-car tile, to exercise car binding without any catalogue tile.
const carProbe = createRegistry([
  ...createRegistry([]).all(),
  {
    declaration: {
      type: 'car-probe',
      name: 'Car probe',
      binding: 'follows-car',
      rate: 'slow',
      minSize: { w: 2, h: 1 },
      defaultSize: { w: 4, h: 2 },
      densities: [
        { density: 'compact', minWidth: 0, minHeight: 0 },
        { density: 'regular', minWidth: 300, minHeight: 100 },
      ],
      reads: [],
    },
    Component: ({ car, units, density, width }) => (
      <p data-testid="probe">
        {car?.status === 'ok' || car?.status === 'retired' ? `car ${car.index}` : 'no car'} {units.speed} {density} {Math.round(width)}
      </p>
    ),
  },
]);

const participant = (driver: number, name: string, over: Record<string, unknown> = {}) => ({
  participant: {
    state: 'live',
    value: { ai_controlled: 1, driver_id: driver, network_id: 255, race_number: driver, show_online_names: 1, name: [...new TextEncoder().encode(name)], ...over },
  },
  retired: false,
  lap: { state: 'live', value: { car_position: driver + 1 } },
});
const session = (extra: Record<string, unknown> = {}) => ({
  playerCar: 0,
  cars: [participant(0, 'Norris'), participant(1, 'Piastri'), participant(2, 'Hamilton')],
  context: { revisions: { layouts: 0, tracks: 0 } },
  ...extra,
});
const probeLayout = () => {
  const l = layoutOf('Cars');
  l.arrangements.wide = [tile('car-probe', 'car-probe', 0, 0, 6, 3)];
  return l;
};

describe('focus car and pins', () => {
  test('the picker lists every car plus Follow player, and a follows-car tile follows the focus', async () => {
    const view = mount({ registry: carProbe, extra: [probeLayout()], stored: { 'pitwall.layout': 'Cars' } });
    await ready(view);
    act(() => view.sendState(session()));
    const picker = (await screen.findByLabelText('Focus car')) as HTMLSelectElement;
    await waitFor(() => expect([...picker.options].map((o) => o.text)).toEqual(['Follow player', 'Norris', 'Piastri', 'Hamilton']));
    expect(screen.getByTestId('probe').textContent).toContain('car 0');
    await user().selectOptions(picker, 'Hamilton');
    expect(view.device.getSnapshot().focus).toEqual({ kind: 'driver', driverId: 2 });
    await waitFor(() => expect(screen.getByTestId('probe').textContent).toContain('car 2'));
    await user().selectOptions(picker, 'Follow player');
    expect(view.device.getSnapshot().focus).toBeNull();
  });

  test('a stored focus that is not in the session says so, and tiles show car not in session', async () => {
    const view = mount({
      registry: carProbe,
      extra: [probeLayout()],
      stored: { 'pitwall.layout': 'Cars', 'pitwall.focus': JSON.stringify({ kind: 'driver', driverId: 99 }) },
    });
    await ready(view);
    act(() => view.sendState(session()));
    await waitFor(() => expect(screen.getAllByText('Car not in session').length).toBeGreaterThan(0));
    expect((screen.getByLabelText('Focus car') as HTMLSelectElement).selectedOptions[0]!.text).toBe('Car not in session');
  });

  test('tile settings pin a driver, saved with the tile instance', async () => {
    const view = mount({ registry: carProbe, extra: [probeLayout()], stored: { 'pitwall.layout': 'Cars' } });
    await ready(view);
    act(() => view.sendState(session()));
    await screen.findByTestId('probe');
    const u = user();
    await u.click(screen.getByRole('button', { name: 'Edit' }));
    await u.click(screen.getByRole('button', { name: 'Settings for Car probe' }));
    const dialog = screen.getByRole('dialog', { name: 'Car probe settings' });
    await u.selectOptions(within(dialog).getByLabelText('Follows'), 'Pinned driver');
    await u.selectOptions(within(dialog).getByLabelText('Driver'), 'Piastri');
    await u.click(within(dialog).getByRole('button', { name: 'Close' }));
    await waitFor(() => expect(screen.getByTestId('probe').textContent).toContain('car 1'));
    await u.click(screen.getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Edit' })).toBeTruthy());
    expect(view.server.layouts.get('Cars')!.arrangements.wide[0]!.settings).toEqual({
      car: { target: 'pinned', pin: { kind: 'driver', driverId: 1 } },
    });
  });

  test('a pinned driver absent from the session shows car not in session and keeps the pin', async () => {
    const pinned = probeLayout();
    pinned.arrangements.wide[0]!.settings = { car: { target: 'pinned', pin: { kind: 'driver', driverId: 77 } } };
    const view = mount({ registry: carProbe, extra: [pinned], stored: { 'pitwall.layout': 'Cars' } });
    await ready(view);
    act(() => view.sendState(session()));
    await waitFor(() => expect(screen.getByText('Car not in session')).toBeTruthy());
    expect(view.server.layouts.get('Cars')!.arrangements.wide[0]!.settings).toEqual(pinned.arrangements.wide[0]!.settings);
  });
});

describe('units', () => {
  test('the display menu changes units for this device only, and tiles see them', async () => {
    const view = mount({ registry: carProbe, extra: [probeLayout()], stored: { 'pitwall.layout': 'Cars' } });
    await ready(view);
    act(() => view.sendState(session()));
    const u = user();
    expect(await screen.findByTestId('probe')).toBeTruthy();
    expect(screen.getByTestId('probe').textContent).toContain('kmh');
    await u.click(screen.getByRole('button', { name: 'Display' }));
    const menu = screen.getByRole('group', { name: 'Display menu' });
    expect((within(menu).getByRole('radio', { name: 'km/h' }) as HTMLInputElement).checked).toBe(true);
    expect((within(menu).getByRole('radio', { name: '°C' }) as HTMLInputElement).checked).toBe(true);
    expect((within(menu).getByRole('radio', { name: 'psi' }) as HTMLInputElement).checked).toBe(true);
    await u.click(within(menu).getByRole('radio', { name: 'mph' }));
    await u.click(within(menu).getByRole('radio', { name: 'bar' }));
    expect(view.device.getSnapshot().units).toEqual({ speed: 'mph', temperature: 'c', pressure: 'bar' });
    expect(view.device.storage.getItem('pitwall.units')).toContain('mph');
    expect(screen.getByTestId('probe').textContent).toContain('mph');
  });
});

describe('density follows the tile’s real size', () => {
  test('a 9×16 tile on the narrowest wide screen reaches Full: the Race preset’s timing tower', async () => {
    const { densityRulesFromCells } = await import('../layout/screen');
    const tower = createRegistry([
      {
        declaration: {
          type: 'tower-probe',
          name: 'Tower probe',
          binding: 'session-wide',
          rate: 'slow',
          minSize: { w: 4, h: 6 },
          defaultSize: { w: 6, h: 12 },
          densities: densityRulesFromCells([
            { density: 'compact', w: 0, h: 0 },
            { density: 'regular', w: 5, h: 8 },
            { density: 'full', w: 9, h: 16 },
          ]),
          reads: [],
        },
        Component: ({ density }) => <p data-testid="density">{density}</p>,
      },
    ]);
    const race = layoutOf('Race copy');
    race.arrangements.wide = [tile('tower-probe', 'tower-probe', 0, 0, 9, 16)];
    // 1280 px viewport with a 15 px scrollbar: the narrowest container a wide screen gives.
    const view = mount({ registry: tower, extra: [race], stored: { 'pitwall.layout': 'Race copy' }, viewport: 1280 });
    Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => 1280 - 15 - 16 });
    await ready(view);
    expect(screen.getByTestId('density').textContent).toBe('full');
  });
});

describe('layout switcher', () => {
  const open = async (view: ReturnType<typeof mount>) => {
    await ready(view);
    const u = user();
    await u.click(screen.getByRole('button', { name: /Layout\s/ }));
    return { u, panel: screen.getByRole('group', { name: 'Layouts' }) };
  };

  test('lists every layout, presets marked, and switches on choice', async () => {
    const view = mount({ extra: [layoutOf('Mine')] });
    const { u, panel } = await open(view);
    const choices = [...panel.querySelectorAll('.pw-menu-choice')].map((b) => b.textContent?.replace(/\s+/g, ' ').trim());
    expect(choices).toEqual(['Pitwall Classic Preset', 'Driver Preset', 'Race Preset', 'Mine']);
    await u.click(within(panel).getByRole('button', { name: 'Race Preset' }));
    await waitFor(() => expect(view.device.getSnapshot().layout).toBe('Race'));
    expect(screen.queryByRole('group', { name: 'Layouts' })).toBeNull();
  });

  test('presets offer Duplicate only: they cannot be renamed or deleted', async () => {
    const view = mount({ extra: [layoutOf('Mine')] });
    const { panel } = await open(view);
    expect(within(panel).getByRole('button', { name: 'Duplicate Race' })).toBeTruthy();
    expect(within(panel).queryByRole('button', { name: 'Rename Race' })).toBeNull();
    expect(within(panel).queryByRole('button', { name: 'Delete Race' })).toBeNull();
    expect(within(panel).getByRole('button', { name: 'Rename Mine' })).toBeTruthy();
    expect(within(panel).getByRole('button', { name: 'Delete Mine' })).toBeTruthy();
  });

  test('Duplicate switches to the copy', async () => {
    const view = mount();
    const { u, panel } = await open(view);
    await u.click(within(panel).getByRole('button', { name: 'Duplicate Driver' }));
    await waitFor(() => expect(screen.getByRole('button', { name: /Layout\s*Driver copy/ })).toBeTruthy());
  });

  test('Rename takes a new name', async () => {
    const view = mount({ extra: [layoutOf('Mine')], stored: { 'pitwall.layout': 'Mine' } });
    const { u, panel } = await open(view);
    await u.click(within(panel).getByRole('button', { name: 'Rename Mine' }));
    const input = within(panel).getByLabelText('New name for Mine');
    await u.clear(input);
    await u.type(input, 'Sunday{Enter}');
    await waitFor(() => expect(screen.getByRole('button', { name: /Layout\s*Sunday/ })).toBeTruthy());
    expect(view.server.layouts.has('Sunday')).toBe(true);
  });

  test('Delete asks first, then falls back to Pitwall Classic', async () => {
    const view = mount({ extra: [layoutOf('Mine')], stored: { 'pitwall.layout': 'Mine' } });
    const { u, panel } = await open(view);
    await u.click(within(panel).getByRole('button', { name: 'Delete Mine' }));
    expect(view.server.layouts.has('Mine')).toBe(true);
    await u.click(within(panel).getByRole('button', { name: 'Delete' }));
    await waitFor(() => expect(screen.getByRole('button', { name: /Layout\s*Pitwall Classic/ })).toBeTruthy());
    expect(view.server.layouts.has('Mine')).toBe(false);
  });

  test('Keep backs out of a delete', async () => {
    const view = mount({ extra: [layoutOf('Mine')] });
    const { u, panel } = await open(view);
    await u.click(within(panel).getByRole('button', { name: 'Delete Mine' }));
    await u.click(within(panel).getByRole('button', { name: 'Keep' }));
    expect(within(panel).getByRole('button', { name: 'Delete Mine' })).toBeTruthy();
    expect(view.server.layouts.has('Mine')).toBe(true);
  });

  test('Escape closes the panel and returns focus to its button', async () => {
    const view = mount();
    const { u } = await open(view);
    await u.keyboard('{Escape}');
    expect(screen.queryByRole('group', { name: 'Layouts' })).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole('button', { name: /Layout\s/ }));
  });

  test('the switcher is off while editing', async () => {
    const view = mount({ extra: [layoutOf('Mine')], stored: { 'pitwall.layout': 'Mine' } });
    await ready(view);
    await user().click(screen.getByRole('button', { name: 'Edit' }));
    expect((screen.getByRole('button', { name: /Layout\s/ }) as HTMLButtonElement).disabled).toBe(true);
  });
});

describe('several screens', () => {
  test('a layout deleted elsewhere moves this screen to Pitwall Classic with a dismissable notice', async () => {
    const view = mount({ extra: [layoutOf('Mine')], stored: { 'pitwall.layout': 'Mine' } });
    await ready(view);
    act(() => view.sendState(session({ context: { revisions: { layouts: 0, tracks: 0 } } })));
    view.server.layouts.delete('Mine');
    view.server.revision++;
    act(() => view.sendState(session({ context: { revisions: { layouts: 1, tracks: 0 } } }), 1));
    const notice = await screen.findByText(/"Mine" was deleted or renamed on another screen. Showing Pitwall Classic./);
    expect(screen.getByRole('button', { name: /Layout\s*Pitwall Classic/ })).toBeTruthy();
    await user().click(within(notice.parentElement!).getByRole('button', { name: 'Dismiss' }));
    expect(screen.queryByText(/deleted or renamed/)).toBeNull();
  });

  test('a layout saved elsewhere updates this screen live', async () => {
    const view = mount({ extra: [layoutOf('Mine')], stored: { 'pitwall.layout': 'Mine' } });
    await ready(view);
    act(() => view.sendState(session()));
    expect(document.querySelectorAll('.pw-tile')).toHaveLength(2);
    const theirs = layoutOf('Mine');
    theirs.arrangements.wide = [tile('notice', 'notice', 0, 0, 24, 1)];
    view.server.layouts.set('Mine', theirs);
    view.server.revision++;
    act(() => view.sendState(session({ context: { revisions: { layouts: 1, tracks: 0 } } })));
    await waitFor(() => expect(document.querySelectorAll('.pw-tile')).toHaveLength(1));
  });

  test('an editor sees a notice when another screen saves the layout, and keeps their draft', async () => {
    const view = mount({ extra: [layoutOf('Mine')], stored: { 'pitwall.layout': 'Mine' } });
    await ready(view);
    act(() => view.sendState(session()));
    const u = user();
    await u.click(screen.getByRole('button', { name: 'Edit' }));
    await u.click(screen.getByRole('button', { name: 'Remove future-tile' }));
    const theirs = layoutOf('Mine');
    theirs.arrangements.wide = [tile('notice', 'notice', 0, 5, 24, 1)];
    view.server.layouts.set('Mine', theirs);
    view.server.revision++;
    act(() => view.sendState(session({ context: { revisions: { layouts: 1, tracks: 0 } } })));
    expect(await screen.findByText(/was saved on another screen. Done will replace it/)).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Done' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Remove future-tile' })).toBeNull();
  });
});

describe('loading', () => {
  test('an unreachable collector offers Try again', async () => {
    const view = mount();
    view.server.list = async () => {
      throw new Error('down');
    };
    await ready(view);
    expect(await screen.findByText('Could not load layouts from the collector.')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeTruthy();
  });
});
