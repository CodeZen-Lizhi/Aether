import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, type App } from 'vue'
import { createI18n, setI18nLocale } from '@/i18n'
import DesktopApp from '../DesktopApp.vue'
import type { DesktopStatus } from '../bridge'

const { nativeInvoke, nativeAvailable } = vi.hoisted(() => ({ nativeInvoke: vi.fn(), nativeAvailable: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: nativeInvoke, isTauri: nativeAvailable }))

const mountedApps: Array<{ app: App; root: HTMLElement }> = []
const status = (overrides: Partial<DesktopStatus> = {}): DesktopStatus => ({
  phase: 'stopped', configured: true, port: 8084, gateway_url: 'http://127.0.0.1:8084',
  data_dir: '/test/data', log_dir: '/test/logs', autostart: false, pid: null, error: null, version: '1.0.0',
  ...overrides,
})
const running = status({ phase: 'running', pid: 42 })
const initialSetup = status({ phase: 'setup', configured: false })

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => { resolve = resolvePromise; reject = rejectPromise })
  return { promise, resolve, reject }
}

async function settle() {
  for (let index = 0; index < 12; index += 1) {
    await Promise.resolve()
    await nextTick()
  }
}

async function mountApp() {
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp(DesktopApp).use(createI18n())
  app.mount(root)
  mountedApps.push({ app, root })
  await settle()
  return root
}

function button(root: HTMLElement, label: string) {
  const match = Array.from(root.querySelectorAll('button')).find(element => element.textContent?.trim() === label)
  expect(match, `button: ${label}`).toBeDefined()
  return match!
}

beforeEach(() => {
  vi.useFakeTimers()
  nativeInvoke.mockReset()
  nativeAvailable.mockReturnValue(true)
  nativeInvoke.mockImplementation(async (command: string) => command === 'desktop_logs' ? [] : status())
})

