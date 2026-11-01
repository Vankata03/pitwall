import type { Readout } from '../../generated/wire/Readout';
import { useSlow } from '../../runtime/react';
import { carLabel, participantsOf } from '../../session/cars';
import { defineTileModule, type TileProps } from '../contract';
import { boundCar, mapValue, SlowTile } from '../slow';

export default defineTileModule({
  declaration: {
    type: 'gaps', name: 'Gaps', binding: 'follows-car', rate: 'slow',
    minSize: { w: 4, h: 2 }, defaultSize: { w: 8, h: 2 },
    densities: [{ density: 'compact', minWidth: 0, minHeight: 0 }, { density: 'regular', minWidth: 320, minHeight: 64 }],
    reads: [{ source: 'state', keys: ['cars.lap', 'cars.gaps'] }],
  },
  Component: function Gaps({ car }: TileProps) {
    const snapshot = useSlow(s => s);
    const c = boundCar(snapshot, car);
    const ahead = mapValue(c?.gaps, g => g.to_car_in_front_ms === null ? null : `+${(g.to_car_in_front_ms / 1000).toFixed(3)} s`);
    let behind: Readout<string> = { state: 'absent' };
    if (c?.lap && 'value' in c.lap) {
      const position = c.lap.value.car_position;
      const next = snapshot.state?.cars?.find(other => other?.lap && 'value' in other.lap && other.lap.value.car_position === position + 1);
      behind = mapValue(next?.gaps, g => g.to_car_in_front_ms === null ? null : `−${(g.to_car_in_front_ms / 1000).toFixed(3)} s`);
      if ('value' in behind && (c.lap.state === 'stale' || next?.lap.state === 'stale')) behind = { state: 'stale', value: behind.value };
    }
    const position = c?.lap && 'value' in c.lap ? c.lap.value.car_position : 0;
    const who = (at: number) => {
      const other = position > 0 ? participantsOf(snapshot.state).find(p => p.position === at) : undefined;
      return other ? `P${at} · ${carLabel(other)}` : undefined;
    };
    return <SlowTile title="Gaps" snapshot={snapshot} rows={[
      { label: 'Ahead', value: ahead, note: who(position - 1) },
      { label: 'Behind', value: behind, note: who(position + 1) },
    ]} />;
  },
});
