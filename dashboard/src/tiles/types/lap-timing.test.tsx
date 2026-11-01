// @vitest-environment jsdom
import { cleanup, fireEvent, screen } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import type { CompletedLap } from '../../generated/wire/CompletedLap';
import type { LapResult } from '../../generated/wire/LapResult';
import type { TimeTrialDataSet } from '../../generated/wire/TimeTrialDataSet';
import { Absent, carState, liveReadout, liveRuntime, mount, props, readoutWord, Restricted, Stale, type CarState } from '../frame-test-kit';
import tile from './lap-timing';

afterEach(cleanup);
const Tile = tile.Component;
const text = (id: string) => screen.getByTestId(id).textContent;
const state = (id: string) => screen.getByTestId(id).dataset.state;
const MINUS = '−';
const regular = { density: 'regular', width: 340, height: 170 } as const;
const full = { density: 'full', width: 440, height: 260 } as const;

const lapData = (over: Record<string, number> = {}) =>
  liveReadout({
    current_lap_num: 19,
    sector: 0,
    sector1_time_ms_part: 0,
    sector1_time_minutes_part: 0,
    sector2_time_ms_part: 0,
    sector2_time_minutes_part: 0,
    current_lap_invalid: 0,
    ...over,
  } as never);
const lap = (lap_number: number, lap_time_ms: number, sectors_ms: [number, number, number], valid = true): CompletedLap => ({
  lap_number,
  session_time: 0,
  lap_time_ms,
  sectors_ms,
  valid,
  sector_valid: [true, true, true],
  authoritative: true,
});
const result = (lap_number: number, lap_time_ms: number, reference_lap_number: number | null, delta_ms: number | null, sector_deltas_ms: LapResult['sector_deltas_ms']): LapResult => ({
  lap_number,
  valid: true,
  lap_time_ms,
  reference_lap_number,
  delta_ms,
  sector_deltas_ms,
});
const sectorEntry = (car: number, lapNumber: number, sector: number, time_ms: number, valid = true) => ({
  kind: 'sector',
  id: `s-${car}-${lapNumber}-${sector}`,
  car,
  body: { lap_number: lapNumber, sector, session_time: 0, time_ms, valid },
});
const lapEntry = (car: number, l: CompletedLap) => ({ kind: 'lap', id: `l-${car}-${l.lap_number}`, car, body: l });
const timeTrial = (lap_time_in_ms: number, s1: number, s2: number, s3: number): TimeTrialDataSet =>
  ({ car_idx: 0, team_id: 0, lap_time_in_ms, sector1_time_in_ms: s1, sector2_time_in_ms: s2, sector3_time_in_ms: s3 }) as TimeTrialDataSet;

/** A car on lap 19: lap 16 was its best (1:24.565), lap 18 its previous. */
const best = lap(16, 84565, [27150, 28600, 28815]);
const previous = lap(18, 85000, [27400, 28700, 28900]);
const onLap19 = (over: Partial<CarState> = {}) => carState({ lap: lapData(), best, previous, ...over });

const drive = (rt: ReturnType<typeof liveRuntime>, f: Record<string, number>, groups: Parameters<typeof readoutWord>[0] = {}, ts = 16) => {
  rt.frame({ 3: { readout: readoutWord(groups), ...f } });
  rt.paint(ts);
};

test('declares the catalogue entry: follows the car, 60 Hz, 4×3 minimum, 6×4 default', () => {
  expect(tile.declaration).toMatchObject({
    type: 'lap-timing',
    binding: 'follows-car',
    rate: 'frame',
    minSize: { w: 4, h: 3 },
    defaultSize: { w: 6, h: 4 },
  });
});

test('shows the lap so far and the live delta against the best lap, signed', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: onLap19() } });
  mount(rt, <Tile {...props(regular)} />);
  drive(rt, { currentLapTime: 84381, deltaBest: -184, deltaPrevious: 250 });
  expect(text('lt-time')).toBe('1:24.381');
  expect(text('lt-delta')).toBe(`${MINUS}0.184`);
  expect(screen.getByTestId('lt-delta').dataset.sign).toBe('ahead');
  drive(rt, { currentLapTime: 84500, deltaBest: 212, deltaPrevious: 250 }, {}, 33);
  expect(text('lt-delta')).toBe('+0.212');
  expect(screen.getByTestId('lt-delta').dataset.sign).toBe('behind');
});

