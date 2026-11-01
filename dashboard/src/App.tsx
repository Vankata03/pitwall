import { StatusPage } from './status/page';
import { Dashboard, type DashboardServices } from './app/dashboard';

export function App(services: DashboardServices) {
  if (globalThis.location?.pathname === '/status') return <StatusPage />;
  return <Dashboard {...services} />;
}
