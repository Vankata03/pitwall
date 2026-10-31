import { describe, expect, test } from 'vitest';
import { createLayoutsApi, LayoutsApiError } from './api';

interface Call {
  url: string;
  method: string;
  contentType: string | null;
  body: unknown;
}

function fakeFetch(respond: (call: Call) => { status: number; body?: unknown }) {
  const calls: Call[] = [];
  const fetch = async (input: string, init: RequestInit = {}) => {
    const headers = new Headers(init.headers);
    const call: Call = {
      url: input,
      method: init.method ?? 'GET',
      contentType: headers.get('content-type'),
      body: typeof init.body === 'string' ? JSON.parse(init.body) : undefined,
    };
    calls.push(call);
    const { status, body } = respond(call);
    return new Response(body === undefined ? null : JSON.stringify(body), {
      status,
      headers: body === undefined ? {} : { 'content-type': 'application/json' },
    });
  };
  return { calls, api: createLayoutsApi(fetch as unknown as typeof globalThis.fetch) };
}

const layoutBody = { schemaVersion: 1, name: 'Mine', preset: false, generated: [], arrangements: { wide: [], tablet: [], phone: [] } };

describe('layouts API client', () => {
  test('list reads the revision and the layouts', async () => {
    const { api, calls } = fakeFetch(() => ({
      status: 200,
      body: { revision: 4, layouts: [{ name: 'Pitwall Classic', preset: true }] },
    }));
    expect(await api.list()).toEqual({ revision: 4, layouts: [{ name: 'Pitwall Classic', preset: true }] });
    expect(calls[0]).toMatchObject({ url: '/api/layouts', method: 'GET' });
  });

  test('names are percent-encoded in the path', async () => {
    const { api, calls } = fakeFetch(() => ({ status: 200, body: layoutBody }));
    await api.get('Pitwall Classic / 2');
    expect(calls[0]!.url).toBe('/api/layouts/Pitwall%20Classic%20%2F%202');
  });

  test('a missing layout is null, not an error', async () => {
    const { api } = fakeFetch(() => ({ status: 404, body: { error: 'no such layout' } }));
    expect(await api.get('Nope')).toBeNull();
  });

  test('save PUTs the whole body as JSON', async () => {
    const { api, calls } = fakeFetch(() => ({ status: 200, body: layoutBody }));
    await api.save('Mine', layoutBody);
    expect(calls[0]).toMatchObject({ url: '/api/layouts/Mine', method: 'PUT', contentType: 'application/json' });
    expect(calls[0]!.body).toEqual(layoutBody);
  });

  test('every non-GET, DELETE included, carries Content-Type: application/json', async () => {
    const { api, calls } = fakeFetch((c) => (c.method === 'DELETE' ? { status: 204 } : { status: 200, body: layoutBody }));
    await api.duplicate('Race');
    await api.duplicate('Race', 'Mine 2');
    await api.rename('Mine', 'Other');
    await api.remove('Mine');
    expect(calls.map((c) => [c.method, c.url, c.contentType, c.body])).toEqual([
      ['POST', '/api/layouts/Race/duplicate', 'application/json', {}],
      ['POST', '/api/layouts/Race/duplicate', 'application/json', { name: 'Mine 2' }],
      ['POST', '/api/layouts/Mine/rename', 'application/json', { name: 'Other' }],
      ['DELETE', '/api/layouts/Mine', 'application/json', undefined],
    ]);
  });

  test('server errors carry the status and message', async () => {
    const { api } = fakeFetch(() => ({ status: 409, body: { error: 'name already in use' } }));
    const error = await api.rename('A', 'B').catch((e: unknown) => e);
    expect(error).toBeInstanceOf(LayoutsApiError);
    expect(error).toMatchObject({ status: 409, message: 'name already in use' });
  });

  test('an unreachable collector is an error with status 0', async () => {
    const api = createLayoutsApi((async () => {
      throw new TypeError('Failed to fetch');
    }) as unknown as typeof fetch);
    expect(await api.list().catch((e: unknown) => e)).toMatchObject({ status: 0 });
  });
});
