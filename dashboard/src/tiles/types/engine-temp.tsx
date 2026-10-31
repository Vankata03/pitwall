import { temperature } from '../../device/units';
import { useSlow } from '../../runtime/react';
import { defineTileModule, type TileProps } from '../contract';
import { boundCar, mapValue, SlowTile } from '../slow';

export default defineTileModule({
  declaration: {
    type: 'engine-temp', name: 'Engine temp', binding: 'follows-car', rate: 'slow',
    minSize: { w: 2, h: 2 }, defaultSize: { w: 3, h: 2 },
    densities: [{ density: 'compact', minWidth: 0, minHeight: 0 }, { density: 'regular', minWidth: 140, minHeight: 64 }],
    reads: [{ source: 'state', keys: ['cars.telemetry'] }],
  },
  Component: function EngineTemp({ car, units }: TileProps) {
    const snapshot = useSlow(s => s);
    const value = mapValue(boundCar(snapshot, car)?.telemetry, t => {
      const q = temperature(t.engine_temperature, units);
      return `${Math.round(q.value)} ${q.unit}`;
    });
    return <SlowTile title="Engine temp" snapshot={snapshot} rows={[{ label: 'Engine temperature', value }]} />;
  },
});
