import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, h, nextTick, ref, type App } from 'vue'

import type { ProviderWithEndpointsSummary } from '@/api/endpoints/types'
import ProviderFormDialog from '../ProviderFormDialog.vue'

const endpointMocks = vi.hoisted(() => ({
  createProvider: vi.fn(),
  updateProvider: vi.fn(),
}))

vi.mock('@/api/endpoints', () => ({
  createProvider: endpointMocks.createProvider,
  updateProvider: endpointMocks.updateProvider,
  normalizePoolAdvancedConfig: (value: unknown) => {
    if (value == null || value === false) return null
    if (value === true) return {}
    if (typeof value !== 'object' || Array.isArray(value)) return null
    return { ...value }
  },
}))

vi.mock('@/components/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/components/ui')>()
  const { defineComponent, h } = await import('vue')
  const passthrough = (name: string) => defineComponent({
    name,
    setup: (_props, { slots }) => () => slots.default?.(),
  })

  return {
    ...actual,
    Select: defineComponent({
      name: 'SelectStub',
      props: {
        modelValue: String,
        disabled: Boolean,
      },
      emits: ['update:modelValue'],
      setup: (props, { emit, slots }) => () => h('select', {
        value: props.modelValue,
        disabled: props.disabled,
        onChange: (event: Event) => emit(
          'update:modelValue',
          (event.target as HTMLSelectElement).value,
        ),
      }, slots.default?.()),
    }),
    SelectTrigger: passthrough('SelectTriggerStub'),
    SelectValue: passthrough('SelectValueStub'),
    SelectContent: passthrough('SelectContentStub'),
    SelectItem: defineComponent({
      name: 'SelectItemStub',
      props: {
        value: { type: String, required: true },
        disabled: Boolean,
      },
      setup: (props, { slots }) => () => h('option', {
        value: props.value,
        disabled: props.disabled,
      }, slots.default?.()),
    }),
  }
})

vi.mock('@/composables/useToast', () => ({
  useToast: () => ({
    success: vi.fn(),
    error: vi.fn(),
  }),
}))

const mountedApps: Array<{ app: App, root: HTMLElement }> = []

function makeProvider(
  overrides: Partial<ProviderWithEndpointsSummary> = {},
): ProviderWithEndpointsSummary {
  return {
    id: 'provider-1',
    name: 'Provider One',
    provider_type: 'custom',
    enable_format_conversion: true,
    max_transfer_count: 0,
    max_transfer_timeout_seconds: 0,
    is_active: true,
    total_endpoints: 0,
    active_endpoints: 0,
    total_keys: 0,
    active_keys: 0,
    total_models: 0,
    active_models: 0,
    global_model_ids: [],
    avg_health_score: 1,
    unhealthy_endpoints: 0,
    api_formats: [],
    endpoint_health_details: [],
    created_at: '2026-07-26T00:00:00Z',
    updated_at: '2026-07-26T00:00:00Z',
    ...overrides,
  }
}

function mountDialog(provider?: ProviderWithEndpointsSummary | null) {
  const root = document.createElement('div')
  document.body.appendChild(root)
  const open = ref(true)
  const entity = ref(provider)
  const app = createApp({
    render: () => h(ProviderFormDialog, {
      modelValue: open.value,
      provider: entity.value,
      'onUpdate:modelValue': (value: boolean) => { open.value = value },
      onProviderUpdated: (value: ProviderWithEndpointsSummary) => { entity.value = value },
    }),
  })
  app.mount(root)
  mountedApps.push({ app, root })
  return { open, entity }
}

async function settle() {
  for (let index = 0; index < 4; index += 1) {
    await Promise.resolve()
    await nextTick()
  }
}

async function setInput(selector: string, value: string) {
  const input = document.body.querySelector<HTMLInputElement>(selector)
  if (!input) throw new Error(`Missing input: ${selector}`)
  input.value = value
  input.dispatchEvent(new Event('input', { bubbles: true }))
  await nextTick()
}

function clickButton(text: string) {
  const button = [...document.body.querySelectorAll<HTMLButtonElement>('button')]
    .find(candidate => candidate.textContent?.trim() === text)
  if (!button) throw new Error(`Missing button: ${text}`)
  button.click()
}

