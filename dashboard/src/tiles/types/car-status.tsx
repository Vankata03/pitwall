import { CarImage } from '../car-status/car-image';
import '../car-status/car-status.css';
import { carStatusView, type Cell, type ConditionCell, type TemperatureCell, type WheelView } from '../car-status/view';
import { wholeTileMessage } from '../../runtime/readout';
import { useSlow } from '../../runtime/react';
import { lifecycleOf } from '../../runtime/wire';
import type { ParticipantData } from '../../generated/wire/ParticipantData';
import { defineTileModule, type TileProps } from '../contract';

const WHEEL_NAMES = { RL: 'Rear left', RR: 'Rear right', FL: 'Front left', FR: 'Front right' } as const;

function Value({ id, cell, glyph, band }: { id: string; cell: Cell; glyph?: string; band?: string | null }) {
  return (
    <span className="pw-readout pw-cs-value" data-testid={id} data-state={cell.state} data-band={band ?? undefined}>
      {cell.text}
      {cell.state === 'live' || cell.state === 'stale' ? glyph : ''}
    </span>
  );
}

function Column({ id, label, cell, unit }: { id: string; label: string; cell: TemperatureCell; unit: string }) {
  return (
    <div className="pw-cs-col" data-channel={label}>
      <span className="pw-label">{label}</span>
      <span className="pw-cs-reading">
        <Value id={id} cell={cell} glyph={cell.glyph} band={cell.band} />
        <span className="pw-cs-unit">{unit}</span>
      </span>
      <span className="pw-cs-meter" aria-hidden="true">
        <span
          className="pw-cs-meter-fill"
          data-band={cell.band ?? undefined}
          data-state={cell.state}
          style={{ height: `${Math.round((cell.fill ?? 0) * 100)}%` }}
        />
      </span>
    </div>
  );
}

function Condition({ cell }: { cell: ConditionCell }) {
  return (
    <span className="pw-cs-bar" aria-hidden="true">
      <span
        className="pw-cs-bar-fill"
        data-testid="condition-bar"
        data-level={cell.level ?? undefined}
        data-state={cell.state}
        style={{ width: `${cell.percent ?? 0}%` }}
      />
    </span>
  );
}

function Wheel({ wheel, units, full }: { wheel: WheelView; units: { temperature: string; pressure: string }; full: boolean }) {
  return (
    <div className="pw-cs-wheel" data-wheel={wheel.id} role="group" aria-label={`${WHEEL_NAMES[wheel.id]} tyre and brake`}>
      <div className="pw-cs-wheel-head">
        <span className="pw-label">{wheel.id} · Tyre</span>
        <Value id="condition" cell={wheel.condition} />
      </div>
      <Condition cell={wheel.condition} />
      <span className="pw-label pw-cs-caption">Condition remaining</span>
      <div className="pw-cs-cols">
        <Column id="brake" label="BRK" cell={wheel.brake} unit={units.temperature} />
        <Column id="surface" label="SFC" cell={wheel.surface} unit={units.temperature} />
        <Column id="inner" label="INR" cell={wheel.inner} unit={units.temperature} />
      </div>
      <div className="pw-cs-line pw-cs-pressure">
        <span className="pw-label">{units.pressure}</span>
        <Value id="pressure" cell={wheel.pressure} />
      </div>
      {full ? (
        <div className="pw-cs-line">
          <span className="pw-label">Brake damage</span>
          <Value id="brake-damage" cell={wheel.brakeDamage} />
        </div>
      ) : null}
    </div>
  );
}

const WINGS = ['front-right-wing', 'front-left-wing'] as const;

/** The car with tyres, brakes, pressures, damage and tyre age (spec section 11). */
function CarStatus({ density, car, units }: TileProps) {
  const index = car && (car.status === 'ok' || car.status === 'retired') ? car.index : null;
  const carState = useSlow((s) => (index === null ? undefined : (s.state?.cars[index] ?? undefined)));
  const lifecycle = useSlow((s) => lifecycleOf(s.state));
  const connected = useSlow((s) => s.connection === 'open');
  const view = carStatusView(carState, units, connected);
  const message = view.hasData ? null : wholeTileMessage([], { lifecycle, connected });
  const participant: ParticipantData | undefined =
    carState && (carState.participant.state === 'live' || carState.participant.state === 'stale')
      ? carState.participant.value
      : undefined;
  const full = density === 'full';
  const wheel = (id: string) => <Wheel wheel={view.wheels.find((w) => w.id === id)!} units={view.units} full={full} />;
  const damage = (key: string) => view.damage.find((d) => d.key === key)!;
  const rest = view.damage.filter((d) => !(WINGS as readonly string[]).includes(d.key));

  return (
    <section className="pw-cs" data-testid="car-status" data-density={density} aria-label="Car status">
      <header className="pw-cs-head">
        <span className="pw-cs-title">Car status</span>
        {car?.status === 'retired' ? (
          <span className="pw-cs-retired" data-testid="retired">
            Retired
          </span>
        ) : null}
        <span className="pw-cs-tyre">
          <span className="pw-cs-tyre-name">
            <Value id="compound" cell={view.compound} /> <span className="pw-label">tyres</span>
          </span>
          <span className="pw-cs-tyre-age">
            <Value id="age" cell={view.age} /> <span className="pw-label">laps</span>
          </span>
        </span>
      </header>
      {message ? (
        <p className="pw-cs-message" role="status">
          {message}
        </p>
      ) : null}
      <div className="pw-cs-body">
        <div className="pw-cs-slot" data-slot="rr">{wheel('RR')}</div>
        <div className="pw-cs-slot" data-slot="rl">{wheel('RL')}</div>
        <div className="pw-cs-slot" data-slot="fr">{wheel('FR')}</div>
        <div className="pw-cs-slot" data-slot="fl">{wheel('FL')}</div>
        {density === 'compact' ? null : (
          <div className="pw-cs-slot" data-slot="car">
            <CarImage participant={participant} />
          </div>
        )}
        {(['frw', 'flw'] as const).map((slot, i) => {
          const part = damage(WINGS[i]!);
          return (
            <div className="pw-cs-slot pw-cs-line" data-slot={slot} key={slot}>
              <span className="pw-label">{part.label}</span>
              <Value id={`damage-${part.key}`} cell={part.cell} />
              <span className="pw-cs-damage-bar" aria-hidden="true">
                <i style={{ width: `${part.percent ?? 0}%` }} />
              </span>
            </div>
          );
        })}
      </div>
      {full ? (
        <ul className="pw-cs-damage" aria-label="Damage">
          {rest.map((part) => (
            <li className="pw-cs-line" key={part.key}>
              <span className="pw-label">{part.label}</span>
              <Value id={`damage-${part.key}`} cell={part.cell} />
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}

export default defineTileModule({
  declaration: {
    type: 'car-status',
    name: 'Car status',
    binding: 'follows-car',
    rate: 'slow',
    minSize: { w: 8, h: 6 },
    defaultSize: { w: 12, h: 12 },
    densities: [
      { density: 'compact', minWidth: 0, minHeight: 0 },
      { density: 'regular', minWidth: 560, minHeight: 380 },
      { density: 'full', minWidth: 800, minHeight: 600 },
    ],
    reads: [{ source: 'state', keys: ['cars'] }],
  },
  Component: CarStatus,
});
