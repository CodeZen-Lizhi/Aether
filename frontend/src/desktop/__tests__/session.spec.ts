import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { deferred, loginResponse, setDesktopBridge } from './sessionFixtures'
import { getClientDeviceId } from '@/utils/deviceId'

let session: typeof import('../session')

beforeEach(async () => {
  vi.resetModules()
  session = await import('../session')
})

afterEach(() => { Reflect.deleteProperty(window, '__AETHER_DESKTOP__') })

describe('desktop session adapter', () => {
  it.each([undefined, true, {}, [], { authenticate: true }])('does not treat an ordinary marker as a native session: %j', async marker => {
    setDesktopBridge(marker)
    expect(session.hasDesktopSession()).toBe(false)
    await expect(session.authenticateDesktopSession()).rejects.toThrow('请在 Aether macOS 客户端中打开管理界面')
  })

  it('accepts the real local login response, including a null email, and uses the API device identity', async () => {
    const authenticate = vi.fn().mockResolvedValue(loginResponse())
    setDesktopBridge(Object.freeze({ authenticate }))
    const deviceId = getClientDeviceId()
    expect(session.hasDesktopSession()).toBe(true)
    await expect(session.authenticateDesktopSession()).resolves.toEqual(loginResponse())
    expect(authenticate).toHaveBeenCalledExactlyOnceWith(deviceId)
    expect(session.desktopSessionState.value.phase).toBe('authenticated')
  })

  it.each([null, [], {}, { access_token: ' ' }, { access_token: 123 },
    { ...loginResponse(), expires_in: -1 }, { ...loginResponse(), email: 123 },
    { ...loginResponse(), role: { admin: true } },
  ])('rejects malformed native session payloads: %j', async response => {
    setDesktopBridge({ authenticate: vi.fn().mockResolvedValue(response) })
    await expect(session.authenticateDesktopSession()).rejects.toThrow('无法识别的会话')
    expect(session.desktopSessionState.value.phase).toBe('failed')
  })

  it('shares one exchange for concurrent startup and recovery requests', async () => {
    const exchange = deferred<unknown>()
    const authenticate = vi.fn(() => exchange.promise)
    setDesktopBridge({ authenticate })
    const first = session.authenticateDesktopSession()
    const second = session.authenticateDesktopSession()
    expect(first).toBe(second)
    await Promise.resolve()
    expect(authenticate).toHaveBeenCalledTimes(1)
    exchange.resolve(loginResponse())
    await expect(first).resolves.toEqual(loginResponse())
  })

  it('latches native failures without leaking their detail until an explicit retry', async () => {
    const authenticate = vi.fn().mockRejectedValueOnce(new Error('private native request detail')).mockResolvedValue(loginResponse('retry'))
    setDesktopBridge({ authenticate })
    await expect(session.authenticateDesktopSession()).rejects.toThrow('无法连接本机网关')
    await expect(session.authenticateDesktopSession()).rejects.toThrow('无法连接本机网关')
    expect(authenticate).toHaveBeenCalledTimes(1)
    expect(session.desktopSessionState.value.error).not.toContain('private')
    await expect(session.authenticateDesktopSession({ retry: true })).resolves.toEqual(loginResponse('retry'))
    expect(authenticate).toHaveBeenCalledTimes(2)
  })

  it('does not let a late exchange restore an invalidated session', async () => {
    const exchange = deferred<unknown>()
    setDesktopBridge({ authenticate: vi.fn(() => exchange.promise) })
    const request = session.authenticateDesktopSession()
    session.failDesktopSession()
    exchange.resolve(loginResponse())
    await expect(request).rejects.toThrow('无法连接本机网关')
    expect(session.desktopSessionState.value.phase).toBe('failed')
  })

  it('completes one explicit retry even while the invalidated exchange is still in flight', async () => {
    const exchange = deferred<unknown>()
    const authenticate = vi.fn().mockReturnValueOnce(exchange.promise).mockResolvedValue(loginResponse('retry'))
    setDesktopBridge({ authenticate })
    const first = session.authenticateDesktopSession()
    session.failDesktopSession()
    const retry = session.authenticateDesktopSession({ retry: true })
    exchange.resolve(loginResponse('stale'))
    await expect(first).rejects.toThrow('无法连接本机网关')
    await expect(retry).resolves.toEqual(loginResponse('retry'))
    expect(authenticate).toHaveBeenCalledTimes(2)
    expect(session.desktopSessionState.value.phase).toBe('authenticated')
  })
})
