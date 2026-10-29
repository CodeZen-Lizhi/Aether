import { afterEach, describe, expect, it, vi } from 'vitest'
import { createApp, h, nextTick } from 'vue'

import type { Model, ProviderEndpoint, ProviderWithEndpointsSummary } from '@/api/endpoints'
import type { EndpointAPIKey } from '@/api/endpoints/keys'
import ModelsTab from '../ModelsTab.vue'

const modelTestStart = vi.hoisted(() => vi.fn())
const getProviderKeys = vi.hoisted(() => vi.fn())

vi.mock('@/composables/useModelTest', async () => {
  const { ref: createRef } = await import('vue')
  return {
    useModelTest: () => ({
      dialogOpen: createRef(true),
      testResult: createRef(null),
      testMode: createRef('global'),
      testing: createRef(false),
      testTrace: createRef(null),
      requestId: createRef(null),
      startTest: modelTestStart,
      stopPolling: vi.fn(),
      resetState: vi.fn(),
    }),
  }
})

vi.mock('@/api/endpoints/keys', () => ({ getProviderKeys }))
vi.mock('@/api/endpoints/models', () => ({ updateModel: vi.fn() }))
vi.mock('@/composables/useToast', () => ({
  useToast: () => ({ error: vi.fn(), success: vi.fn() }),
}))
vi.mock('@/composables/useClipboard', () => ({
  useClipboard: () => ({ copyToClipboard: vi.fn() }),
}))

const provider = {
  id: 'provider-1',
  name: 'Provider',
  provider_type: 'custom',
  active_keys: 1,
} as ProviderWithEndpointsSummary

const endpoint = {
  id: 'endpoint-1',
  api_format: 'openai:responses',
  base_url: 'https://api.example/v1',
  is_active: true,
  active_keys: 1,
} as ProviderEndpoint

const key = {
  id: 'key-1',
  name: 'key-1',
  api_formats: ['openai:responses'],
  allowed_models: null,
  is_active: true,
  auth_type: 'api_key',
  api_key_masked: 'sk-***',
} as unknown as EndpointAPIKey

const model = {
  id: 'model-1',
  provider_id: 'provider-1',
  global_model_id: 'global-1',
  global_model_name: 'gpt-5.6',
  provider_model_name: 'gpt-5.6',
  provider_model_mappings: [
    { name: 'mapping-a', priority: 10 },
    { name: 'mapping-b', priority: 20 },
  ],
  is_active: true,
  is_available: true,
  created_at: '2026-10-29T00:00:00Z',
  updated_at: '2026-10-29T00:00:00Z',
} as Model

const mounted: Array<{ app: ReturnType<typeof createApp>, root: HTMLElement }> = []

function mountModelsTab() {
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp({
    render: () => h(ModelsTab, {
      provider,
      models: [model],
      endpoints: [endpoint],
      providerKeys: [key],
    }),
  })
  app.mount(root)
  mounted.push({ app, root })
}

function findButton(title: string, text?: string): HTMLButtonElement {
  const button = [...document.querySelectorAll<HTMLButtonElement>('button')].find((item) => (
    (title ? item.title === title : true)
    && (text ? item.textContent?.trim() === text : true)
  ))
  expect(button).toBeDefined()
  return button!
}

afterEach(() => {
  for (const { app, root } of mounted.splice(0)) {
    app.unmount()
    root.remove()
  }
  vi.clearAllMocks()
})

describe('ModelsTab model testing', () => {
  it('keeps automatic mapping enabled when starting from the model list', async () => {
    getProviderKeys.mockResolvedValue([key])
    modelTestStart.mockResolvedValue(undefined)
    mountModelsTab()

    findButton('测试模型').click()
    await nextTick()
    await vi.waitFor(() => expect(getProviderKeys).toHaveBeenCalledWith('provider-1'))
    await nextTick()
    findButton('', '开始测试').click()

    await vi.waitFor(() => expect(modelTestStart).toHaveBeenCalledTimes(1))
    expect(modelTestStart.mock.calls[0][0]).toMatchObject({
      mode: 'global',
      modelName: 'gpt-5.6',
      applyModelMapping: true,
      apiKeyIds: [],
    })
    expect(modelTestStart.mock.calls[0][0].mappedModelNames).toBeUndefined()
  })
})
