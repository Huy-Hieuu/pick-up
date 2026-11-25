import { describe, expect, it, vi, afterEach } from 'vitest';
import { dateKey, safeWebUrl } from './format';
afterEach(() => vi.unstubAllGlobals());
describe('safe links and local dates', () => {
  it('rejects executable and credential-bearing links', () => {
    for (const value of ['javascript:alert(1)', 'data:text/html,test', 'https://user:secret@example.test', '//example.test', 'http://example.test']) expect(safeWebUrl(value)).toBeUndefined();
    expect(safeWebUrl('https://example.test/payment')).toBe('https://example.test/payment');
  });
  it('supports local HTTP links only in development', () => {
    vi.stubGlobal('__DEV__', true);
    expect(safeWebUrl('http://localhost:8080/payment')).toBe('http://localhost:8080/payment');
  });
  it('uses Vietnam calendar dates across UTC midnight and offsets', () => {
    vi.spyOn(Date, 'now').mockReturnValue(Date.parse('2026-10-05T18:00:00Z'));
    expect(dateKey()).toBe('2026-10-06');
    expect(dateKey(-1)).toBe('2026-10-05');
    vi.restoreAllMocks();
  });
});