test('the reference setting picks the live delta: best by default, previous when set', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: onLap19() } });
  const view = mount(rt, <Tile {...props(regular)} />);
  drive(rt, { currentLapTime: 50000, deltaBest: -184, deltaPrevious: 250 });
  expect(text('lt-delta')).toBe(`${MINUS}0.184`);
  expect(text('lt-delta-label')).toMatch(/Best lap/);
  view.rerender(<Tile {...props({ ...regular, settings: { reference: 'previous' } })} />);
  rt.paint(33);
  expect(text('lt-delta')).toBe('+0.250');
  expect(text('lt-delta-label')).toMatch(/Previous lap/);
});

test('a reference value the tile does not know falls back to best, and the setting is left alone', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: onLap19() } });
  mount(rt, <Tile {...props({ ...regular, settings: { reference: 'warp' } })} />);
  drive(rt, { currentLapTime: 50000, deltaBest: -184, deltaPrevious: 250 });
  expect(text('lt-delta')).toBe(`${MINUS}0.184`);
});

test('the four readout states of the live delta and the lap time', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: onLap19() } });
  mount(rt, <Tile {...props(regular)} />);
  const f = { currentLapTime: 84381, deltaBest: -184 };
  drive(rt, f);
  expect([state('lt-delta'), text('lt-delta')]).toEqual(['live', `${MINUS}0.184`]);
  drive(rt, f, { deltaBest: Stale, lap: Stale }, 33);
  expect([state('lt-delta'), text('lt-delta'), state('lt-time')]).toEqual(['stale', `${MINUS}0.184`, 'stale']);
  drive(rt, f, { deltaBest: Restricted, lap: Restricted }, 50);
  expect([state('lt-delta'), text('lt-delta'), text('lt-time')]).toEqual(['restricted', 'Restricted', 'Restricted']);
  // No delta yet, but the lap time still runs: one readout missing does not blank the tile.
  drive(rt, f, { deltaBest: Absent }, 66);
  expect([state('lt-delta'), text('lt-delta'), state('lt-time'), text('lt-time')]).toEqual(['absent', '—', 'live', '1:24.381']);
  expect(screen.getByTestId('tile-message').hidden).toBe(true);
});

test('values read stale while the collector is unreachable', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: onLap19({ lap: lapData({ sector1_time_ms_part: 27102 }) }) } });
  mount(rt, <Tile {...props(regular)} />);
  drive(rt, { currentLapTime: 84381, deltaBest: -184 });
  rt.disconnect();
  rt.paint(33);
  expect([state('lt-delta'), state('lt-time'), state('lt-s1-time')]).toEqual(['stale', 'stale', 'stale']);
});

test('with no readout holding data the whole tile says so and keeps its shape', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: carState() } });
  mount(rt, <Tile {...props(regular)} />);
  drive(rt, {}, { lap: Absent, deltaBest: Absent });
  expect(screen.getByTestId('tile-message').textContent).toBe('No data');
  expect(screen.getByTestId('lt-time')).toBeTruthy();
});

test('an invalid lap keeps its delta running and shows its mark in words', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: onLap19({ lap: lapData({ current_lap_invalid: 1 }) }) } });
  mount(rt, <Tile {...props(regular)} />);
  drive(rt, { currentLapTime: 40000, deltaBest: 120 });
  expect(text('lt-invalid')).toBe('Invalid');
  expect(text('lt-delta')).toBe('+0.120');
});

