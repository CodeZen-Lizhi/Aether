import axios from 'axios'
import type { AxiosInstance, AxiosRequestConfig, AxiosResponse, InternalAxiosRequestConfig } from 'axios'
import { NETWORK_CONFIG, AUTH_CONFIG } from '@/config/constants'
import { getClientDeviceId } from '@/utils/deviceId'
import { CrossTabRefreshCoordinator } from '@/utils/crossTabRefresh'
import { log } from '@/utils/logger'
import { cache } from '@/utils/cache'
import { authenticateDesktopSession, desktopSessionState, failDesktopSession, hasDesktopSession } from '@/desktop/session'
import { getApiBaseUrl } from '@/utils/url'

export const AUTH_STATE_CHANGE_EVENT = 'aether-auth-state-change'

/**
 * 判断请求是否为公共端点
 */
function isPublicEndpoint(url?: string, method?: string): boolean {
  if (!url) return false

  const isHealthCheck = url.includes('/health') &&
                       method?.toLowerCase() === 'get' &&
                       !url.includes('/api/admin')

  return url.includes('/public') ||
         url.includes('.json') ||
         isHealthCheck
}

/**
 * 判断是否为认证相关请求
 */
function isAuthRequest(url?: string): boolean {
  return url?.includes('/auth/login') || url?.includes('/auth/refresh') || url?.includes('/auth/logout') || false
}

/**
 * 判断 403 错误是否表示用户账号级别的问题（需要清除认证并跳转）
 */
function isAccountLevelForbidden(status: number, errorDetail: string): boolean {
  if (status !== 403) return false
  const accountErrors = [
    '用户不存在或已禁用',
    '用户已禁用',
  ]
  return accountErrors.some((msg) => errorDetail.includes(msg))
}

class ApiClient {
  private client: AxiosInstance
  private token: string | null = null
  private isRefreshing = false
  private refreshPromise: Promise<string> | null = null
  private readonly refreshCoordinator = new CrossTabRefreshCoordinator()

  private readonly onStorageSync = (event: StorageEvent): void => {
    if (event.key !== 'access_token') {
      return
    }
    this.syncTokenState(event.newValue)
  }

  constructor() {
    this.client = axios.create({
      baseURL: getApiBaseUrl(),
      timeout: NETWORK_CONFIG.API_TIMEOUT,
      withCredentials: true,
      headers: {
        'Content-Type': 'application/json',
      },
    })

    this.setupInterceptors()
    this.setupCrossTabAuthSync()
  }

  /**
   * 配置请求和响应拦截器
   */
  private setupInterceptors(): void {
    // 请求拦截器 - 仅处理认证
    this.client.interceptors.request.use(
      (config) => {
        if (config.url?.includes('/api/')) {
          config.headers['X-Client-Device-Id'] = getClientDeviceId()
        }

        const requiresAuth = !isPublicEndpoint(config.url, config.method) &&
                           config.url?.includes('/api/')

        if (requiresAuth) {
          const token = this.getToken()
          if (token) {
            config.headers.Authorization = `Bearer ${token}`
          }
        }
        return config
      },
      (error) => Promise.reject(error)
    )

    // 响应拦截器
    this.client.interceptors.response.use(
      (response) => response,
      async (error) => this.handleResponseError(error)
    )
  }

  private setupCrossTabAuthSync(): void {
    if (typeof window !== 'undefined') {
      window.addEventListener('storage', this.onStorageSync)
    }
  }

  private emitAuthStateChange(token: string | null): void {
    if (typeof window === 'undefined') {
      return
    }
    window.dispatchEvent(
      new CustomEvent<{ token: string | null }>(AUTH_STATE_CHANGE_EVENT, {
        detail: { token },
      })
    )
  }

