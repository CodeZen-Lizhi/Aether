import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, type App } from 'vue'

import type { Model, ProviderWithEndpointsSummary } from '@/api/endpoints'
import { updateModel } from '@/api/endpoints/models'
import ModelMappingTab from '../provider-tabs/ModelMappingTab.vue'

const feedback = vi.hoisted(() => ({
  error: vi.fn(),
  success: vi.fn(),
}))

vi.mock('@/api/endpoints/models', () => ({ updateModel: vi.fn() }))
vi.mock('@/composables/useToast', () => ({ useToast: () => feedback }))

const provider = {
  id: 'provider-demo',
  name: 'Demo Provider',
  provider_type: 'custom',
  is_active: true,
  active_keys: 0,
  api_formats: [],
} as unknown as ProviderWithEndpointsSummary

const model = {
  id: 'model-demo',
  provider_id: provider.id,
  global_model_id: 'global-demo',
  provider_model_name: 'client-demo',
  global_model_display_name: 'Client Demo',
  provider_model_mappings: [
    { name: 'upstream-alpha', priority: 1 },
    { name: 'upstream-beta', priority: 2 },
    { name: 'upstream-gamma', priority: 3 },
  ],
  is_active: true,
  is_available: true,
  created_at: '2026-10-29T00:00:00Z',
  updated_at: '2026-10-29T00:00:00Z',
} as Model

const mountedApps: Array<{ app: App, root: HTMLElement }> = []

function pointerEvent(type: string, clientY: number): MouseEvent {
  const event = new MouseEvent(type, {
    bubbles: true,
    cancelable: true,
    button: 0,
    clientX: 0,
    clientY,
  })
  Object.defineProperty(event, 'pointerId', { value: 1 })
  return event
}

function mockRowRect(element: HTMLElement, top: number) {
  element.getBoundingClientRect = () => ({
    top,
    bottom: top + 32,
    height: 32,
    left: 0,
    right: 240,
    width: 240,
    x: 0,
    y: top,
    toJSON: () => ({}),
  })
}

async function mountMappingTab() {
  const refresh = vi.fn()
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp(ModelMappingTab, {
    provider,
    models: [model],
    mappingPreview: null,
    loading: false,
    onRefresh: refresh,
  })
  app.mount(root)
  mountedApps.push({ app, root })
  await nextTick()
  return { root, refresh }
}

beforeEach(() => {
  vi.resetAllMocks()
  vi.mocked(updateModel).mockResolvedValue(model)
  vi.stubGlobal('ResizeObserver', class {
    observe() {}
    unobserve() {}
    disconnect() {}
  })
})

afterEach(() => {
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
  vi.unstubAllGlobals()
})

describe('ModelMappingTab mapping priority dragging', () => {
  it('reorders mappings only through the drag handle and saves contiguous priorities', async () => {
    const { root, refresh } = await mountMappingTab()
    root.querySelector<HTMLElement>('[data-mapping-group-toggle]')!.click()
    await nextTick()

    const rows = [...root.querySelectorAll<HTMLElement>('[data-mapping-name]')]
    expect(rows.map(row => row.dataset.mappingName)).toEqual([
      'upstream-alpha',
      'upstream-beta',
      'upstream-gamma',
    ])
    rows.forEach((row, index) => mockRowRect(row, index * 40))
    expect(root.querySelector('[aria-label^="上移"]')).toBeNull()
    expect(root.querySelector('[aria-label^="下移"]')).toBeNull()

    const handle = root.querySelector<HTMLElement>(
      '[aria-label="拖动 upstream-alpha 调整优先级"]',
    )
    expect(handle).not.toBeNull()
    handle!.dispatchEvent(pointerEvent('pointerdown', 10))
    window.dispatchEvent(pointerEvent('pointermove', 65))
    await nextTick()
    window.dispatchEvent(pointerEvent('pointerup', 65))

    await vi.waitFor(() => expect(updateModel).toHaveBeenCalledOnce())
    expect(updateModel).toHaveBeenCalledWith('provider-demo', 'model-demo', {
      provider_model_mappings: [
        { name: 'upstream-beta', priority: 1 },
        { name: 'upstream-alpha', priority: 2 },
        { name: 'upstream-gamma', priority: 3 },
      ],
    })
    expect(feedback.success).toHaveBeenCalledWith('映射优先级已更新')
    expect(refresh).toHaveBeenCalledOnce()
  })
})
