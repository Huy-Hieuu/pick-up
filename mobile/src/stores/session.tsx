import React, { createContext, useContext, useEffect, useMemo, useRef, useState, useCallback } from 'react';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { createApiClient, isSession, type RequestOptions } from '../api/client';
import { resolveApiBaseUrl } from '../api/config';
import type { Session, User } from '../types/api';
import { readSession, writeSession } from './secure';
import { endSession } from './logout';

interface SessionContextValue {
  session: Session | null; ready: boolean; saved: string[];
  configurationError: Error | null; storageError: string | null;
  request: <T>(path: string, options?: RequestOptions) => Promise<T>;
  authenticate: (session: Session) => Promise<void>;
  updateUser: (user: User) => Promise<void>;
  logout: () => Promise<void>;
  toggleSaved: (id: string) => void;
  clearStorageError: () => void;
}
const Context = createContext<SessionContextValue | null>(null);
export function SessionProvider({ children }: { children: React.ReactNode }) {
  const [session, setSession] = useState<Session | null>(null);
  const [ready, setReady] = useState(false);
  const [saved, setSaved] = useState<string[]>([]);
  const [storageError, setStorageError] = useState<string | null>(null);
  const [configuration] = useState(() => {
    try { return { baseUrl: resolveApiBaseUrl(), error: null }; }
    catch (error) { return { baseUrl: '', error: error instanceof Error ? error : new Error('Chưa cấu hình địa chỉ máy chủ.') }; }
  });
  const ref = useRef<Session | null>(null);
  const savedRef = useRef<string[]>([]);
  const revision = useRef(0);
  const writes = useRef<Promise<void>>(Promise.resolve());
  const bookmarkWrites = useRef<Promise<void>>(Promise.resolve());
  const authenticate = useCallback(async (value: Session | null) => {
    if (value !== null && !isSession(value)) throw new Error('Phiên đăng nhập không hợp lệ.');
    revision.current += 1;
    ref.current = value;
    setSession(value);
    const operation = writes.current.catch(() => undefined).then(() => writeSession(value ? JSON.stringify(value) : null));
    writes.current = operation;
    try { await operation; }
    catch (error) {
      setStorageError('Không thể lưu trạng thái đăng nhập trên thiết bị.');
      if (value === null) throw error;
    }
  }, []);
  const updateUser = useCallback(async (user: User) => {
    const current = ref.current;
    if (!current || current.user.id !== user.id) throw new Error('Phiên đăng nhập đã thay đổi. Vui lòng thử lại.');
    await authenticate({...current, user});
  }, [authenticate]);
  const client = useMemo(() => createApiClient({
    baseUrl: configuration.baseUrl,
    getSession: () => ref.current,
    onSession: authenticate,
  }), [authenticate, configuration.baseUrl]);
  useEffect(() => {
    let active = true;
    const initialRevision = revision.current;
    (async () => {
      const [storedResult, bookmarksResult] = await Promise.allSettled([
        readSession(),
        AsyncStorage.getItem('pickup.saved.v1'),
      ]);
      if (!active) return;
      if (storedResult.status === 'rejected') {
        setStorageError('Không thể khôi phục phiên đăng nhập trên thiết bị.');
      } else if (storedResult.value && revision.current === initialRevision) {
        let value: unknown;
        try { value = JSON.parse(storedResult.value); } catch { value = null; }
        if (isSession(value)) { ref.current = value; setSession(value); }
        else {
          try { await writeSession(null); }
          catch { setStorageError('Không thể xóa phiên đăng nhập không hợp lệ khỏi thiết bị.'); }
        }
      }
      if (bookmarksResult.status === 'rejected') {
        setStorageError('Không thể tải danh sách sân đã lưu.');
      } else if (bookmarksResult.value) {
        try {
          const value: unknown = JSON.parse(bookmarksResult.value);
          if (Array.isArray(value)) {
            const next = value.filter((id): id is string => typeof id === 'string');
            savedRef.current = next;
            setSaved(next);
          }
        } catch {
          setStorageError('Dữ liệu sân đã lưu không hợp lệ. Vui lòng thử lại.');
        }
      }
      if (active) setReady(true);
    })();
    return () => { active = false; };
  }, []);
  const value: SessionContextValue = {
    session, ready, saved, configurationError: configuration.error, storageError,
    request: client.request, authenticate, updateUser,
    clearStorageError: () => setStorageError(null),
    logout: async () => {
      await endSession(() => client.revoke(), () => authenticate(null));
    },
    toggleSaved: (id) => {
      const previous = savedRef.current;
      const next = previous.includes(id) ? previous.filter((item) => item !== id) : [...previous, id];
      savedRef.current = next;
      setSaved(next);
      const operation = bookmarkWrites.current.catch(() => undefined).then(() => AsyncStorage.setItem('pickup.saved.v1', JSON.stringify(next)));
      bookmarkWrites.current = operation;
      void operation.catch(() => setStorageError('Không thể lưu thay đổi vào danh sách sân.'));
    },
  };
  return <Context.Provider value={value}>{children}</Context.Provider>;
}
export function useSession() {
  const value = useContext(Context);
  if (!value) throw new Error('SessionProvider is missing');
  return value;
}