  /**
   * 处理响应错误
   */
  private async handleResponseError(error: unknown): Promise<AxiosResponse> {
    // 请求被取消
    if (axios.isCancel(error)) {
      return Promise.reject(error)
    }

    if (!axios.isAxiosError(error)) {
      return Promise.reject(error)
    }

    const originalRequest = error.config

    // Refresh failures are handled by the shared recovery promise below.
    if (isAuthRequest(originalRequest?.url)) {
      return Promise.reject(error)
    }

    // 网络错误或服务器不可达
    if (!error.response) {
      log.warn('Network error or server unreachable', error.message)
      if (hasDesktopSession() && originalRequest?.url?.includes('/api/')
        && !isPublicEndpoint(originalRequest.url, originalRequest.method)) {
        this.failDesktopAuth(originalRequest)
      }
      return Promise.reject(error)
    }

    const status = error.response?.status ?? 0

    // 处理 403 用户账号级别错误（被禁用/删除）
    if (status === 403) {
      const rawDetail = (error.response?.data as Record<string, unknown>)?.detail
      const errorDetail = typeof rawDetail === 'string' ? rawDetail : ''
      if (isAccountLevelForbidden(status, errorDetail)) {
        log.info('User account issue detected, clearing auth', { errorDetail })
        if (hasDesktopSession()) {
          this.failDesktopAuth(originalRequest)
        } else {
          this.clearAuth()
          window.location.href = '/'
        }
        return Promise.reject(error)
      }
    }

    // 处理401错误
    if (status === 401) {
      return this.handle401Error(error, originalRequest)
    }

    return Promise.reject(error)
  }

  /**
   * 处理401认证错误
   */
  private async handle401Error(error: import('axios').AxiosError, originalRequest: InternalAxiosRequestConfig & { _retry?: boolean; _retryCount?: number } | undefined): Promise<AxiosResponse> {
    // 如果不需要认证,直接返回错误
    if (isPublicEndpoint(originalRequest?.url, originalRequest?.method)) {
      return Promise.reject(error)
    }

    // 如果已经重试过,不再重试
    if (!originalRequest || originalRequest._retry
      || (hasDesktopSession() && desktopSessionState.value.phase === 'failed')) {
      if (hasDesktopSession()) this.failDesktopAuth(originalRequest)
      return Promise.reject(error)
    }

    log.debug('Got 401 error, attempting token refresh')

    // 标记为已重试
    originalRequest._retry = true
    originalRequest._retryCount = (originalRequest._retryCount || 0) + 1

    // 超过最大重试次数
    if (originalRequest._retryCount > AUTH_CONFIG.MAX_RETRY_COUNT) {
      log.error('Max retry attempts reached')
      if (hasDesktopSession()) this.failDesktopAuth(originalRequest)
      return Promise.reject(error)
    }

    // Another desktop request may already have replaced the rejected token.
    const currentToken = this.getToken()
    if (hasDesktopSession() && currentToken && originalRequest.headers.Authorization !== `Bearer ${currentToken}`) {
      originalRequest.headers.Authorization = `Bearer ${currentToken}`
      return this.client.request(originalRequest)
    }

    // 如果正在刷新,等待刷新完成
    if (this.isRefreshing) {
      try {
        const accessToken = await this.refreshPromise
        if (hasDesktopSession() && desktopSessionState.value.phase === 'failed') throw new Error('Desktop session unavailable')
        originalRequest.headers.Authorization = `Bearer ${accessToken}`
        return this.client.request(originalRequest)
      } catch {
        return Promise.reject(error)
      }
    }

    // 开始刷新token
    return this.refreshTokenAndRetry(originalRequest, error)
  }

