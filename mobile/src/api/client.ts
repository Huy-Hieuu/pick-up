import type { Session } from '../types/api';
import { isUser } from '../types/validators';

export class ApiError extends Error {
  constructor(public status: number, public code: string, message: string) { super(message); this.name = 'ApiError'; }
}
export interface RequestOptions { method?: 'GET' | 'POST' | 'PATCH'; body?: unknown; signal?: AbortSignal }
interface TransportOptions {
  baseUrl: string;
  getSession: () => Session | null;
  onSession: (session: Session | null) => Promise<void>;
  fetcher?: typeof fetch;
  timeoutMs?: number;
}
function cancellable<T>(pending: Promise<T>, signal?: AbortSignal): Promise<T> {
  if (!signal) return pending;
  if (signal.aborted) return Promise.reject(new DOMException('Request aborted', 'AbortError'));
  return new Promise<T>((resolve, reject) => {
    const abort = () => reject(new DOMException('Request aborted', 'AbortError'));
    signal.addEventListener('abort', abort, {once: true});
    pending.then(resolve, reject).finally(() => signal.removeEventListener('abort', abort));
  });
}
export function isSession(value: unknown): value is Session {
  if (!value || typeof value !== 'object') return false;
  const session = value as Partial<Session>;
  return typeof session.access_token === 'string' && session.access_token.length > 0 &&
    typeof session.refresh_token === 'string' && session.refresh_token.length > 0 &&
    isUser(session.user);
}
export function createApiClient(options: TransportOptions) {
  const baseUrl = options.baseUrl.replace(/\/+$/, '');
  const fetcher = options.fetcher ?? fetch;
  let refreshing: { source: Session; promise: Promise<Session> } | null = null;
  const unauthorized = () => new ApiError(401, 'UNAUTHORIZED', 'Phiên đăng nhập đã hết hạn.');
  function checkPath(path: string) {
    if (!/^\/(?!\/)/.test(path) || /[\\\r\n#]/.test(path) || path.split('?')[0].split('/').some((part) => part === '..' || part === '.')) {
      throw new ApiError(0, 'INVALID_URL', 'Đường dẫn yêu cầu không hợp lệ.');
    }
    try {
      const url = new URL(baseUrl);
      const development = typeof __DEV__ !== 'undefined' && __DEV__;
      if (!(url.protocol === 'https:' || (development && url.protocol === 'http:')) || url.username || url.password || url.search || url.hash) throw new Error();
    } catch { throw new ApiError(0, 'API_NOT_CONFIGURED', 'Chưa cấu hình địa chỉ máy chủ.'); }
  }
  async function send(path: string, opts: RequestOptions, token?: string): Promise<{response: Response; text: string}> {
    checkPath(path);
    const controller = new AbortController();
    const abort = () => controller.abort();
    if (opts.signal?.aborted) controller.abort();
    opts.signal?.addEventListener('abort', abort, { once: true });
    const timer = setTimeout(abort, options.timeoutMs ?? 15000);
    try {
      const response = await fetcher(baseUrl + path, {
        method: opts.method ?? 'GET', signal: controller.signal,
        headers: { Accept: 'application/json', ...(opts.body !== undefined ? { 'Content-Type': 'application/json' } : {}), ...(token ? { Authorization: 'Bearer ' + token } : {}) },
        ...(opts.body !== undefined ? { body: JSON.stringify(opts.body) } : {}),
      });
      const text = await response.text();
      if (controller.signal.aborted) throw new DOMException('Request aborted', 'AbortError');
      return { response, text };
    } finally {
      clearTimeout(timer);
      opts.signal?.removeEventListener('abort', abort);
    }
  }
  async function decode<T>({response, text}: {response: Response; text: string}): Promise<T> {
    if (response.status === 204) return undefined as T;
    let value: unknown;
    try { value = text ? JSON.parse(text) : null; }
    catch { throw new ApiError(response.status, 'INVALID_RESPONSE', 'Máy chủ trả về dữ liệu không hợp lệ. Vui lòng thử lại.'); }
    if (!response.ok) {
      const detail = (value as {error?: {code?: string; message?: string}} | null)?.error;
      throw new ApiError(response.status, typeof detail?.code === 'string' ? detail.code : 'HTTP_ERROR', response.status < 500 && typeof detail?.message === 'string' ? detail.message : 'Không thể hoàn tất yêu cầu.');
    }
    return value as T;
  }
  async function renew(current: Session): Promise<Session> {
    const result = await send('/auth/refresh', {method: 'POST', body: {refresh_token: current.refresh_token}});
    const session = await decode<unknown>(result);
    if (!isSession(session)) throw new ApiError(502, 'INVALID_RESPONSE', 'Không thể xác thực phiên đăng nhập.');
    return session;
  }
  async function refresh(current: Session): Promise<Session> {
    const result = await send('/auth/refresh', {method: 'POST', body: {refresh_token: current.refresh_token}});
    if (options.getSession() !== current) throw unauthorized();
    if (result.response.status === 401) await options.onSession(null);
    const session = await decode<unknown>(result);
    if (!isSession(session)) throw new ApiError(502, 'INVALID_RESPONSE', 'Không thể xác thực phiên đăng nhập.');
    await options.onSession(session);
    return session;
  }
  async function revoke(): Promise<void> {
    const current = options.getSession();
    if (!current) return;
    let result = await send('/auth/logout', {method: 'POST'}, current.access_token);
    if (result.response.status === 401) {
      const renewed = await renew(current);
      result = await send('/auth/logout', {method: 'POST'}, renewed.access_token);
    }
    await decode(result);
  }
  async function request<T>(path: string, opts: RequestOptions = {}): Promise<T> {
    const usedSession = options.getSession();
    let result = await send(path, opts, usedSession?.access_token);
    const authAction = ['/auth/login', '/auth/register', '/auth/refresh', '/auth/logout'].includes(path.split('?')[0]);
    if (result.response.status === 401 && usedSession && !authAction) {
      const latest = options.getSession();
      if (latest === usedSession) {
        if (!refreshing || refreshing.source !== usedSession) {
          const entry = {source: usedSession, promise: refresh(usedSession)};
          refreshing = entry;
          void entry.promise.finally(() => { if (refreshing === entry) refreshing = null; }).catch(() => undefined);
        }
        await cancellable(refreshing.promise, opts.signal);
      } else if (!latest || latest.user.id !== usedSession.user.id) {
        throw unauthorized();
      }
      if (opts.signal?.aborted) throw new DOMException('Request aborted', 'AbortError');
      const renewed = options.getSession();
      if (!renewed || renewed.user.id !== usedSession.user.id) throw unauthorized();
      result = await send(path, opts, renewed.access_token);
      if (result.response.status === 401 && options.getSession() === renewed) await options.onSession(null);
    }
    return decode<T>(result);
  }
  return { request, revoke };
}
