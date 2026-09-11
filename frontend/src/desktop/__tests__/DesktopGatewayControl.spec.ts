import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, type App } from 'vue'
import DesktopGatewayControl from '../DesktopGatewayControl.vue'

const actions = vi.hoisted(() => ({
  refreshStatus: vi.fn(), start: vi.fn(), stop: vi.fn(), restart: vi.fn(),
}))
const gatewayState = vi.hoisted(() => ({
  phase: undefined as ReturnType<typeof import('vue')['ref']> | undefined,
  status: undefined as ReturnType<typeof import('vue')['ref']> | undefined,
  loading: undefined as ReturnType<typeof import('vue')['ref']> | undefined,
  pendingAction: undefined as ReturnType<typeof import('vue')['ref']> | undefined,
  connectionError: undefined as ReturnType<typeof import('vue')['ref']> | undefined,
  errors: undefined as ReturnType<typeof import('vue')['ref']> | undefined,
}))
vi.mock('../useDesktopGateway', async () => {
  const { computed, ref } = await import('vue')
  const phase = ref<'starting' | 'running' | 'stopped' | null>('running')
  const status = ref<{ gateway_url: string; pid: number | null } | null>({
    gateway_url: 'http://127.0.0.1:8084', pid: 42,
  })
  const loading = ref(false)
  const pendingAction = ref<string | null>(null)
  const connectionError = ref('')
  const errors = ref<string[]>([])
  Object.assign(gatewayState, { phase, status, loading, pendingAction, connectionError, errors })
  return {
    useDesktopGateway: () => ({
      phase,
      status,
      loading,
      pendingAction,
      connectionError,
      busy: ref(false),
      canStart: computed(() => phase.value === 'stopped'),
      canStop: computed(() => phase.value === 'running'),
      errors,
      ...actions,
    }),
  }
})

const mounted: Array<{ app: App; root: HTMLElement }> = []

beforeEach(() => {
  gatewayState.phase!.value = 'running'
  gatewayState.status!.value = { gateway_url: 'http://127.0.0.1:8084', pid: 42 }
  gatewayState.loading!.value = false
  gatewayState.pendingAction!.value = null
  gatewayState.connectionError!.value = ''
  gatewayState.errors!.value = []
})

afterEach(() => {
  for (const { app, root } of mounted.splice(0)) {
    app.unmount()
    root.remove()
  }
  document.body.innerHTML = ''
  vi.clearAllMocks()
})

describe('desktop gateway header control', () => {
  it('shows the current state and exposes restart and stop actions', async () => {
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(DesktopGatewayControl)
    app.mount(root)
    mounted.push({ app, root })

    const trigger = root.querySelector<HTMLButtonElement>('button')!
    expect(trigger.textContent).toContain('网关')
    expect(trigger.textContent).toContain('运行中')
    expect(trigger.dataset.gatewayState).toBe('running')
    expect(trigger.classList).toContain('text-emerald-700')
    trigger.click()
    await nextTick()
    await Promise.resolve()

    const items = Array.from(document.body.querySelectorAll<HTMLElement>('[role="menuitem"]'))
    expect(items.map(item => item.textContent?.trim())).toEqual(['重启网关', '停止网关'])
    items[0].dispatchEvent(new Event('click', { bubbles: true }))
    expect(actions.restart).toHaveBeenCalledOnce()
  })

  it('keeps recovery actions reachable while the first status request is pending', async () => {
    gatewayState.phase!.value = null
    gatewayState.status!.value = null
    gatewayState.loading!.value = true

    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(DesktopGatewayControl)
    app.mount(root)
    mounted.push({ app, root })

    const trigger = root.querySelector<HTMLButtonElement>('button')!
    expect(trigger.disabled).toBe(false)
    expect(trigger.dataset.gatewayState).toBe('progress')
    expect(trigger.classList).toContain('text-sky-700')
    trigger.click()
    await nextTick()
    await Promise.resolve()

    const items = Array.from(document.body.querySelectorAll<HTMLElement>('[role="menuitem"]'))
    expect(items.map(item => item.textContent?.trim())).toEqual([
      '重新读取状态', '重启网关', '停止网关',
    ])
  })

  it('shows a failed connection without leaving an indefinite spinner', async () => {
    gatewayState.phase!.value = null
    gatewayState.status!.value = null
    gatewayState.loading!.value = false
    gatewayState.connectionError!.value = '无法读取网关状态'
    gatewayState.errors!.value = ['无法读取网关状态']

    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(DesktopGatewayControl)
    app.mount(root)
    mounted.push({ app, root })

    const trigger = root.querySelector<HTMLButtonElement>('button')!
    expect(trigger.textContent).toContain('连接失败')
    expect(trigger.dataset.gatewayState).toBe('failed')
    expect(trigger.classList).toContain('text-destructive')
    expect(trigger.querySelector('.animate-spin')).toBeNull()
    trigger.click()
    await nextTick()
    await Promise.resolve()

    const retry = Array.from(document.body.querySelectorAll<HTMLElement>('[role="menuitem"]'))
      .find(item => item.textContent?.trim() === '重新读取状态')!
    retry.dispatchEvent(new Event('click', { bubbles: true }))
    expect(actions.refreshStatus).toHaveBeenCalledOnce()
  })

  it('keeps stop enabled so a pending restart can be interrupted', async () => {
    gatewayState.phase!.value = 'starting'
    gatewayState.status!.value = { gateway_url: 'http://127.0.0.1:8084', pid: 42 }
    gatewayState.pendingAction!.value = 'restart'

    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(DesktopGatewayControl)
    app.mount(root)
    mounted.push({ app, root })

    root.querySelector<HTMLButtonElement>('button')!.click()
    await nextTick()
    await Promise.resolve()

    const items = Array.from(document.body.querySelectorAll<HTMLElement>('[role="menuitem"]'))
    const restart = items.find(item => item.textContent?.trim() === '重启网关')!
    const stop = items.find(item => item.textContent?.trim() === '停止网关')!
    expect(restart.getAttribute('data-disabled')).not.toBeNull()
    expect(stop.getAttribute('data-disabled')).toBeNull()
    stop.dispatchEvent(new Event('click', { bubbles: true }))
    expect(actions.stop).toHaveBeenCalledOnce()
  })

  it('uses a neutral square state for a stopped gateway', () => {
    gatewayState.phase!.value = 'stopped'
    gatewayState.status!.value = { gateway_url: 'http://127.0.0.1:8084', pid: null }

    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(DesktopGatewayControl)
    app.mount(root)
    mounted.push({ app, root })

    const trigger = root.querySelector<HTMLButtonElement>('button')!
    expect(trigger.textContent).toContain('已停止')
    expect(trigger.dataset.gatewayState).toBe('stopped')
    expect(trigger.classList).toContain('text-muted-foreground')
  })
})
