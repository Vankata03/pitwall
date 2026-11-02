import { useSlow } from '../runtime/react';
import { raceStatus } from '../session/race-status';
import { sessionName, trackName } from '../session/names';

/** The header strip: the race status, where and what the session is, and the focus car's lap. */
export function RaceStrip({ focusIndex }: { focusIndex: number | null }) {
  const state = useSlow((s) => s.state);
  const history = useSlow((s) => s.history);
  const session = state?.session && 'value' in state.session ? state.session.value : null;
  const lapReadout = focusIndex === null ? undefined : state?.cars[focusIndex]?.lap;
  const lap = lapReadout && 'value' in lapReadout ? lapReadout.value.current_lap_num : null;
  const place = [trackName(session?.trackId), sessionName(session?.sessionType)].filter(Boolean).join(' / ');
  const total = session && session.totalLaps > 0 ? ` / ${session.totalLaps}` : '';
  return (
    <div className="pw-race-strip" data-testid="race-strip">
      <b className="pw-race-strip-name">Session</b>
      <span data-testid="race-status">{session ? raceStatus(session, history) || 'Racing' : 'No session'}</span>
      {place ? <span className="pw-race-strip-quiet">{place}</span> : null}
      {lap !== null ? <span className="pw-num">Lap {lap}{total}</span> : null}
    </div>
  );
}
