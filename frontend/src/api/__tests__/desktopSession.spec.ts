import { AxiosError, type AxiosInstance, type InternalAxiosRequestConfig } from 'axios'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { apiResponse, deferred, loginResponse, sessionToken, setDesktopBridge } from '@/desktop/__tests__/sessionFixtures'

let api: (typeof import('../client'))['default']
let native: ReturnType<typeof vi.fn>
let session: typeof import('@/desktop/session')
let raw: { client: AxiosInstance; onStorageSync: EventListener }

beforeEach(async () => {
  vi.resetModules()
  vi.stubEnv('VITE_API_URL', 'https://remote-deployment.example')
  native = vi.fn().mockResolvedValue(loginResponse('native'))
  setDesktopBridge({ authenticate: native })
  api = (await import('../client')).default
  session = await import('@/desktop/session')
  raw = api as unknown as typeof raw
  api.setToken(sessionToken('expired'))
})

afterEach(() => {
  api.clearAuth()
  window.removeEventListener('storage', raw.onStorageSync)
  Reflect.deleteProperty(window, '__AETHER_DESKTOP__')
  vi.unstubAllEnvs()
  vi.restoreAllMocks()
})

describe('desktop API session recovery', () => {
  it('keeps authenticated requests and stream URLs local even when the build has a remote API URL', async () => {
    const { getApiUrl } = await import('@/utils/url')
    const requests: InternalAxiosRequestConfig[] = []
    raw.client.defaults.adapter = async config => {
      requests.push(config)
      return apiResponse(config, {})
    }
    await api.get('/api/users/me')
    expect(requests[0].baseURL).toBe('')
    expect(requests[0].url).toBe('/api/users/me')
    expect(getApiUrl('/api/admin/stream')).toBe('/api/admin/stream')
    Reflect.deleteProperty(window, '__AETHER_DESKTOP__')
    expect(getApiUrl('/api/admin/stream')).toBe('https://remote-deployment.example/api/admin/stream')
  })

  it('tries the existing bodyless Cookie refresh before native authentication', async () => {
    const seen: Array<{ url?: string; body: unknown; auth: unknown }> = []
    raw.client.defaults.adapter = async config => {
      seen.push({ url: config.url, body: config.data, auth: config.headers.Authorization })
      if (config.url === '/api/auth/refresh') return apiResponse(config, loginResponse('cookie'))
      return apiResponse(config, { ok: true }, config.headers.Authorization === `Bearer ${sessionToken('cookie')}` ? 200 : 401)
    }
    await expect(api.get('/api/admin/data')).resolves.toMatchObject({ data: { ok: true } })
    expect(seen.map(request => request.url)).toEqual(['/api/admin/data', '/api/auth/refresh', '/api/admin/data'])
    expect(seen[1].body).toBeUndefined()
    expect(native).not.toHaveBeenCalled()
    expect(api.getToken()).toBe(sessionToken('cookie'))
  })

  it('deduplicates concurrent 401s, then retries with a native session and the same device header', async () => {
    const exchange = deferred<unknown>()
    native.mockReturnValue(exchange.promise)
    let refreshCalls = 0
    const retriedDevices: unknown[] = []
    raw.client.defaults.adapter = async config => {
      if (config.url === '/api/auth/refresh') {
        refreshCalls += 1
        return apiResponse(config, {}, 401)
      }
      if (config.headers.Authorization === `Bearer ${sessionToken('native')}`) {
        retriedDevices.push(config.headers['X-Client-Device-Id'])
        return apiResponse(config, { ok: true })
      }
      return apiResponse(config, {}, 401)
    }
    const first = api.get('/api/admin/first')
    const second = api.get('/api/admin/second')
    await vi.waitFor(() => expect(native).toHaveBeenCalledTimes(1))
    exchange.resolve(loginResponse('native'))
    await Promise.all([first, second])
    expect(refreshCalls).toBe(1)
    expect(retriedDevices).toEqual([native.mock.calls[0][0], native.mock.calls[0][0]])
    expect(api.getToken()).toBe(sessionToken('native'))
  })

  it('does not start another exchange for a late 401 issued with the previous token', async () => {
    const oldResponse = deferred<void>()
    let refreshCalls = 0
    raw.client.defaults.adapter = async config => {
      if (config.url === '/api/auth/refresh') {
        refreshCalls += 1
        return apiResponse(config, {}, 401)
      }
      if (config.headers.Authorization === `Bearer ${sessionToken('native')}`) return apiResponse(config, {})
      if (config.url === '/api/admin/slow') await oldResponse.promise
      return apiResponse(config, {}, 401)
    }
    const slow = api.get('/api/admin/slow')
    await api.get('/api/admin/fast')
    oldResponse.resolve()
    await slow
    expect(refreshCalls).toBe(1)
    expect(native).toHaveBeenCalledTimes(1)
  })

  it('stops on a second 401 and latches failure without a login redirect or another recovery loop', async () => {
    let refreshCalls = 0
    raw.client.defaults.adapter = async config => {
      if (config.url === '/api/auth/refresh') refreshCalls += 1
      return apiResponse(config, {}, 401)
    }
    const originalPath = window.location.pathname
    await expect(api.get('/api/admin/data')).rejects.toBeInstanceOf(AxiosError)
    expect(session.desktopSessionState.value.phase).toBe('failed')
    expect(api.getToken()).toBeNull()
    await expect(api.get('/api/admin/data')).rejects.toBeInstanceOf(AxiosError)
    expect(native).toHaveBeenCalledTimes(1)
    expect(refreshCalls).toBe(1)
    expect(window.location.pathname).toBe(originalPath)
  })

  it('latches native recovery failures and clears the expired token', async () => {
    native.mockRejectedValue(new Error('native session unavailable'))
    raw.client.defaults.adapter = async config => apiResponse(config, {}, 401)
    await expect(api.get('/api/admin/data')).rejects.toBeInstanceOf(AxiosError)
    expect(api.getToken()).toBeNull()
    expect(session.desktopSessionState.value.phase).toBe('failed')
    await expect(api.get('/api/admin/data')).rejects.toBeInstanceOf(AxiosError)
    expect(native).toHaveBeenCalledTimes(1)
  })

  it.each(['network', 'disabled'])('shows a connection failure for %s errors without opening login', async errorKind => {
    const originalPath = window.location.pathname
    raw.client.defaults.adapter = async config => {
      if (errorKind === 'network') throw new AxiosError('Network error', 'ERR_NETWORK', config)
      return apiResponse(config, { detail: '用户已禁用' }, 403)
    }
    await expect(api.get('/api/admin/data')).rejects.toBeInstanceOf(AxiosError)
    expect(session.desktopSessionState.value.phase).toBe('failed')
    expect(api.getToken()).toBeNull()
    expect(window.location.pathname).toBe(originalPath)
    expect(native).not.toHaveBeenCalled()
  })

  it('does not let a delayed failure from a previous session invalidate a successful reconnect', async () => {
    const oldRetry = deferred<void>()
    let retryStarted = false
    raw.client.defaults.adapter = async config => {
      if (config.url === '/api/auth/refresh') return apiResponse(config, {}, 401)
      if (config.headers.Authorization === `Bearer ${sessionToken('native')}`) {
        retryStarted = true
        await oldRetry.promise
      }
      return apiResponse(config, {}, 401)
    }
    const oldRequest = api.get('/api/admin/old').catch(error => error)
    await vi.waitFor(() => expect(retryStarted).toBe(true))
    session.failDesktopSession()
    api.clearAuth()
    native.mockResolvedValue(loginResponse('retry'))
    const response = await session.authenticateDesktopSession({ retry: true })
    api.setToken(response.access_token)
    oldRetry.resolve()
    expect(await oldRequest).toBeInstanceOf(AxiosError)
    expect(api.getToken()).toBe(sessionToken('retry'))
    expect(session.desktopSessionState.value.phase).toBe('authenticated')
  })

  it('does not let an old network failure interrupt a reconnect while its new token is pending', async () => {
    const oldResponse = deferred<void>()
    const exchange = deferred<unknown>()
    native.mockReturnValue(exchange.promise)
    raw.client.defaults.adapter = async config => {
      if (config.url === '/api/admin/slow') await oldResponse.promise
      throw new AxiosError('Network error', 'ERR_NETWORK', config)
    }
    const slow = api.get('/api/admin/slow').catch(error => error)
    await expect(api.get('/api/admin/first')).rejects.toBeInstanceOf(AxiosError)
    expect(api.getToken()).toBeNull()
    const retry = session.authenticateDesktopSession({ retry: true })
    expect(session.desktopSessionState.value.phase).toBe('connecting')
    oldResponse.resolve()
    expect(await slow).toBeInstanceOf(AxiosError)
    expect(session.desktopSessionState.value.phase).toBe('connecting')
    exchange.resolve(loginResponse('retry'))
    const response = await retry
    api.setToken(response.access_token)
    expect(api.getToken()).toBe(sessionToken('retry'))
    expect(session.desktopSessionState.value.phase).toBe('authenticated')
    expect(native).toHaveBeenCalledTimes(1)
  })
})
