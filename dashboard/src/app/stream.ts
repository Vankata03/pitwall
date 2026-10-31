import type { Units } from '../device/units';
import type { ScreenClass } from '../layout/screen';

export interface StreamOptions {
  enabled: boolean;
  layout: string | null;
  screenClass?: ScreenClass;
  units: Partial<Units>;
}

/** URL values use the same tokens as device units: kmh/mph, c/f, psi/bar. */
export function streamFromUrl(search: string): StreamOptions {
  const params = new URLSearchParams(search);
  const screenClass = params.get('class');
  const speed = params.get('speed');
  const temperature = params.get('temp');
  const pressure = params.get('pressure');
  return {
    enabled: params.get('stream') === '1',
    layout: params.get('layout'),
    screenClass: screenClass === 'wide' || screenClass === 'tablet' || screenClass === 'phone' ? screenClass : undefined,
    units: {
      ...(speed === 'kmh' || speed === 'mph' ? { speed } : {}),
      ...(temperature === 'c' || temperature === 'f' ? { temperature } : {}),
      ...(pressure === 'psi' || pressure === 'bar' ? { pressure } : {}),
    },
  };
}

export function streamUnits(options: StreamOptions, device: Units): Units {
  return { ...device, ...options.units };
}