test('completed sectors show their time and signed delta against the reference lap; the rest wait', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: onLap19({ lap: lapData({ sector: 1, sector1_time_ms_part: 27102 }) }) } });
  const view = mount(rt, <Tile {...props(regular)} />);
  drive(rt, { currentLapTime: 40000, deltaBest: -50, deltaPrevious: -298 });
  expect(text('lt-s1-time')).toMatch(/^27\.102/);
  expect(text('lt-s1-delta')).toBe(`${MINUS}0.048`); // against the best lap's 27.150
  expect([text('lt-s2-time'), text('lt-s2-delta'), text('lt-s3-time')]).toEqual(['—', '—', '—']);
  expect(screen.getByTestId('lt-s2').dataset.current).toBe('true');

  view.rerender(<Tile {...props({ ...regular, settings: { reference: 'previous' } })} />);
  expect(text('lt-s1-delta')).toBe(`${MINUS}0.298`); // against the previous lap's 27.400
});

test('a sector time of a minute or more reads m:ss.mmm', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: onLap19({ lap: lapData({ sector1_time_ms_part: 5000, sector1_time_minutes_part: 1 }) }) } });
  mount(rt, <Tile {...props(regular)} />);
  drive(rt, { currentLapTime: 70000 });
  expect(text('lt-s1-time')).toMatch(/^1:05\.000/);
});

test('sector colours keep the F1 convention and each marked sector carries a glyph too', () => {
  const rt = liveRuntime();
  rt.state(
    {
      cars: {
        3: onLap19({ lap: lapData({ sector: 2, sector1_time_ms_part: 27102, sector2_time_ms_part: 28500 }) }),
      },
    },
    [
      // Car 5 has the session's fastest earlier sector 2; car 3's own best sector 2 was 28600.
      sectorEntry(3, 17, 0, 27300),
      sectorEntry(5, 18, 0, 27200),
      sectorEntry(3, 17, 1, 28600),
      sectorEntry(5, 18, 1, 28400),
      // The sectors being shown must not count against themselves.
      sectorEntry(3, 19, 0, 27102),
      sectorEntry(3, 19, 1, 28500),
    ],
  );
  mount(rt, <Tile {...props(regular)} />);
  drive(rt, { currentLapTime: 60000 });
  expect(screen.getByTestId('lt-s1').dataset.rank).toBe('session-best');
  expect(text('lt-s1-time')).toMatch(/◆/);
  // Slower than car 5's 28.400 but faster than its own 28.600.
  expect(screen.getByTestId('lt-s2').dataset.rank).toBe('personal-best');
  expect(text('lt-s2-time')).toMatch(/●/);
});

test('a sector slower than the car’s own best is the plain slower colour, with no glyph', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: onLap19({ lap: lapData({ sector: 1, sector1_time_ms_part: 27500 }) }) } }, [
    sectorEntry(3, 17, 0, 27300),
    sectorEntry(3, 19, 0, 27500),
  ]);
  mount(rt, <Tile {...props(regular)} />);
  drive(rt, { currentLapTime: 40000 });
  expect(screen.getByTestId('lt-s1').dataset.rank).toBe('slower');
  expect(text('lt-s1-time')).toBe('27.500');
});

test('an invalid sector can be neither best', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: onLap19({ lap: lapData({ sector: 1, sector1_time_ms_part: 27000 }) }) } }, [
    sectorEntry(3, 17, 0, 27300),
    sectorEntry(3, 19, 0, 27000, false),
  ]);
  mount(rt, <Tile {...props(regular)} />);
  drive(rt, { currentLapTime: 40000 });
  expect(screen.getByTestId('lt-s1').dataset.rank).toBe('slower');
});

// ---- The completed result ------------------------------------------------------

const justFinished = () =>
  onLap19({
    // Lap 18 has just been completed and sector 1 of lap 19 has not.
    lap: lapData({ current_lap_num: 19 }),
    best: lap(18, 84381, [27102, 28491, 28788]),
    previous: lap(18, 84381, [27102, 28491, 28788]),
    completedBest: result(18, 84381, 16, -184, [-48, -109, -27]),
    completedPrevious: result(18, 84381, 17, -619, [-298, -209, -112]),
  });

