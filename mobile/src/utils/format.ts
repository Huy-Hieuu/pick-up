import type { GameStatus, Sport } from '../types/api';
export const money = (value: number) => new Intl.NumberFormat('vi-VN', { style: 'currency', currency: 'VND', maximumFractionDigits: 0 }).format(value);
export const sportName = (sport: Sport) => sport === 'pickleball' ? 'Pickleball' : 'Bóng đá mini';
export const gameStatus: Record<GameStatus, string> = { open: 'Đang tìm đồng đội', full: 'Đã đủ người', in_progress: 'Đang diễn ra', completed: 'Đã hoàn tất', cancelled: 'Đã hủy' };
export const time = (value: string) => new Intl.DateTimeFormat('vi-VN', {hour: '2-digit', minute: '2-digit', timeZone: 'Asia/Ho_Chi_Minh'}).format(new Date(value));
export const day = (value: string) => new Intl.DateTimeFormat('vi-VN', {weekday: 'short', day: '2-digit', month: '2-digit', timeZone: 'Asia/Ho_Chi_Minh'}).format(new Date(value));
export function dateKey(offset = 0) {
  const date = new Date(Date.now() + offset * 86400000);
  const parts = new Intl.DateTimeFormat('en-CA', {timeZone: 'Asia/Ho_Chi_Minh', year: 'numeric', month: '2-digit', day: '2-digit'}).formatToParts(date);
  const get = (name: string) => parts.find((part) => part.type === name)?.value;
  return get('year') + '-' + get('month') + '-' + get('day');
}
export function safeWebUrl(value: string | null): string | undefined {
  if (!value) return undefined;
  try {
    const url = new URL(value);
    if (url.username || url.password) return undefined;
    const development = typeof __DEV__ !== 'undefined' && __DEV__;
    return url.protocol === 'https:' || (development && url.protocol === 'http:') ? url.href : undefined;
  }
  catch { return undefined; }
}
