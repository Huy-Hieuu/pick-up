export function userFacingError(error: unknown): string {
  if (!(error instanceof Error)) return 'Không thể hoàn tất yêu cầu. Vui lòng thử lại.';
  if (error.name === 'AbortError') return 'Yêu cầu đã dừng hoặc quá thời gian chờ. Vui lòng thử lại.';
  if (error.name === 'TimeoutError') return 'Máy chủ phản hồi quá lâu. Vui lòng thử lại.';
  if (error instanceof TypeError || /network request failed|failed to fetch|load failed|networkerror|fetch failed/i.test(error.message)) return 'Không kết nối được máy chủ. Kiểm tra kết nối và thử lại.';
  return error.message || 'Không thể hoàn tất yêu cầu. Vui lòng thử lại.';
}
