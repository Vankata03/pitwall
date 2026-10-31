import type { ConnectionStatus } from './stores';
import type { Lifecycle } from './wire';

export interface Banner {
  id: 'connecting' | 'collector-unreachable' | 'game-not-sending' | 'reload';
  text: string;
  /** Second line, for setup guidance. */
  hint?: string;
  action?: 'reload';
}

export interface BannerInput {
  connection: ConnectionStatus;
  lifecycle: Lifecycle | undefined;
  reloadPrompt: boolean;
}

const SETUP_HINT =
  'In the game, turn UDP telemetry on, set UDP Format to 2025 or 2026, port 20777, and send to broadcast or this computer’s IP.';

/**
 * Banners for the connection lifecycle (spec section 2). One cause at a time: while the
 * collector is unreachable nothing is known about the game, so no game banner shows.
 */
export function deriveBanners(input: BannerInput): Banner[] {
  if (input.connection === 'connecting') {
    return [{ id: 'connecting', text: 'Connecting to the collector' }];
  }
  if (input.connection === 'reconnecting') {
    return [{ id: 'collector-unreachable', text: 'Collector unreachable. Reconnecting' }];
  }
  const out: Banner[] = [];
  if (input.lifecycle === 'waiting') {
    out.push({ id: 'game-not-sending', text: 'Waiting for the game', hint: SETUP_HINT });
  } else if (input.lifecycle === 'lost') {
    out.push({ id: 'game-not-sending', text: 'No data: game paused or closed', hint: SETUP_HINT });
  }
  if (input.reloadPrompt) {
    out.push({ id: 'reload', text: 'A new version of pitwall is available', action: 'reload' });
  }
  return out;
}
