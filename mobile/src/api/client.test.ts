import { afterEach, describe, expect, it, vi } from 'vitest';
import { createApiClient, isSession } from './client';
import type { Session } from '../types/api';

const session = (token = 'old', id = '11111111-1111-4111-8111-111111111111'): Session => ({access_token: token, refresh_token: 'refresh-' + token, user: {id, email: null, phone: '', display_name: null, avatar_url: null, created_at: '2026-10-05T10:00:00Z'}});
const json = (value: unknown, status = 200) => new Response(JSON.stringify(value), {status});
function setup(fetcher: typeof fetch, timeoutMs = 1000) {
  let current: Session | null = session();
  const onSession = vi.fn(async (value: Session | null) => { current = value; });
  return {client: createApiClient({baseUrl: 'https://api.example.test', fetcher, timeoutMs, getSession: () => current, onSession}), onSession, setSession: (value: Session | null) => { current = value; }};
}
afterEach(() => vi.useRealTimers());
describe('API transport', () => {
  it('deduplicates concurrent refresh and retries with the rotated token', async () => {
    const fetcher = vi.fn(async (url: string | URL | Request, init?: RequestInit) => {
      if (String(url).endsWith('/auth/refresh')) return json(session('new'));
      return (init?.headers as Record<string, string>).Authorization === 'Bearer new' ? json({ok: true}) : json({}, 401);
    });
    const {client, onSession} = setup(fetcher as typeof fetch);
    await expect(Promise.all([client.request('/games'), client.request('/courts')])).resolves.toEqual([{ok: true}, {ok: true}]);
    expect(fetcher.mock.calls.filter(([url]) => String(url).endsWith('/auth/refresh'))).toHaveLength(1);
    expect(onSession).toHaveBeenCalledTimes(1);
  });
  it.each([null, session('other', 'other-user')])('does not restore a session after logout or account change', async (replacement) => {
    let finish!: (response: Response) => void;
    let entered!: () => void;
    const refreshing = new Promise<void>((resolve) => { entered = resolve; });
    const fetcher = vi.fn(async (url: string | URL | Request) => {
      if (String(url).endsWith('/auth/refresh')) { entered(); return new Promise<Response>((resolve) => { finish = resolve; }); }
      return json({}, 401);
    });
    const {client, onSession, setSession} = setup(fetcher as typeof fetch);
    const pending = client.request('/games');
    const assertion = expect(pending).rejects.toMatchObject({code: 'UNAUTHORIZED'});
    await refreshing;
    setSession(replacement);
    finish(json(session('new')));
    await assertion;
    expect(onSession).not.toHaveBeenCalled();
  });
  it('keeps the timeout active while reading the response body', async () => {
    vi.useFakeTimers();
    const fetcher = vi.fn(async (_url: unknown, init?: RequestInit) => ({status: 200, ok: true, text: () => new Promise<string>((_resolve, reject) => init?.signal?.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError'))))} as Response));
    const {client} = setup(fetcher as typeof fetch, 20);
    const assertion = expect(client.request('/games')).rejects.toMatchObject({name: 'AbortError'});
    await vi.advanceTimersByTimeAsync(21);
    await assertion;
  });
  it('allows one caller to cancel while the shared refresh continues', async () => {
    let finish!: (response: Response) => void;
    let entered!: () => void;
    const refreshing = new Promise<void>((resolve) => { entered = resolve; });
    const fetcher = vi.fn(async (url: string | URL | Request, init?: RequestInit) => {
      if (String(url).endsWith('/auth/refresh')) { entered(); return new Promise<Response>((resolve) => { finish = resolve; }); }
      return (init?.headers as Record<string, string>).Authorization === 'Bearer new' ? json({ok: true}) : json({}, 401);
    });
    const {client, onSession} = setup(fetcher as typeof fetch);
    const controller = new AbortController();
    const first = client.request('/games', {signal: controller.signal});
    const assertion = expect(first).rejects.toMatchObject({name: 'AbortError'});
    const second = client.request('/courts');
    await refreshing;
    controller.abort();
    await assertion;
    finish(json(session('new')));
    await expect(second).resolves.toEqual({ok: true});
    expect(onSession).toHaveBeenCalledTimes(1);
  });
  it('passes caller cancellation through response body consumption', async () => {
    const controller = new AbortController();
    const fetcher = vi.fn(async (_url: unknown, init?: RequestInit) => ({status: 200, ok: true, text: async () => { controller.abort(); if (init?.signal?.aborted) throw new DOMException('aborted', 'AbortError'); return '{}'; }} as Response));
    await expect(setup(fetcher as typeof fetch).client.request('/games', {signal: controller.signal})).rejects.toMatchObject({name: 'AbortError'});
  });
  it('rejects untrusted request paths without sending a bearer token', async () => {
    const fetcher = vi.fn();
    const {client} = setup(fetcher as typeof fetch);
    for (const path of ['https://evil.test', '//evil.test', '/games\\evil', '/../admin']) await expect(client.request(path)).rejects.toMatchObject({code: 'INVALID_URL'});
    expect(fetcher).not.toHaveBeenCalled();
  });
  it('handles invalid JSON and hides internal server failures', async () => {
    const fetcher = vi.fn().mockResolvedValueOnce(new Response('oops')).mockResolvedValueOnce(json({error: {code: 12, message: 'secret stack trace'}}, 500));
    const {client} = setup(fetcher as typeof fetch);
    await expect(client.request('/games')).rejects.toMatchObject({code: 'INVALID_RESPONSE'});
    await expect(client.request('/games')).rejects.toMatchObject({code: 'HTTP_ERROR', message: 'Không thể hoàn tất yêu cầu.'});
  });
  it('refreshes an expired access token before revoking logout', async () => {
    const calls: string[] = [];
    const fetcher = vi.fn(async (url: string | URL | Request, init?: RequestInit) => {
      const path = String(url);
      const token = (init?.headers as Record<string, string>).Authorization;
      calls.push(path + ':' + token);
      if (path.endsWith('/auth/logout') && token === 'Bearer old') return json({}, 401);
      if (path.endsWith('/auth/refresh')) return json(session('new'));
      if (path.endsWith('/auth/logout') && token === 'Bearer new') return new Response(null, {status: 204});
      return json({}, 500);
    });
    const {client, onSession} = setup(fetcher as typeof fetch);
    await expect(client.revoke()).resolves.toBeUndefined();
    expect(calls).toEqual([
      'https://api.example.test/auth/logout:Bearer old',
      'https://api.example.test/auth/refresh:undefined',
      'https://api.example.test/auth/logout:Bearer new',
    ]);
    expect(onSession).not.toHaveBeenCalled();
  });
  it('rejects sessions without a usable identity', () => {
    expect(isSession(session())).toBe(true);
    expect(isSession(session('token', ''))).toBe(false);
    expect(isSession({access_token: 'a', refresh_token: 'r', user: null})).toBe(false);
  });
});
