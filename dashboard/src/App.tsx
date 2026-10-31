import { Dashboard, type DashboardServices } from './app/dashboard';

export function App(services: DashboardServices) {
  return <Dashboard {...services} />;
}
