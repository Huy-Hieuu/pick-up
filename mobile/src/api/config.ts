export function resolveApiBaseUrl(
  value = process.env.EXPO_PUBLIC_API_URL,
  development = typeof __DEV__ !== 'undefined' && __DEV__,
): string {
  const configured = value?.trim();
  if (!configured) {
    if (development) return 'http://localhost:8080';
    throw new Error('Chưa cấu hình địa chỉ máy chủ. Vui lòng thiết lập EXPO_PUBLIC_API_URL trước khi phát hành.');
  }
  let url: URL;
  try {
    url = new URL(configured);
  } catch {
    throw new Error('EXPO_PUBLIC_API_URL phải là địa chỉ HTTP(S) hợp lệ.');
  }
  if (
    !(url.protocol === 'https:' || (development && url.protocol === 'http:')) ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    url.pathname !== '/'
  ) {
    throw new Error('EXPO_PUBLIC_API_URL phải là nguồn gốc HTTP(S) hợp lệ, không chứa đường dẫn, tài khoản hoặc tham số.');
  }
  return url.origin;
}