test('the completed result replaces the running lap: official time, delta and sectors', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: justFinished() } }, [lapEntry(3, lap(16, 84565, [27150, 28600, 28815]))]);
  mount(rt, <Tile {...props(regular)} />);
  // The game's clock has already restarted for lap 19.
  drive(rt, { currentLapTime: 1200, deltaBest: -184 });
  expect(text('lt-time')).toBe('1:24.381');
  expect(text('lt-lap-label')).toBe('Lap 18 complete');
  expect(text('lt-delta')).toBe(`${MINUS}0.184`);
  expect([text('lt-s1-time'), text('lt-s2-time'), text('lt-s3-time')].map((t) => t.replace(/[◆●\s]/g, ''))).toEqual(['27.102', '28.491', '28.788']);
  expect([text('lt-s1-delta'), text('lt-s2-delta'), text('lt-s3-delta')]).toEqual([`${MINUS}0.048`, `${MINUS}0.109`, `${MINUS}0.027`]);
});

test('the completed result follows the reference setting', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: justFinished() } });
  mount(rt, <Tile {...props({ ...regular, settings: { reference: 'previous' } })} />);
  drive(rt, { currentLapTime: 1200, deltaPrevious: -619 });
  expect(text('lt-delta')).toBe(`${MINUS}0.619`);
  expect(text('lt-s1-delta')).toBe(`${MINUS}0.298`);
});

test('the completed result stays until sector 1 of the next lap is complete, then the running lap returns', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: justFinished() } });
  mount(rt, <Tile {...props(regular)} />);
  drive(rt, { currentLapTime: 1200, deltaBest: -184 });
  drive(rt, { currentLapTime: 15000, deltaBest: -90 }, {}, 33);
  expect(text('lt-time')).toBe('1:24.381'); // still the result, ticks nothing
  expect(text('lt-lap-label')).toBe('Lap 18 complete');
  expect(text('lt-s1-time')).toMatch(/^27\.102/); // sectors stay with the result
  expect(text('lt-delta')).toBe(`${MINUS}0.090`); // but the headline delta is live again

  rt.state({
    cars: {
      3: onLap19({
        lap: lapData({ sector: 1, sector1_time_ms_part: 27300 }),
        best: lap(18, 84381, [27102, 28491, 28788]),
        previous: lap(18, 84381, [27102, 28491, 28788]),
      }),
    },
  });
  drive(rt, { currentLapTime: 28000, deltaBest: 120 }, {}, 50);
  expect(text('lt-lap-label')).toBe('Lap 19');
  expect(text('lt-time')).toBe('0:28.000');
  expect(text('lt-delta')).toBe('+0.120');
  expect(text('lt-s1-time')).toMatch(/^27\.300/);
  expect(text('lt-s1-delta')).toBe('+0.198'); // against the new best lap, 18
  expect(text('lt-s2-time')).toBe('—');
});

test('an invalid completed lap keeps its mark on the result', () => {
  const rt = liveRuntime();
  const car = justFinished();
  car.completedBest = { ...car.completedBest!, valid: false };
  rt.state({ cars: { 3: car } });
  mount(rt, <Tile {...props(regular)} />);
  drive(rt, { currentLapTime: 1200 });
  expect(text('lt-invalid')).toBe('Invalid');
});

// ---- Time Trial ----------------------------------------------------------------

const ttSession = (rt: ReturnType<typeof liveRuntime>, car: CarState) =>
  rt.state({
    session: { state: 'live', value: { trackId: 0, trackLength: 5000, sessionType: 18, weather: 0, trackTemperature: 30, airTemperature: 20, safetyCarStatus: 0, forecast: [] } },
    cars: { 3: car },
  });

test('in Time Trial a personal best or rival can be the reference: plain times, no live delta', () => {
  const rt = liveRuntime();
  ttSession(rt, onLap19({ lap: lapData({ sector: 1, sector1_time_ms_part: 27102 }), personalBest: timeTrial(83000, 27000, 28000, 28000), rival: timeTrial(82500, 26900, 27800, 27800) }));
  const view = mount(rt, <Tile {...props({ ...full, settings: { reference: 'personal-best' } })} />);
  drive(rt, { currentLapTime: 40000, deltaBest: -184 });
  // Nothing is seeded, so there is no distance trace to hold a live delta against.
  expect([state('lt-delta'), text('lt-delta')]).toEqual(['absent', '—']);
  expect(text('lt-s1-delta')).toBe('+0.102'); // 27.102 against the personal best's 27.000
  expect(text('lt-ref-time')).toBe('1:23.000');
  expect(text('lt-delta-label')).toMatch(/Personal best/);
  view.rerender(<Tile {...props({ ...full, settings: { reference: 'rival' } })} />);
  rt.paint(33);
  expect(text('lt-s1-delta')).toBe('+0.202');
  expect(text('lt-ref-time')).toBe('1:22.500');
  expect(text('lt-delta-label')).toMatch(/Rival/);
});

