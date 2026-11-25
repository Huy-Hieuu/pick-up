import { describe, expect, it } from 'vitest';
import { authReturnRoute } from './navigation';
import { userFacingError } from './errors';
describe('authentication return routes', () => {
  it('preserves known app routes and booking query parameters', () => {
    const route = '/game/create?slot=11111111-1111-4111-8111-111111111111&court=22222222-2222-4222-8222-222222222222';
    expect(authReturnRoute(route)).toBe(route);
    expect(authReturnRoute('/court/11111111-1111-4111-8111-111111111111')).toBe('/court/11111111-1111-4111-8111-111111111111');
    expect(authReturnRoute(['/profile/', '/login'])).toBe('/profile');
    expect(authReturnRoute('/explore#ignored')).toBe('/explore');
  });
  it('blocks external, unknown and normalized authentication routes', () => {
    for (const route of [undefined, '', '//evil.test', '/\\evil.test', 'https://evil.test', '/login/', '/login#x', '/register?next=/login', '/%6cogin', '/games/../login', '/%2f%2fevil.test', '/%256cogin', '/unknown', '/court/not-a-uuid', '/profile\r\n']) expect(authReturnRoute(route)).toBe('/');
  });
});
describe('Vietnamese errors', () => {
  it('maps browser and native network/abort failures without exposing technical messages', () => {
    expect(userFacingError(new TypeError('Failed to fetch'))).toContain('Không kết nối được');
    expect(userFacingError(new Error('Network request failed'))).toContain('Không kết nối được');
    expect(userFacingError(new DOMException('aborted', 'AbortError'))).toContain('quá thời gian');
    expect(userFacingError(new Error('Email chưa được xác minh.'))).toBe('Email chưa được xác minh.');
    expect(userFacingError(null)).toContain('Vui lòng thử lại');
  });
});
