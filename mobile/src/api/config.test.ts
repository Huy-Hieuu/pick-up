import { describe, expect, it } from 'vitest';
import { resolveApiBaseUrl } from './config';

describe('API base URL configuration', () => {
  it('uses the local API only in development', () => {
    expect(resolveApiBaseUrl('', true)).toBe('http://localhost:8080');
    expect(() => resolveApiBaseUrl('', false)).toThrow(/EXPO_PUBLIC_API_URL/);
  });

  it('accepts an HTTPS origin and normalizes a trailing slash', () => {
    expect(resolveApiBaseUrl('https://api.example.test/', false)).toBe('https://api.example.test');
  });

  it.each([
    'http://api.example.test',
    'https://user:secret@api.example.test',
    'https://api.example.test/path',
    'https://api.example.test?x=1',
    'https://api.example.test#fragment',
    'not-a-url',
  ])('rejects unsafe production configuration %s', (value) => {
    expect(() => resolveApiBaseUrl(value, false)).toThrow();
  });
});
