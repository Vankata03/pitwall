/** Test-only helpers: hand-built wire buffers. */

/** Hand-built Frame per spec section 8: 16-byte header, 24 x 40-byte entries, little-endian. */
export function buildFrame(opts: {
  epoch: number;
  seq: number;
  sessionTime: number;
  cars?: Record<number, Partial<Record<string, number>>>;
}): ArrayBuffer {
  const buf = new ArrayBuffer(976);
  const v = new DataView(buf);
  v.setUint32(0, opts.epoch, true);
  v.setUint32(4, opts.seq, true);
  v.setFloat64(8, opts.sessionTime, true);
  // Empty slots read absent throughout: every 2-bit group = 3.
  for (let i = 0; i < 24; i++) v.setUint16(16 + i * 40 + 38, 0x3fff, true);
  for (const [idx, f] of Object.entries(opts.cars ?? {})) {
    const o = 16 + Number(idx) * 40;
    if (f.speed !== undefined) v.setUint16(o + 0, f.speed, true);
    if (f.gear !== undefined) v.setInt8(o + 2, f.gear);
    if (f.throttle !== undefined) v.setUint8(o + 3, f.throttle);
    if (f.brake !== undefined) v.setUint8(o + 4, f.brake);
    if (f.steering !== undefined) v.setInt8(o + 5, f.steering);
    if (f.rpm !== undefined) v.setUint16(o + 6, f.rpm, true);
    if (f.worldX !== undefined) v.setFloat32(o + 8, f.worldX, true);
    if (f.worldZ !== undefined) v.setFloat32(o + 12, f.worldZ, true);
    if (f.lapDistance !== undefined) v.setFloat32(o + 16, f.lapDistance, true);
    if (f.currentLapTime !== undefined) v.setUint32(o + 20, f.currentLapTime, true);
    if (f.deltaBest !== undefined) v.setInt32(o + 24, f.deltaBest, true);
    if (f.deltaPrevious !== undefined) v.setInt32(o + 28, f.deltaPrevious, true);
    if (f.gLateral !== undefined) v.setInt16(o + 32, f.gLateral, true);
    if (f.gLongitudinal !== undefined) v.setInt16(o + 34, f.gLongitudinal, true);
    if (f.ersPercent !== undefined) v.setUint8(o + 36, f.ersPercent);
    if (f.aeroState !== undefined) v.setUint8(o + 37, f.aeroState);
    if (f.readout !== undefined) v.setUint16(o + 38, f.readout, true);
  }
  return buf;
}

export function hello(epoch: number, buildId = 'b1', source: 'live' | 'replay' = 'live'): string {
  return JSON.stringify({ type: 'hello', buildId, epoch, source });
}
export function snapshot(
  epoch: number,
  state: Record<string, unknown> = {},
  history: { kind: string; id: string; [k: string]: unknown }[] = [],
  source: 'live' | 'replay' = 'live',
): string {
  return JSON.stringify({ type: 'snapshot', epoch, source, state, history });
}
export function stateMsg(epoch: number, fields: Record<string, unknown>): string {
  return JSON.stringify({ type: 'state', epoch, ...fields });
}
export function historyMsg(epoch: number, entries: { kind: string; id: string; [k: string]: unknown }[]): string {
  return JSON.stringify({ type: 'history', epoch, entries });
}
