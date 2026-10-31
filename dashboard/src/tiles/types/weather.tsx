import { temperature } from '../../device/units';
import { useSlow } from '../../runtime/react';
import { defineTileModule, type TileProps } from '../contract';
import { mapValue, SlowTile } from '../slow';

const skies = ['Clear', 'Light cloud', 'Overcast', 'Light rain', 'Heavy rain', 'Storm'];
const sky = (n: number) => skies[n] ?? `Unknown sky (${n})`;
export default defineTileModule({
  declaration: {
    type: 'weather', name: 'Weather', binding: 'session-wide', rate: 'slow',
    minSize: { w: 4, h: 2 }, defaultSize: { w: 6, h: 2 },
    densities: [{ density: 'compact', minWidth: 0, minHeight: 0 }, { density: 'regular', minWidth: 280, minHeight: 104 }],
    reads: [{ source: 'state', keys: ['session'] }],
  },
  Component: function Weather({ units, density }: TileProps) {
    const snapshot = useSlow(s => s);
    const session = snapshot.state?.session;
    const temp = (n: number) => { const q = temperature(n, units); return `${Math.round(q.value)} ${q.unit}`; };
    const forecast = mapValue(session, s => s.forecast.filter(f => f.session_type === s.sessionType).sort((a, b) => a.time_offset - b.time_offset)[0]);
    const rows = [
      { label: 'Air', value: mapValue(session, s => temp(s.airTemperature)) },
      { label: 'Track', value: mapValue(session, s => temp(s.trackTemperature)) },
      { label: 'Rain chance', value: mapValue(forecast, f => `${f.rain_percentage}%`) },
      { label: 'Sky', value: mapValue(session, s => sky(s.weather)) },
      { label: 'Forecast', value: mapValue(forecast, f => density === 'compact' ? `${sky(f.weather)} +${f.time_offset}m` : `+${f.time_offset} min: ${sky(f.weather)}`) },
    ];
    if (density !== 'compact') rows.push({ label: 'Forecast air / track', value: mapValue(forecast, f => `${temp(f.air_temperature)} / ${temp(f.track_temperature)}`) });
    return <SlowTile title="Weather" snapshot={snapshot} rows={rows} />;
  },
});
