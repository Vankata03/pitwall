import type { ConnectionStatus } from './stores';

/** The slice of WebSocket the client uses; lets tests inject a fake. */
export interface SocketLike {
  binaryType: string;
  onopen: (() => void) | null;
  onmessage: ((e: { data: string | ArrayBuffer }) => void) | null;
  onclose: (() => void) | null;
  onerror: (() => void) | null;
  close(): void;
}

const BACKOFF_BASE_MS = 250;
const BACKOFF_CAP_MS = 5000;
const JITTER = 0.2;
/** The collector sends State at 5 Hz, so a few seconds of nothing means it is gone. */
const DEFAULT_SILENCE_MS = 5000;

/** Delay before reconnect attempt `attempt` (0-based): 250 ms doubling to 5 s, +-20% jitter. */
export function backoffDelay(attempt: number, random: () => number = Math.random): number {
  const base = Math.min(BACKOFF_CAP_MS, BACKOFF_BASE_MS * 2 ** Math.min(attempt, 10));
  return Math.round(base * (1 - JITTER + 2 * JITTER * random()));
}

export interface ClientOptions {
  url: string;
  createSocket?: (url: string) => SocketLike;
  onMessage: (data: string | ArrayBuffer) => void;
  onStatus: (status: ConnectionStatus) => void;
  silenceMs?: number;
  random?: () => number;
}

/**
 * One WebSocket to the collector. Reconnects forever with backoff; an open socket that
 * delivers nothing for `silenceMs` counts as unreachable and is replaced.
 */
export class CollectorClient {
  private socket: SocketLike | null = null;
  private retry: ReturnType<typeof setTimeout> | null = null;
  private watchdog: ReturnType<typeof setTimeout> | null = null;
  private attempt = 0;
  private running = false;
  private everConnected = false;

  constructor(private readonly options: ClientOptions) {}

  start(): void {
    if (this.running) return;
    this.running = true;
    this.connect();
  }

  stop(): void {
    this.running = false;
    this.clearTimers();
    this.detach();
  }

  private connect(): void {
    this.options.onStatus(this.everConnected || this.attempt > 0 ? 'reconnecting' : 'connecting');
    const create = this.options.createSocket ?? ((u: string) => new WebSocket(u) as unknown as SocketLike);
    const sock = create(this.options.url);
    sock.binaryType = 'arraybuffer';
    this.socket = sock;
    const live = () => this.socket === sock;
    sock.onopen = () => {
      if (!live()) return;
      this.everConnected = true;
      this.options.onStatus('open');
      this.armWatchdog();
    };
    sock.onmessage = (e) => {
      if (!live()) return;
      this.attempt = 0; // a connection that delivers resets the ladder
      this.armWatchdog();
      this.options.onMessage(e.data);
    };
    sock.onclose = () => {
      if (live()) this.lost();
    };
    sock.onerror = () => {
      // 'close' follows an error; nothing to do here.
    };
  }

  private lost(): void {
    this.clearTimers();
    this.detach();
    if (!this.running) return;
    this.options.onStatus('reconnecting');
    this.retry = setTimeout(() => {
      this.retry = null;
      this.connect();
    }, backoffDelay(this.attempt++, this.options.random));
  }

  private armWatchdog(): void {
    if (this.watchdog !== null) clearTimeout(this.watchdog);
    this.watchdog = setTimeout(() => this.lost(), this.options.silenceMs ?? DEFAULT_SILENCE_MS);
  }

  private clearTimers(): void {
    if (this.retry !== null) clearTimeout(this.retry);
    if (this.watchdog !== null) clearTimeout(this.watchdog);
    this.retry = this.watchdog = null;
  }

  /** Close the current socket so its late events are ignored. */
  private detach(): void {
    const sock = this.socket;
    this.socket = null;
    if (!sock) return;
    sock.onopen = sock.onmessage = sock.onclose = sock.onerror = null;
    try {
      sock.close();
    } catch {
      // already closed
    }
  }
}
