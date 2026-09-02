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
    // 单用户版兜底：token 缺失或脏数据时回管理台登录。
    log.warn('Non-admin attempted to access admin page, redirecting to admin dashboard')
    return '/admin/dashboard'
  }

  return null
}
