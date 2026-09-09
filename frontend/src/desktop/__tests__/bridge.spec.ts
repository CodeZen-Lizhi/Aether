import { beforeEach, describe, expect, it, vi } from 'vitest'
import { desktopApi, type DesktopStatus } from '../bridge'
import { validatePort } from '../validation'

const { nativeInvoke, nativeAvailable } = vi.hoisted(() => ({ nativeInvoke: vi.fn(), nativeAvailable: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: nativeInvoke, isTauri: nativeAvailable }))

const ready: DesktopStatus = {
  phase: 'running', configured: true, port: 8084, gateway_url: 'http://127.0.0.1:8084',
  data_dir: '/test/data', log_dir: '/test/logs', autostart: false, pid: 42, error: null, version: '1.0.0',
}

beforeEach(() => {
  nativeInvoke.mockReset()
  nativeAvailable.mockReturnValue(true)
})

describe('desktop IPC boundary', () => {
  it('uses the native command names and explicit argument fields', async () => {
    nativeInvoke.mockResolvedValue(ready)
    await desktopApi.start()
    await desktopApi.setPort(8085)
    await desktopApi.setAutostart(true)
    await desktopApi.openDashboard()
    expect(nativeInvoke.mock.calls).toEqual([
      ['desktop_start', undefined],
      ['desktop_set_port', { port: 8085 }],
      ['desktop_set_autostart', { enabled: true }],
      ['desktop_open_dashboard', undefined],
    ])
  })

  it.each([
    null,
    { phase: 'running' },
    { ...ready, phase: 'unknown' },
    { ...ready, port: 0 },
    { ...ready, gateway_url: 'http://example.com:8084' },
    { ...ready, gateway_url: 'http://127.0.0.1:8085' },
    { ...ready, error: { detail: 'failure' } },
    { ...ready, pid: -1 },
  ])('rejects incompatible native state instead of rendering a successful gateway: %j', async payload => {
    nativeInvoke.mockResolvedValue(payload)
    await expect(desktopApi.status()).rejects.toThrow('无法识别的状态')
  })

  it('clearly reports a missing native bridge without attempting IPC', async () => {
    nativeAvailable.mockReturnValue(false)
    await expect(desktopApi.status()).rejects.toThrow('浏览器无法管理本机网关')
    expect(nativeInvoke).not.toHaveBeenCalled()
  })

  it('bounds diagnostics and rejects non-text log payloads', async () => {
    nativeInvoke.mockResolvedValue(Array.from({ length: 150 }, (_, index) => `line ${index} ${'a'.repeat(5000)}`))
    const logs = await desktopApi.logs()
    expect(logs).toHaveLength(100)
    expect(logs[0]).toMatch(/^line 50 /)
    expect(logs.every(line => line.length === 4096)).toBe(true)
    nativeInvoke.mockResolvedValue(['valid line', { password: 'must not appear in an error' }])
    await expect(desktopApi.logs()).rejects.toThrow('无法读取诊断日志')
  })
})

describe('gateway port validation', () => {
  it('requires an unprivileged integer port', () => {
    for (const port of ['', '0', '1023', '65536', '8084.5', '8e3', 'abc']) expect(validatePort(port)).not.toBe('')
    expect(validatePort('1024')).toBe('')
    expect(validatePort('65535')).toBe('')
  })
})
