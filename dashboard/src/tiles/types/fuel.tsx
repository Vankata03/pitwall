import { useSlow } from '../../runtime/react';
import { defineTileModule, type TileProps } from '../contract';
import { boundCar, mapValue, SlowTile } from '../slow';

export default defineTileModule({
  declaration: {
    type: 'fuel', name: 'Fuel', binding: 'follows-car', rate: 'slow',
    minSize: { w: 3, h: 2 }, defaultSize: { w: 6, h: 2 },
    densities: [{ density: 'compact', minWidth: 0, minHeight: 0 }, { density: 'regular', minWidth: 240, minHeight: 64 }],
    reads: [{ source: 'state', keys: ['cars.fuel'] }],
  },
  Component: function Fuel({ car }: TileProps) {
    const snapshot = useSlow(s => s);
    const fuel = boundCar(snapshot, car)?.fuel;
    const delta = mapValue(fuel, f => `${f.remaining_laps < 0 ? '−' : '+'}${Math.abs(f.remaining_laps).toFixed(2)} laps`);
    const short = fuel && 'value' in fuel && fuel.value.remaining_laps < 0;
    return <SlowTile title="Fuel" snapshot={snapshot} rows={[
      { label: 'Fuel delta', value: delta, tone: short ? 'bad' : 'good' },
      { label: 'Fuel in tank', value: mapValue(fuel, f => `${f.in_tank.toFixed(1)} kg`) },
    ]} />;
  },
});
