import { createApp, nextTick, type App } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useDesktopGateway } from '../useDesktopGateway'

const { statusCommand, startCommand } = vi.hoisted(() => ({
  statusCommand: vi.fn(),
  startCommand: vi.fn(),
}))

vi.mock('../bridge', () => ({
  desktopApi: {
    status: statusCommand,
    start: startCommand,
    stop: vi.fn(),
    restart: vi.fn(),
    setPort: vi.fn(),
    setAutostart: vi.fn(),
    openDashboard: vi.fn(),
    openDataDir: vi.fn(),
    openLogDir: vi.fn(),
    quit: vi.fn(),
    logs: vi.fn(),
  },
  describeDesktopError: (error: unknown) => error instanceof Error ? error.message : String(error),
}))

const mounted: Array<{ app: App; root: HTMLElement }> = []

const runningStatus = {
  phase: 'running', configured: true, port: 8084, gateway_url: 'http://127.0.0.1:8084',
  data_dir: '/tmp/data', log_dir: '/tmp/logs', autostart: false, pid: 42, error: null, version: '1.0.0',
} as const

async function settle() {
  for (let index = 0; index < 8; index += 1) {
    await Promise.resolve()
    await nextTick()
  }
}

function mountGateway() {
  const exposed: { gateway?: ReturnType<typeof useDesktopGateway> } = {}
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp({
    setup() {
      exposed.gateway = useDesktopGateway()
      return () => null
    },
  })
  app.mount(root)
  mounted.push({ app, root })
  return exposed.gateway!
}

beforeEach(() => {
  statusCommand.mockReset()
  startCommand.mockReset()
})

afterEach(() => {
  for (const { app, root } of mounted.splice(0)) {
    app.unmount()
    root.remove()
  }
})

describe('useDesktopGateway recovery state', () => {
  it('allows starting when the status request fails and no process is known', async () => {
    statusCommand.mockRejectedValue(new Error('无法读取网关状态'))
    startCommand.mockResolvedValue(runningStatus)

    const gateway = mountGateway()
    await settle()

    expect(gateway.status.value).toBeNull()
    expect(gateway.connectionError.value).toBe('无法读取网关状态')
    expect(gateway.canStart.value).toBe(true)

    await gateway.start()
    expect(startCommand).toHaveBeenCalledOnce()
    expect(gateway.status.value?.phase).toBe('running')
  })
})
