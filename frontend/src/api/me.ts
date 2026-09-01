import apiClient from './client'
import type { UserSession } from '@/types/session'
import type { BillingSummary } from './auth'
import type { FeatureSettingsMap } from '@/utils/featureSettings'

export interface Profile {
  id: string // UUID
  email?: string | null
  username: string
  role: string
  is_active: boolean
  billing: BillingSummary
  created_at: string
  updated_at?: string
  last_login_at?: string
  auth_source: 'local' | 'ldap' | 'oauth'
  has_password: boolean
  preferences?: UserPreferences
  feature_settings?: FeatureSettingsMap | null
}

export interface UserPreferences {
  avatar_url?: string
  bio?: string
  default_provider_id?: string // UUID
  default_provider?: Record<string, unknown> | string | null
  theme: string
  language: string
  timezone?: string
  notifications?: {
    email?: boolean
    usage_alerts?: boolean
    announcements?: boolean
  }
}

export interface ChangePasswordRequest {
  old_password?: string  // 可选：首次设置密码时不需要
  new_password: string
}

// 单用户版：meApi 只保留管理员自助的账号能力（资料/密码/会话/偏好）。
export const meApi = {
  // 获取个人信息
  async getProfile(): Promise<Profile> {
    const response = await apiClient.get<Profile>('/api/users/me')
    return response.data
  },

  // 更新个人信息
  async updateProfile(data: {
    email?: string
    username?: string
    feature_settings?: FeatureSettingsMap | null
  }): Promise<{ message: string }> {
    const response = await apiClient.put('/api/users/me', data)
    return response.data
  },

  // 修改密码
  async changePassword(data: ChangePasswordRequest): Promise<{ message: string }> {
    const response = await apiClient.patch('/api/users/me/password', data)
    return response.data
  },

  async listSessions(): Promise<UserSession[]> {
    const response = await apiClient.get<UserSession[]>('/api/users/me/sessions')
    return response.data
  },

  async updateSessionLabel(sessionId: string, deviceLabel: string): Promise<UserSession> {
    const response = await apiClient.patch<UserSession>(`/api/users/me/sessions/${sessionId}`, {
      device_label: deviceLabel,
    })
    return response.data
  },

  async revokeSession(sessionId: string): Promise<{ message: string }> {
    const response = await apiClient.delete(`/api/users/me/sessions/${sessionId}`)
    return response.data
  },

  async revokeOtherSessions(): Promise<{ message: string; revoked_count: number }> {
    const response = await apiClient.delete('/api/users/me/sessions/others')
    return response.data
  },

  // 偏好设置
  async getPreferences(): Promise<UserPreferences> {
    const response = await apiClient.get('/api/users/me/preferences')
    return response.data
  },

  async updatePreferences(data: Partial<UserPreferences>): Promise<{ message: string }> {
    const response = await apiClient.put('/api/users/me/preferences', data)
    return response.data
  }
}
