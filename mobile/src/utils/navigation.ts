const uuid = '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}';
const detailRoute = new RegExp('^/(court|game)/' + uuid + '$', 'i');
export function authReturnRoute(value: string | string[] | undefined): string {
  const raw = Array.isArray(value) ? value[0] : value;
  if (!raw || !raw.startsWith('/') || raw.startsWith('//') || /[\\\r\n]/.test(raw)) return '/';
  try {
    const parsed = new URL(raw, 'https://pickup.invalid');
    if (parsed.origin !== 'https://pickup.invalid') return '/';
    const decoded = decodeURIComponent(parsed.pathname);
    if (/[\\%\r\n]/.test(decoded)) return '/';
    const normalized = new URL(decoded, 'https://pickup.invalid');
    if (normalized.origin !== parsed.origin) return '/';
    const path = normalized.pathname.replace(/\/+$/, '') || '/';
    if (!['/', '/explore', '/saved', '/games', '/profile', '/game/create'].includes(path) && !detailRoute.test(path)) return '/';
    return path + parsed.search;
  } catch { return '/'; }
}