test('in Time Trial the completed lap is compared with the personal best too', () => {
  const rt = liveRuntime();
  ttSession(rt, { ...justFinished(), personalBest: timeTrial(84000, 27000, 28500, 28500), rival: null });
  mount(rt, <Tile {...props({ ...full, settings: { reference: 'personal-best' } })} />);
  drive(rt, { currentLapTime: 1200 });
  expect(text('lt-time')).toBe('1:24.381');
  expect(text('lt-delta')).toBe('+0.381');
  expect([text('lt-s1-delta'), text('lt-s2-delta'), text('lt-s3-delta')]).toEqual(['+0.102', `${MINUS}0.009`, '+0.288']);
});

test('a personal best the game has not sent leaves every delta absent, never zero', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: onLap19({ lap: lapData({ sector: 1, sector1_time_ms_part: 27102 }) }) } });
  mount(rt, <Tile {...props({ ...full, settings: { reference: 'personal-best' } })} />);
  drive(rt, { currentLapTime: 40000 });
  expect([text('lt-delta'), text('lt-s1-delta'), text('lt-ref-time')]).toEqual(['—', '—', '—']);
});

// ---- Densities and settings ----------------------------------------------------

test('compact keeps the delta and the lap time; regular adds sectors; full adds the reference lap', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: onLap19() } });
  const view = mount(rt, <Tile {...props({ density: 'compact', width: 210, height: 112 })} />);
  expect(screen.getByTestId('lt-delta')).toBeTruthy();
  expect(screen.getByTestId('lt-time')).toBeTruthy();
  expect(screen.queryByTestId('lt-s1')).toBeNull();
  expect(screen.queryByTestId('lt-ref-time')).toBeNull();
  view.rerender(<Tile {...props(regular)} />);
  expect(screen.getByTestId('lt-s1')).toBeTruthy();
  expect(screen.queryByTestId('lt-ref-time')).toBeNull();
  view.rerender(<Tile {...props(full)} />);
  expect(text('lt-ref-time')).toBe('1:24.565');
  expect(text('lt-ref-label')).toMatch(/Best lap/);
  expect(text('lt-ref-label')).toMatch(/16/);
});

test('the settings panel picks the reference and leaves unknown settings alone', () => {
  const Settings = tile.Settings!;
  const rt = liveRuntime();
  const saved: Record<string, unknown>[] = [];
  mount(rt, <Settings settings={{ other: 1, car: { target: 'focus' } }} onChange={(s) => saved.push(s)} />);
  const select = screen.getByLabelText('Reference lap') as HTMLSelectElement;
  expect(select.value).toBe('best');
  expect([...select.options].map((o) => o.textContent)).toEqual(['Best lap', 'Previous lap', 'Personal best (Time Trial)', 'Rival (Time Trial)']);
  fireEvent.change(select, { target: { value: 'previous' } });
  expect(saved[0]).toEqual({ other: 1, car: { target: 'focus' }, reference: 'previous' });
});

test('Time Trial reference sectors are shown as plain times alongside the selected lap time', () => {
  const rt = liveRuntime();
  ttSession(rt, onLap19({ personalBest: timeTrial(83000, 27000, 28000, 28000) }));
  mount(rt, <Tile {...props({ ...full, settings: { reference: 'personal-best' } })} />);
  expect([1, 2, 3].map(s => text(`lt-ref-s${s}`))).toEqual(['27.000', '28.000', '28.000']);
});

