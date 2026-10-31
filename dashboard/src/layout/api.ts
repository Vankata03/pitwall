import type { Layout } from './model';

export interface LayoutSummary {
  name: string;
  preset: boolean;
}

export interface LayoutList {
  /** Advances on every successful change anywhere; State carries the same counter. */
  revision: number;
  layouts: LayoutSummary[];
}

export class LayoutsApiError extends Error {
  /** HTTP status, or 0 when the collector could not be reached. */
  readonly status: number;
  constructor(status: number, message: string) {
    super(message);
    this.name = 'LayoutsApiError';
    this.status = status;
  }
}

export interface LayoutsApi {
  list(): Promise<LayoutList>;
  /** Null when no layout has that name. */
  get(name: string): Promise<Layout | null>;
  /** Replace the whole layout, creating it if new. */
  save(name: string, body: unknown): Promise<Layout>;
  duplicate(name: string, newName?: string): Promise<Layout>;
  rename(name: string, newName: string): Promise<Layout>;
  remove(name: string): Promise<void>;
}

type Fetch = typeof globalThis.fetch;

const path = (name: string) => `/api/layouts/${encodeURIComponent(name)}`;

/**
 * Client for the layout routes. The collector refuses any non-GET request without
 * `Content-Type: application/json`, DELETE included, so every mutation sends it.
 */
export function createLayoutsApi(fetchImpl: Fetch = (...args) => globalThis.fetch(...args)): LayoutsApi {
  async function send(url: string, method: string, body?: unknown): Promise<Response> {
    const mutating = method !== 'GET';
    try {
      return await fetchImpl(url, {
        method,
        headers: mutating ? { 'Content-Type': 'application/json' } : undefined,
        body: body === undefined ? undefined : JSON.stringify(body),
      });
    } catch {
      throw new LayoutsApiError(0, 'The collector could not be reached');
    }
  }

  async function fail(response: Response): Promise<never> {
    let message = `Request failed (${response.status})`;
    try {
      const data = (await response.json()) as { error?: unknown };
      if (typeof data.error === 'string') message = data.error;
    } catch {
      // No JSON body: keep the status message.
    }
    throw new LayoutsApiError(response.status, message);
  }

  async function json<T>(response: Response): Promise<T> {
    if (!response.ok) return fail(response);
    return (await response.json()) as T;
  }

  return {
    async list() {
      return json<LayoutList>(await send('/api/layouts', 'GET'));
    },
    async get(name) {
      const response = await send(path(name), 'GET');
      if (response.status === 404) return null;
      return json<Layout>(response);
    },
    async save(name, body) {
      return json<Layout>(await send(path(name), 'PUT', body));
    },
    async duplicate(name, newName) {
      return json<Layout>(await send(`${path(name)}/duplicate`, 'POST', newName === undefined ? {} : { name: newName }));
    },
    async rename(name, newName) {
      return json<Layout>(await send(`${path(name)}/rename`, 'POST', { name: newName }));
    },
    async remove(name) {
      const response = await send(path(name), 'DELETE');
      if (!response.ok) await fail(response);
    },
  };
}
