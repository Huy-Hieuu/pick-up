import { useCallback, useEffect, useRef, useState } from 'react';
import { useSession } from '../stores/session';
import { ApiError } from '../api/client';
export function useResource<T>(path: string | null, validate?: (value: unknown) => value is T) {
  const {request, session, ready} = useSession();
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(!!path);
  const [version, setVersion] = useState(0);
  const identity = useRef<{path: string | null; userId: string | undefined}>({path, userId: session?.user.id});
  useEffect(() => {
    const controller = new AbortController();
    let active = true;
    if (identity.current.path !== path || identity.current.userId !== session?.user.id || !path) setData(null);
    identity.current = {path, userId: session?.user.id};
    setError(null); setLoading(!!path);
    if (path && ready) request<T>(path, {signal: controller.signal})
      .then((value) => {
        if (!active) return;
        if (validate && !validate(value)) throw new ApiError(502, 'INVALID_RESPONSE', 'Máy chủ trả về dữ liệu không hợp lệ. Vui lòng thử lại.');
        setData(value);
      })
      .catch((reason: unknown) => { if (active) setError(reason instanceof Error ? reason : new Error('Không thể tải dữ liệu.')); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; controller.abort(); };
  }, [path, version, session?.user.id, request, ready, validate]);
  const refresh = useCallback(() => setVersion((value) => value + 1), []);
  const sameResource = identity.current.path === path && identity.current.userId === session?.user.id;
  return { data: sameResource ? data : null, error: sameResource ? error : null, loading: sameResource ? loading : !!path, refresh };
}
