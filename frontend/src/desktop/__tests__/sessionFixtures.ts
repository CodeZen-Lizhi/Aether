import { AxiosError, type AxiosResponse, type InternalAxiosRequestConfig } from 'axios'
import type { LoginResponse, User } from '@/api/auth'

export const localUser: User = {
  id: 'local-user', username: 'desktop-local', role: 'admin', is_active: true, created_at: '2026-09-09T00:00:00Z',
}

export function sessionToken(name = 'initial'): string {
  return `header.${btoa(JSON.stringify({ user_id: localUser.id, role: localUser.role, jti: name }))}.signature`
}

export function loginResponse(name = 'initial'): LoginResponse {
  return {
    access_token: sessionToken(name), token_type: 'bearer', expires_in: 1800,
    user_id: localUser.id, username: localUser.username, email: null, role: localUser.role,
  }
}

export function setDesktopBridge(value: unknown): void {
  Object.defineProperty(window, '__AETHER_DESKTOP__', { value, configurable: true })
}

export function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => { resolve = resolvePromise; reject = rejectPromise })
  return { promise, resolve, reject }
}

export function apiResponse(config: InternalAxiosRequestConfig, data: unknown, status = 200): AxiosResponse {
  const response = { config, data, status, statusText: String(status), headers: {} }
  if (status >= 400) throw new AxiosError('API request failed', 'ERR_BAD_RESPONSE', config, undefined, response)
  return response
}