afterEach(() => {
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
  vi.clearAllTimers()
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe('desktop initialization', () => {
  it('shows first-run status without an account form or duplicating the host startup', async () => {
    nativeInvoke.mockResolvedValue(initialSetup)
    const root = await mountApp()
    expect(root.textContent).toContain('首次启动会自动准备本地数据')
    expect(root.textContent).not.toMatch(/账号|用户名|密码|登录管理/)
    expect(nativeInvoke).toHaveBeenCalledTimes(1)
    nativeInvoke.mockResolvedValue(running)
    await vi.advanceTimersByTimeAsync(3000)
    expect(root.textContent).toContain('网关正在运行')
    expect(nativeInvoke.mock.calls.every(([command]) => command === 'desktop_status')).toBe(true)
  })

  it('starts an uninitialized profile without arguments and opens the dashboard once', async () => {
    let current = initialSetup
    nativeInvoke.mockImplementation(async (command: string) => {
      if (command === 'desktop_start') current = running
      return current
    })
    const root = await mountApp()
    button(root, '启动网关').click()
    await settle()
    expect(nativeInvoke).toHaveBeenCalledWith('desktop_start', undefined)
    expect(nativeInvoke).toHaveBeenCalledWith('desktop_open_dashboard', undefined)
    await vi.advanceTimersByTimeAsync(9000)
    expect(nativeInvoke.mock.calls.filter(([command]) => command === 'desktop_open_dashboard')).toHaveLength(1)
  })

})

describe('gateway lifecycle and preferences', () => {
  it('shows pending state, prevents duplicate starts, and enables controls from the returned state', async () => {
    const start = deferred<DesktopStatus>()
    nativeInvoke.mockImplementation(async (command: string) => command === 'desktop_start' ? start.promise : status())
    const root = await mountApp()
    button(root, '启动网关').click()
    await settle()
    expect(root.textContent).toContain('正在启动网关')
    expect(button(root, '正在启动…').disabled).toBe(true)
    button(root, '正在启动…').click()
    expect(nativeInvoke.mock.calls.filter(([command]) => command === 'desktop_start')).toHaveLength(1)
    start.resolve(running)
    await settle()
    expect(root.textContent).toContain('网关正在运行')
    expect(button(root, '打开管理界面').disabled).toBe(false)
    expect(button(root, '停止网关').disabled).toBe(false)
  })

  it('allows stop to interrupt a pending start without stale action errors winning', async () => {
    const start = deferred<DesktopStatus>()
    nativeInvoke.mockImplementation(async (command: string) => {
      if (command === 'desktop_start') return start.promise
      if (command === 'desktop_stop') return status()
      return status()
    })
    const root = await mountApp()
    button(root, '启动网关').click()
    await settle()
    button(root, '停止网关').click()
    await settle()
    expect(nativeInvoke).toHaveBeenCalledWith('desktop_stop', undefined)
    expect(button(root, '启动网关').disabled).toBe(false)

    start.reject(new Error('网关启动已取消'))
    await settle()
    expect(root.textContent).not.toContain('网关启动已取消')
    expect(button(root, '启动网关').disabled).toBe(false)
  })

  it('shows a stop operation until completion and sends restart through its own command', async () => {
    const stop = deferred<DesktopStatus>()
    nativeInvoke.mockImplementation(async (command: string) => command === 'desktop_stop' ? stop.promise : running)
    const root = await mountApp()
    button(root, '重启').click()
    await settle()
    expect(nativeInvoke).toHaveBeenCalledWith('desktop_restart', undefined)
    button(root, '停止网关').click()
    await settle()
    expect(root.textContent).toContain('正在安全停止网关')
    expect(button(root, '正在停止…').disabled).toBe(true)
    stop.resolve(status())
    await settle()
    expect(button(root, '启动网关').disabled).toBe(false)
  })

  it('offers retry and diagnostics for a failed gateway instead of hiding the native error', async () => {
    nativeInvoke.mockResolvedValue(status({ phase: 'failed', error: '网关进程意外退出。' }))
    const root = await mountApp()
    expect(root.textContent).toContain('网关进程意外退出。')
    expect(root.textContent).toContain('恢复网关')
    expect(root.textContent).not.toContain('登录 Mac 时自动启动')
    expect(root.querySelector('#gateway-port')).not.toBeNull()
    expect(root.querySelector<HTMLDetailsElement>('details')?.open).toBe(true)
    expect(button(root, '启动网关').disabled).toBe(false)
    expect(Array.from(root.querySelectorAll('button')).find(element => element.textContent?.trim() === '打开管理界面')).toBeUndefined()
  })
})

describe('status synchronization and bridge failures', () => {
  it('does not let a stale poll overwrite a newer user action', async () => {
    const oldPoll = deferred<DesktopStatus>()
    let polls = 0
    nativeInvoke.mockImplementation(async (command: string) => {
      if (command === 'desktop_status') return polls++ === 0 ? status() : oldPoll.promise
      if (command === 'desktop_start') return running
    })
    const root = await mountApp()
    await vi.advanceTimersByTimeAsync(3000)
    button(root, '启动网关').click()
    await settle()
    oldPoll.resolve(status())
    await settle()
    expect(root.textContent).toContain('网关正在运行')
    expect(button(root, '打开管理界面').disabled).toBe(false)
  })

  it('uses non-overlapping transient/stable polls and stops polling after unmount', async () => {
    const secondPoll = deferred<DesktopStatus>()
    let polls = 0
    nativeInvoke.mockImplementation(async () => {
      polls += 1
      if (polls === 1) return status({ phase: 'starting' })
      if (polls === 2) return secondPoll.promise
      return running
    })
    await mountApp()
    await vi.advanceTimersByTimeAsync(999)
    expect(polls).toBe(1)
    await vi.advanceTimersByTimeAsync(1)
    expect(polls).toBe(2)
    await vi.advanceTimersByTimeAsync(10000)
    expect(polls).toBe(2)
    secondPoll.resolve(running)
    await settle()
    await vi.advanceTimersByTimeAsync(2999)
    expect(polls).toBe(2)
    await vi.advanceTimersByTimeAsync(1)
    expect(polls).toBe(3)
    const mounted = mountedApps.pop()!
    mounted.app.unmount()
    mounted.root.remove()
    await vi.advanceTimersByTimeAsync(20000)
    expect(polls).toBe(3)
  })

  it('reports malformed status and disables actions until a valid response is received', async () => {
    const root = await mountApp()
    nativeInvoke.mockResolvedValue({ phase: 'running' })
    await vi.advanceTimersByTimeAsync(3000)
    expect(root.textContent).toContain('无法确认网关状态')
    expect(root.textContent).toContain('客户端返回了无法识别的状态')
    expect(button(root, '启动网关').disabled).toBe(true)
    nativeInvoke.mockResolvedValue(running)
    button(root, '重试连接').click()
    await settle()
    expect(button(root, '打开管理界面').disabled).toBe(false)
  })

  it('shows a clear browser-only error and never invents first-run or running state', async () => {
    nativeAvailable.mockReturnValue(false)
    const root = await mountApp()
    expect(root.textContent).toContain('浏览器无法管理本机网关')
    expect(root.querySelector('.desktop-setup')).toBeNull()
    expect(root.textContent).not.toContain('网关正在运行')
    expect(nativeInvoke).not.toHaveBeenCalled()
  })

  it('translates desktop status and settings through the existing locale mechanism', async () => {
    setI18nLocale('en-US')
    nativeInvoke.mockImplementation(async (command: string) => command === 'desktop_logs' ? ['Gateway ready'] : running)
    const root = await mountApp()
    await vi.advanceTimersByTimeAsync(20)
    expect(root.textContent).toContain('Your gateway is running')
    expect(root.textContent).toContain('Open dashboard')
    expect(root.textContent).not.toContain('Client settings')
    const diagnostics = root.querySelector<HTMLDetailsElement>('details')!
    diagnostics.open = true
    diagnostics.dispatchEvent(new Event('toggle'))
    await settle()
    expect(root.querySelector('pre')?.getAttribute('aria-label')).toBe('Recent diagnostic logs')
  })
})
