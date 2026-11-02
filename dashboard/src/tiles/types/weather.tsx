import { temperature } from '../../device/units';
import { useSlow } from '../../runtime/react';
import { defineTileModule, type TileProps } from '../contract';
import { mapValue, SlowTile } from '../slow';

const skies = ['Clear', 'Light cloud', 'Overcast', 'Light rain', 'Heavy rain', 'Storm'];
const sky = (n: number) => skies[n] ?? `Unknown sky (${n})`;

/** Outline glyphs for the sky, drawn in the value's colour. Decorative: the word says it. */
function SkyIcon({ weather }: { weather: number }) {
  const sun = (
    <g>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6l1.4 1.4M17 17l1.4 1.4M5.6 18.4 7 17M17 7l1.4-1.4" />
    </g>
  );
  const cloud = <path d="M7 18h10a4 4 0 0 0 .4-8 5.5 5.5 0 0 0-10.6 1.6A3.2 3.2 0 0 0 7 18Z" />;
  const glyph =
    weather === 0 ? sun
    : weather === 1 ? <g><g transform="translate(-3 -3) scale(.75)">{sun}</g><g transform="translate(3 2) scale(.85)">{cloud}</g></g>
    : weather === 2 ? cloud
    : weather === 5 ? <g><g transform="translate(0 -3)">{cloud}</g><path d="m12 16-2 4h3l-2 3" /></g>
    : <g><g transform="translate(0 -3)">{cloud}</g><path d={weather === 4 ? 'M8 18l-1 3M12 18l-1 3M16 18l-1 3' : 'M9 18l-1 2.5M14 18l-1 2.5'} /></g>;
  return (
    <svg viewBox="0 0 24 24" width="28" height="28" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
      {glyph}
    </svg>
  );
}
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
      {
        label: 'Sky', value: mapValue(session, s => sky(s.weather)), tone: 'accent' as const,
        icon: session && 'value' in session ? <SkyIcon weather={session.value.weather} /> : undefined,
      },
      { label: 'Forecast', value: mapValue(forecast, f => density === 'compact' ? `${sky(f.weather)} +${f.time_offset}m` : `+${f.time_offset} min: ${sky(f.weather)}`) },
    ];
    if (density !== 'compact') rows.push({ label: 'Forecast air / track', value: mapValue(forecast, f => `${temp(f.air_temperature)} / ${temp(f.track_temperature)}`) });
    return <SlowTile title="Weather" snapshot={snapshot} rows={rows} />;
  },
});
