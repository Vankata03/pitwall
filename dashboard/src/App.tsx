import { StatusPage } from './status/page';
import { ThrottleBrakeOverlay } from './overlay/throttle-brake';
import { Dashboard, type DashboardServices } from './app/dashboard';

/** The page the collector's native overlay window loads. */
export const OVERLAY_THROTTLE_BRAKE = '/overlay/throttle-brake';

export function App(services: DashboardServices) {
  if (globalThis.location?.pathname === '/status') return <StatusPage />;
  if (globalThis.location?.pathname === OVERLAY_THROTTLE_BRAKE) return <ThrottleBrakeOverlay />;
  return <Dashboard {...services} />;
}
