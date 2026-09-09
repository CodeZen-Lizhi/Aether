import { readonly, shallowRef } from 'vue'
import type { LoginResponse } from '@/api/auth'
import { getClientDeviceId } from '@/utils/deviceId'

interface DesktopSessionBridge {
  authenticate: (clientDeviceId: string) => Promise<unknown>
}

declare global {
  interface Window {
    readonly __AETHER_DESKTOP__?: unknown
  }
}

type DesktopSessionPhase = 'idle' | 'connecting' | 'authenticated' | 'failed'
const CONNECTION_ERROR = '无法连接本机网关。请确认网关正在运行，然后重试。'
const state = shallowRef<{ phase: DesktopSessionPhase; error: string }>({ phase: 'idle', error: '' })
export const desktopSessionState = readonly(state)

let inFlight: Promise<LoginResponse> | null = null
let revision = 0

function getBridge(): DesktopSessionBridge | null {
  const value = typeof window === 'undefined' ? undefined : window.__AETHER_DESKTOP__
  if (!value || typeof value !== 'object' || Array.isArray(value)
    || !('authenticate' in value) || typeof value.authenticate !== 'function') return null
  return value as DesktopSessionBridge
}

/** This selects the desktop UX only. The gateway still verifies every session and API request. */
export function hasDesktopSession(): boolean {
  return getBridge() !== null
}

function isLoginResponse(value: unknown): value is LoginResponse {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false
  const response = value as Record<string, unknown>
  return typeof response.access_token === 'string' && response.access_token.trim().length > 0
    && ['token_type', 'user_id', 'username', 'role'].every(key =>
      response[key] === undefined || typeof response[key] === 'string')
    && (response.email === undefined || response.email === null || typeof response.email === 'string')
    && (response.expires_in === undefined
      || (typeof response.expires_in === 'number' && Number.isFinite(response.expires_in) && response.expires_in > 0))
}

export function failDesktopSession(message = CONNECTION_ERROR): void {
  revision += 1
  // Preserve a more specific failure until the user chooses to reconnect.
  if (state.value.phase !== 'failed') state.value = { phase: 'failed', error: message }
}

/** No Tauri IPC or native capability is exposed to dashboard code. */
export function authenticateDesktopSession(options: { retry?: boolean } = {}): Promise<LoginResponse> {
  if (state.value.phase === 'failed' && !options.retry) {
    return Promise.reject(new Error(state.value.error))
  }
  if (inFlight) {
    if (options.retry && state.value.phase === 'failed') {
      // An explicit retry waits out an invalidated exchange, then performs a new one.
      return inFlight.catch(() => undefined).then(() => authenticateDesktopSession(options))
    }
    return inFlight
  }
  const bridge = getBridge()
  if (!bridge) return Promise.reject(new Error('请在 Aether macOS 客户端中打开管理界面。'))

  const requestRevision = ++revision
  state.value = { phase: 'connecting', error: '' }
  let failureMessage = CONNECTION_ERROR
  const request = Promise.resolve()
    .then(() => bridge.authenticate(getClientDeviceId()))
    .then(response => {
      if (requestRevision !== revision) throw new Error(CONNECTION_ERROR)
      if (!isLoginResponse(response)) {
        failureMessage = '本机网关返回了无法识别的会话，请重试连接。'
        throw new Error(failureMessage)
      }
      state.value = { phase: 'authenticated', error: '' }
      return response
    })
    .catch(() => {
      if (requestRevision === revision) failDesktopSession(failureMessage)
      // Native/network errors are deliberately not copied into UI or logs.
      throw new Error(state.value.error || CONNECTION_ERROR)
    })
    .finally(() => {
      if (inFlight === request) inFlight = null
    })
  inFlight = request
  return request
}