test('Time Trial reference sectors keep their shape when missing and become stale on disconnect', () => {
  const rt = liveRuntime();
  ttSession(rt, onLap19({ personalBest: null }));
  mount(rt, <Tile {...props({ ...full, settings: { reference: 'personal-best' } })} />);
  expect([state('lt-ref-s1'), text('lt-ref-s1')]).toEqual(['absent', '—']);
  ttSession(rt, onLap19({ personalBest: timeTrial(83000, 27000, 0, 28000) }));
  expect([state('lt-ref-s2'), text('lt-ref-s2')]).toEqual(['absent', '—']);
  rt.disconnect();
  expect(state('lt-ref-s1')).toBe('stale');
});

test('Time Trial references cannot use leftover packet data in another session type', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: onLap19({ personalBest: timeTrial(83000, 27000, 28000, 28000) }) } });
  mount(rt, <Tile {...props({ ...full, settings: { reference: 'personal-best' } })} />);
  drive(rt, { currentLapTime: 40000, deltaBest: -184 });
  expect([text('lt-ref-time'), text('lt-ref-s1'), text('lt-delta')]).toEqual(['—', '—', '—']);
});

test('switching cars replaces the held result and clearing car data removes it', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: justFinished(), 5: carState({ lap: lapData({ current_lap_num: 7, sector: 1, sector1_time_ms_part: 30000 }) }) } });
  const view = mount(rt, <Tile {...props(regular)} />);
  drive(rt, { currentLapTime: 1000 });
  expect(text('lt-lap-label')).toBe('Lap 18 complete');
  view.rerender(<Tile {...props({ ...regular, car: { status: 'ok', index: 5 } })} />);
  rt.frame({ 5: { currentLapTime: 32000, readout: readoutWord({ deltaBest: Absent }) } });
  rt.paint(33);
  expect(text('lt-lap-label')).toBe('Lap 7');
  expect(text('lt-time')).toBe('0:32.000');
  rt.state({ cars: { 5: carState() } });
  rt.frame({ 5: { readout: readoutWord({ lap: Absent, deltaBest: Absent }) } });
  rt.paint(50);
  expect(text('lt-time')).toBe('—');
  expect(text('lt-s1-time')).toBe('—');
});

for (const reference of ['best', 'previous']) {
  test(`held completed time allows live ${reference} delta after five seconds of S1`, () => {
    const rt = liveRuntime();
    ttSession(rt, justFinished());
    mount(rt, <Tile {...props({ ...regular, settings: { reference } })} />);
    drive(rt, { currentLapTime: 5000, deltaBest: -91, deltaPrevious: 123 });
    expect(text('lt-time')).toBe('1:24.381');
    expect(text('lt-delta')).toBe(reference === 'best' ? `${MINUS}0.091` : '+0.123');
    drive(rt, { currentLapTime: 6000 }, { deltaBest: Absent, deltaPrevious: Absent }, 33);
    expect(text('lt-delta')).toBe('—');
  });
}

test('the headline delta holds the official delta for the first five seconds, then runs live', () => {
  const rt = liveRuntime();
  rt.state({ cars: { 3: justFinished() } });
  mount(rt, <Tile {...props(regular)} />);
  drive(rt, { currentLapTime: 100, deltaBest: -5 });
  expect(text('lt-delta')).toBe(`${MINUS}0.184`);
  drive(rt, { currentLapTime: 4999, deltaBest: -90 }, {}, 33);
  expect(text('lt-delta')).toBe(`${MINUS}0.184`);
  drive(rt, { currentLapTime: 5000, deltaBest: -90 }, {}, 50);
  expect(text('lt-delta')).toBe(`${MINUS}0.090`);
  expect(text('lt-time')).toBe('1:24.381');
  expect(text('lt-lap-label')).toBe('Lap 18 complete');
});

test('personal best and rival deltas stay the held completed-lap difference past five seconds', () => {
  for (const reference of ['personal-best', 'rival']) {
    const rt = liveRuntime();
    ttSession(rt, { ...justFinished(), personalBest: timeTrial(84000, 27000, 28500, 28500), rival: timeTrial(84000, 27000, 28500, 28500) });
    mount(rt, <Tile {...props({ ...regular, settings: { reference } })} />);
    drive(rt, { currentLapTime: 15000, deltaBest: -90 });
    expect(text('lt-delta')).toBe('+0.381');
    cleanup();
  }
});
