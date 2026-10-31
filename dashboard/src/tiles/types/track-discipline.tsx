import { useSlow } from '../../runtime/react';
import { defineTileModule, type TileProps } from '../contract';
import { boundCar, mapValue, SlowTile } from '../slow';

export default defineTileModule({
  declaration: {
    type: 'track-discipline', name: 'Track discipline', binding: 'follows-car', rate: 'slow',
    minSize: { w: 4, h: 2 }, defaultSize: { w: 6, h: 3 },
    densities: [{ density: 'compact', minWidth: 0, minHeight: 0 }, { density: 'regular', minWidth: 240, minHeight: 104 }],
    reads: [{ source: 'state', keys: ['cars.penalties', 'cars.lap'] }],
  },
  Component: function TrackDiscipline({ car, density }: TileProps) {
    const snapshot = useSlow(s => s);
    const c = boundCar(snapshot, car);
    const rows = [
      { label: 'Corner cuts', value: mapValue(c?.penalties, p => String(p.corner_cutting_warnings)) },
      { label: 'Warnings', value: mapValue(c?.penalties, p => String(p.total_warnings)) },
      { label: 'Penalties', value: mapValue(c?.penalties, p => `${p.seconds} s`) },
      { label: 'Lap validity', value: mapValue(c?.lap, l => l.current_lap_invalid ? 'Invalid' : 'Valid') },
    ];
    if (density !== 'compact') rows.push(
      { label: 'Drive-through', value: mapValue(c?.penalties, p => String(p.drive_throughs)) },
      { label: 'Stop-go', value: mapValue(c?.penalties, p => String(p.stop_gos)) },
    );
    return <SlowTile title="Track discipline" snapshot={snapshot} rows={rows} />;
  },
});
