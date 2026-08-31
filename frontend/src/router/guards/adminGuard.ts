import type { useAuthStore } from '@/stores/auth'
import { log } from '@/utils/logger'

/**
 * 检查管理员权限。
 * @returns 重定向路径，或 null 表示通过
 */
export function checkAdminAccess(
  authStore: ReturnType<typeof useAuthStore>
): string | null {
  if (!authStore.canAccessAdmin) {
    log.warn('Non-admin user attempted to access admin page, redirecting to user dashboard')
    return '/dashboard'
  }

  return null
}
