import { useSlow } from '../../runtime/react';
import { defineTileModule, type TileProps } from '../contract';
import { boundCar, mapValue, SlowTile } from '../slow';

export default defineTileModule({
  declaration: {
    type: 'lap-state', name: 'Lap state', binding: 'follows-car', rate: 'slow',
    minSize: { w: 4, h: 2 }, defaultSize: { w: 6, h: 2 },
    densities: [{ density: 'compact', minWidth: 0, minHeight: 0 }, { density: 'regular', minWidth: 280, minHeight: 64 }],
    reads: [{ source: 'state', keys: ['cars.lap', 'session'] }],
  },
  Component: function LapState({ car }: TileProps) {
    const snapshot = useSlow(s => s);
    const lap = boundCar(snapshot, car)?.lap;
    const session = snapshot.state?.session;
    return <SlowTile title="Lap state" snapshot={snapshot} rows={[
      { label: 'Lap', value: mapValue(lap, l => String(l.current_lap_num)) },
      { label: 'Total', value: mapValue(session, s => s.totalLaps > 0 ? String(s.totalLaps) : null) },
      { label: 'Position', value: mapValue(lap, l => l.car_position > 0 ? String(l.car_position) : null) },
      { label: 'Sector', value: mapValue(lap, l => l.sector < 3 ? String(l.sector + 1) : null) },
      { label: 'Pit status', value: mapValue(lap, l => ['On track', 'Pitting', 'In pit'][l.pit_status]) },
    ]} />;
  },
});