beforeEach(() => {
  endpointMocks.createProvider.mockReset()
  endpointMocks.createProvider.mockResolvedValue({ id: 'provider-new', name: 'New Provider' })
  endpointMocks.updateProvider.mockReset()
  endpointMocks.updateProvider.mockResolvedValue(makeProvider())
})

afterEach(() => {
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
  document.body.innerHTML = ''
})

describe('ProviderFormDialog transfer limits', () => {
  it('reopens with authoritative attempts, source and budget returned after saving', async () => {
    const saved = makeProvider({
      effective_max_attempts: 3,
      effective_max_attempts_source: 'failover_rules.max_attempts',
      stream_failover_budget_ms: 45000,
      failover_rules: { max_attempts: 3, stream_failover_budget_ms: 45000 },
    })
    endpointMocks.updateProvider.mockResolvedValue(saved)
    const dialog = mountDialog(makeProvider({
      effective_max_attempts: 2,
      effective_max_attempts_source: 'provider.max_retries',
      stream_failover_budget_ms: 90000,
    }))
    await settle()
    await setInput('#chat-max-attempts', '3')
    await setInput('#stream-failover-budget', '45')
    clickButton('保存')
    await settle()
    expect(dialog.open.value).toBe(false)
    expect(dialog.entity.value).toEqual(saved)
    dialog.open.value = true
    await settle()
    expect(document.body.querySelector<HTMLInputElement>('#chat-max-attempts')?.value).toBe('3')
    expect(document.body.querySelector<HTMLInputElement>('#stream-failover-budget')?.value).toBe('45')
    expect(document.body.textContent).toContain('故障转移规则')
    clickButton('保存')
    await settle()
    expect(endpointMocks.updateProvider.mock.calls[1]?.[1]).not.toHaveProperty('failover_rules')
  })

  it('edits attempts and budget without replacing unrelated failover rules', async () => {
    mountDialog(makeProvider({
      effective_max_attempts: 2,
      effective_max_attempts_source: 'provider.max_retries',
      stream_failover_budget_ms: 90000,
      failover_rules: { stop_on_transport_errors: true, future_rule: { enabled: true } },
    }))
    await settle()
    expect(document.body.querySelector<HTMLInputElement>('#chat-max-attempts')?.value).toBe('2')
    expect(document.body.textContent).toContain('当前生效次数')
    await setInput('#chat-max-attempts', '3')
    await setInput('#stream-failover-budget', '45')
    clickButton('保存')
    await settle()
    expect(endpointMocks.updateProvider).toHaveBeenCalledWith('provider-1', expect.objectContaining({
      failover_rules: { max_attempts: 3, stream_failover_budget_ms: 45000 },
    }))
  })

  it('preserves millisecond precision when reopening and saving seconds', async () => {
    mountDialog(makeProvider({ stream_failover_budget_ms: 1001 }))
    await settle()
    expect(document.body.querySelector<HTMLInputElement>('#stream-failover-budget')?.value).toBe('1.001')
    await setInput('#stream-failover-budget', '1.001')
    clickButton('保存')
    await settle()
    expect(endpointMocks.updateProvider.mock.calls[0]?.[1]).not.toHaveProperty('failover_rules')
  })

  it('clears an explicit budget to inherit the default', async () => {
    mountDialog(makeProvider({ stream_failover_budget_ms: 300000 }))
    await settle()
    await setInput('#stream-failover-budget', '')
    clickButton('保存')
    await settle()
    expect(endpointMocks.updateProvider.mock.calls[0]?.[1]).toHaveProperty(
      'failover_rules.stream_failover_budget_ms', null,
    )
  })

  it('validates seconds against the supported millisecond range and precision', async () => {
    const dialog = mountDialog(makeProvider())
    await settle()
    for (const value of ['0', '-1', '1200.001', '0.0001', '1.0001']) {
      await setInput('#stream-failover-budget', value)
      clickButton('保存')
      await settle()
    }
    expect(endpointMocks.updateProvider).not.toHaveBeenCalled()
    for (const [seconds, milliseconds] of [['0.001', 1], ['1.001', 1001], ['1200', 1200000]] as const) {
      dialog.open.value = true
      await settle()
      await setInput('#stream-failover-budget', seconds)
      clickButton('保存')
      await settle()
      expect(endpointMocks.updateProvider).toHaveBeenLastCalledWith('provider-1', expect.objectContaining({
        failover_rules: { stream_failover_budget_ms: milliseconds },
      }))
    }
  })

  it('keeps inherited attempts absent when saving another field', async () => {
    mountDialog(makeProvider({ effective_max_attempts: 5, effective_max_attempts_source: 'provider.max_retries' }))
    await settle()
    clickButton('保存')
    await settle()
    expect(endpointMocks.updateProvider.mock.calls[0]?.[1]).not.toHaveProperty('failover_rules')
    expect(endpointMocks.updateProvider.mock.calls[0]?.[1]).not.toHaveProperty('max_retries')
  })

  it('rejects fractional or out-of-range attempts before saving', async () => {
    mountDialog(makeProvider({ effective_max_attempts: 2 }))
    await settle()
    for (const value of ['0', '100', '2.5']) {
      await setInput('#chat-max-attempts', value)
      clickButton('保存')
      await settle()
    }
    expect(endpointMocks.updateProvider).not.toHaveBeenCalled()
  })

  it('loads and submits configured limits in edit mode', async () => {
    mountDialog(makeProvider({
      max_transfer_count: 10,
      max_transfer_timeout_seconds: 60,
    }))
    await settle()

    expect(document.body.querySelector<HTMLInputElement>('#max-transfer-count')?.value).toBe('10')
    expect(document.body.querySelector<HTMLInputElement>('#max-transfer-timeout-seconds')?.value).toBe('60')

    await setInput('#max-transfer-count', '12')
    await setInput('#max-transfer-timeout-seconds', '45')
    clickButton('保存')
    await settle()

    expect(endpointMocks.updateProvider).toHaveBeenCalledWith(
      'provider-1',
      expect.objectContaining({
        max_transfer_count: 12,
        max_transfer_timeout_seconds: 45,
      }),
    )
  })

  it('shows zero limits as unlimited placeholders while submitting explicit zero', async () => {
    mountDialog(makeProvider({
      max_transfer_count: undefined,
      max_transfer_timeout_seconds: undefined,
    }))
    await settle()

    const countInput = document.body.querySelector<HTMLInputElement>('#max-transfer-count')
    const timeoutInput = document.body.querySelector<HTMLInputElement>('#max-transfer-timeout-seconds')

    expect(countInput?.value).toBe('')
    expect(countInput?.placeholder).toBe('0 (不限制)')
    expect(timeoutInput?.value).toBe('')
    expect(timeoutInput?.placeholder).toBe('0 (不限制)')

    clickButton('保存')
    await settle()

    expect(endpointMocks.updateProvider).toHaveBeenCalledWith(
      'provider-1',
      expect.objectContaining({
        max_transfer_count: 0,
        max_transfer_timeout_seconds: 0,
      }),
    )
  })

  it('allows configuring transfer limits when creating', async () => {
    mountDialog(null)
    await settle()

    expect(document.body.querySelector<HTMLInputElement>('#max-transfer-count')?.value).toBe('')
    expect(document.body.querySelector<HTMLInputElement>('#max-transfer-timeout-seconds')?.value).toBe('')

    expect(document.body.querySelector<HTMLInputElement>('#stream-failover-budget')?.placeholder).toBe('90')
    await setInput('#stream-failover-budget', '300')
    await setInput('#name', 'New Provider')
    await setInput('#max-transfer-count', '8')
    await setInput('#max-transfer-timeout-seconds', '30')
    clickButton('创建')
    await settle()

    expect(endpointMocks.createProvider).toHaveBeenCalledWith(
      expect.objectContaining({
        failover_rules: { stream_failover_budget_ms: 300000 },
        max_transfer_count: 8,
        max_transfer_timeout_seconds: 30,
      }),
    )
  })
})
