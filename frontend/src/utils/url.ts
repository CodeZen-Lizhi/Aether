import { hasDesktopSession } from '@/desktop/session'

export function getApiBaseUrl(): string {
  // A desktop build may inherit Web deployment variables. Its session is local only.
  return hasDesktopSession() ? '' : import.meta.env.VITE_API_URL || ''
}

/**
 * 构建完整的 API URL
 *
 * 用于需要完整 URL 的场景（如 OAuth 重定向），
 * 处理 VITE_API_URL 环境变量和路径拼接。
 */
export function getApiUrl(path: string): string {
  const base = getApiBaseUrl()
  // 移除 base 尾部的 `/`，避免拼接成 `//api/...`
  return base ? `${base.replace(/\/$/, '')}${path}` : path
}