  /**
   * 刷新token并重试原始请求
   */
  private async refreshTokenAndRetry(
    originalRequest: InternalAxiosRequestConfig,
    originalError: import('axios').AxiosError
  ): Promise<AxiosResponse> {
    this.isRefreshing = true
    const previousToken = this.getToken()
    this.refreshPromise = this.coordinatedRefresh()

    try {
      let accessToken = await this.refreshPromise
      if (hasDesktopSession() && desktopSessionState.value.phase === 'failed') throw new Error('Desktop session unavailable')
      const latestToken = this.getToken()
      if (hasDesktopSession() && latestToken && latestToken !== previousToken) accessToken = latestToken
      this.setToken(accessToken)
      this.isRefreshing = false
      this.refreshPromise = null

      // 重试原始请求
      originalRequest.headers.Authorization = `Bearer ${accessToken}`
      return this.client.request(originalRequest)
    } catch (refreshError: unknown) {
      log.error('Token refresh failed', refreshError instanceof Error ? refreshError.message : String(refreshError))
      this.isRefreshing = false
      this.refreshPromise = null
      if (hasDesktopSession()) this.failDesktopAuth(originalRequest)
      else this.clearAuth()
      return Promise.reject(originalError)
    }
  }

  private async coordinatedRefresh(): Promise<string> {
    if (hasDesktopSession()) {
      if (desktopSessionState.value.phase === 'failed') throw new Error('Desktop session unavailable')
      try {
        const response = await this.refreshToken()
        if (typeof response.data?.access_token !== 'string' || !response.data.access_token.trim()) {
          throw new Error('Refresh response missing access token')
        }
        return response.data.access_token
      } catch {
        return (await authenticateDesktopSession()).access_token
      }
    }
    return this.refreshCoordinator.run(async () => {
      const response = await this.refreshToken()
      const accessToken = response.data.access_token
      if (!accessToken) {
        throw new Error('Refresh response missing access token')
      }
      return accessToken
    })
  }

  private failDesktopAuth(request?: InternalAxiosRequestConfig): void {
    const currentToken = this.getToken()
    const currentAuthorization = currentToken ? `Bearer ${currentToken}` : undefined
    // Old failures cannot invalidate a reconnect, including while its token is still pending.
    if (request && (request.headers.Authorization ?? undefined) !== currentAuthorization) return
    failDesktopSession()
    this.clearAuth()
  }

  private syncTokenState(token: string | null): void {
    if (this.token !== token) {
      cache.clear()
    }
    this.token = token
  }

  setToken(token: string): void {
    const changed = this.token !== token
    if (this.token === token) {
      cache.clear()
    }
    this.syncTokenState(token)
    localStorage.setItem('access_token', token)
    if (changed && hasDesktopSession()) this.emitAuthStateChange(token)
  }

  getToken(): string | null {
    if (!this.token) {
      this.syncTokenState(localStorage.getItem('access_token'))
    }
    return this.token
  }

  clearAuth(): void {
    const hadAuth = this.token !== null || localStorage.getItem('access_token') !== null
    if (hadAuth && this.token === null) {
      cache.clear()
    }
    this.syncTokenState(null)
    localStorage.removeItem('access_token')
    // 同标签页内清理认证状态时不会触发 storage 事件，这里主动广播一次。
    if (hadAuth) {
      this.emitAuthStateChange(null)
    }
  }

  async refreshToken(): Promise<AxiosResponse> {
    return this.client.post('/api/auth/refresh')
  }

  // 以下方法直接委托给 axios client
  async request<T = any>(config: AxiosRequestConfig): Promise<AxiosResponse<T>> {
    return this.client.request<T>(config)
  }

  async get<T = any>(url: string, config?: AxiosRequestConfig): Promise<AxiosResponse<T>> {
    return this.client.get<T>(url, config)
  }

  async post<T = any>(url: string, data?: unknown, config?: AxiosRequestConfig): Promise<AxiosResponse<T>> {
    return this.client.post<T>(url, data, config)
  }

  async put<T = any>(url: string, data?: unknown, config?: AxiosRequestConfig): Promise<AxiosResponse<T>> {
    return this.client.put<T>(url, data, config)
  }

  async patch<T = any>(url: string, data?: unknown, config?: AxiosRequestConfig): Promise<AxiosResponse<T>> {
    return this.client.patch<T>(url, data, config)
  }

  async delete<T = any>(url: string, config?: AxiosRequestConfig): Promise<AxiosResponse<T>> {
    return this.client.delete<T>(url, config)
  }
}

export default new ApiClient()
