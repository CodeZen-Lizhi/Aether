import { afterEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, type App } from 'vue'
import DesktopGatewayControl from '../DesktopGatewayControl.vue'

const actions = vi.hoisted(() => ({ start: vi.fn(), stop: vi.fn(), restart: vi.fn() }))
vi.mock('../useDesktopGateway', async () => {
  const { computed, ref } = await import('vue')
  const phase = ref<'running' | 'stopped'>('running')
  return {
    useDesktopGateway: () => ({
      phase,
      status: ref({ gateway_url: 'http://127.0.0.1:8084' }),
      busy: ref(false),
      canStart: computed(() => phase.value === 'stopped'),
      canStop: computed(() => phase.value === 'running'),
      errors: ref<string[]>([]),
      ...actions,
    }),
  }
})

const mounted: Array<{ app: App; root: HTMLElement }> = []

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
    trigger.click()
    await nextTick()
    await Promise.resolve()

    const items = Array.from(document.body.querySelectorAll<HTMLElement>('[role="menuitem"]'))
    expect(items.map(item => item.textContent?.trim())).toEqual(['重启网关', '停止网关'])
    items[0].dispatchEvent(new Event('click', { bubbles: true }))
    expect(actions.restart).toHaveBeenCalledOnce()
  })
})
