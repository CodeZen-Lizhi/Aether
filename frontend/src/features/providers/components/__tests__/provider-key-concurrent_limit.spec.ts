import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, type App, type Component } from 'vue'
import KeyFormDialog from '@/features/providers/components/KeyFormDialog.vue'
import type { EndpointAPIKey } from '@/api/endpoints'

const endpointMocks = vi.hoisted(() => ({
  addProviderKey: vi.fn(),
  updateProviderKey: vi.fn(),
  getAllCapabilities: vi.fn(),
  sortApiFormats: vi.fn((formats: string[]) => [...formats].sort()),
}))

vi.mock('@/api/endpoints', () => ({
  addProviderKey: endpointMocks.addProviderKey,
  updateProviderKey: endpointMocks.updateProviderKey,
  getAllCapabilities: endpointMocks.getAllCapabilities,
  sortApiFormats: endpointMocks.sortApiFormats,
}))

vi.mock('@/components/ui', async () => {
  const { cloneVNode, computed, defineComponent, h, inject, provide } = await import('vue')
  const SelectContextKey = Symbol('SelectContext')
  const CollapsibleContextKey = Symbol('CollapsibleContext')

  const passthrough = (name: string, tag = 'div') => defineComponent({
    name,
    setup(_, { slots }) {
      return () => h(tag, slots.default?.())
    },
  })

  const Dialog = defineComponent({
    name: 'DialogStub',
    props: {
      modelValue: Boolean,
    },
    setup(props, { slots }) {
      return () => props.modelValue
        ? h('section', [slots.default?.(), slots.footer?.()])
        : null
    },
  })

  const Input = defineComponent({
    name: 'InputStub',
    inheritAttrs: false,
    props: {
      modelValue: {
        type: [String, Number],
        default: '',
      },
      masked: Boolean,
    },
    emits: ['update:modelValue'],
    setup(props, { attrs, emit }) {
      return () => h('input', {
        ...attrs,
        value: props.modelValue ?? '',
        onInput: (event: Event) => emit('update:modelValue', (event.target as HTMLInputElement).value),
      })
    },
  })

  const Label = defineComponent({
    name: 'LabelStub',
    inheritAttrs: false,
    props: {
      for: String,
    },
    setup(props, { attrs, slots }) {
      return () => h('label', { ...attrs, for: props.for }, slots.default?.())
    },
  })

  const Button = defineComponent({
    name: 'ButtonStub',
    inheritAttrs: false,
    props: {
      disabled: Boolean,
      variant: String,
    },
    setup(props, { attrs, slots }) {
      return () => h('button', {
        ...attrs,
        disabled: props.disabled,
        type: attrs.type ?? 'button',
      }, slots.default?.())
    },
  })

  const Switch = defineComponent({
    name: 'SwitchStub',
    inheritAttrs: false,
    props: {
      modelValue: Boolean,
    },
    emits: ['update:modelValue'],
    setup(props, { attrs, emit }) {
      return () => h('input', {
        ...attrs,
        type: 'checkbox',
        checked: props.modelValue,
        onChange: (event: Event) => emit('update:modelValue', (event.target as HTMLInputElement).checked),
      })
    },
  })

  const Collapsible = defineComponent({
    name: 'CollapsibleStub',
    props: {
      open: Boolean,
    },
    emits: ['update:open'],
    setup(props, { emit, slots }) {
      provide(CollapsibleContextKey, {
        open: computed(() => props.open),
        toggle: () => emit('update:open', !props.open),
      })
      return () => h('div', {
        'data-collapsible': 'true',
        'data-state': props.open ? 'open' : 'closed',
      }, slots.default?.())
    },
  })

  const CollapsibleTrigger = defineComponent({
    name: 'CollapsibleTriggerStub',
    setup(_, { slots }) {
      const context = inject<{
        open: { value: boolean }
        toggle: () => void
      } | null>(CollapsibleContextKey, null)

      return () => {
        const child = slots.default?.()[0]
        if (!child) return null
        return cloneVNode(child, {
          'data-collapsible-trigger': 'true',
          'aria-expanded': context?.open.value ? 'true' : 'false',
          onClick: context?.toggle,
        })
      }
    },
  })

  const CollapsibleContent = defineComponent({
    name: 'CollapsibleContentStub',
    setup(_, { slots }) {
      const context = inject<{
        open: { value: boolean }
      } | null>(CollapsibleContextKey, null)
      return () => context?.open.value
        ? h('div', { 'data-collapsible-content': 'true' }, slots.default?.())
        : null
    },
  })

  const Select = defineComponent({
    name: 'SelectStub',
    props: {
      modelValue: String,
    },
    emits: ['update:modelValue'],
    setup(props, { emit, slots }) {
      provide(SelectContextKey, {
        select: (value: string) => emit('update:modelValue', value),
        modelValue: props.modelValue,
      })

      return () => h('div', {
        'data-select': 'true',
        'data-value': props.modelValue,
      }, slots.default?.())
    },
  })

  const SelectItem = defineComponent({
    name: 'SelectItemStub',
    inheritAttrs: false,
    props: {
      value: {
        type: String,
        required: true,
      },
    },
    setup(props, { attrs, slots }) {
      const context = inject<{ select: (value: string) => void } | null>(SelectContextKey, null)
      return () => h('button', {
        ...attrs,
        type: 'button',
        'data-select-item': props.value,
        onClick: () => context?.select(props.value),
      }, slots.default?.())
    },
  })

  return {
    Dialog,
    Button,
    Collapsible,
    CollapsibleContent,
    CollapsibleTrigger,
    Input,
    Label,
    Switch,
    Select,
    SelectTrigger: passthrough('SelectTriggerStub'),
    SelectValue: passthrough('SelectValueStub', 'span'),
    SelectContent: passthrough('SelectContentStub'),
    SelectItem,
  }
})

vi.mock('@/components/common/JsonImportInput.vue', async () => {
  const { defineComponent, h } = await import('vue')

  return {
    default: defineComponent({
      name: 'JsonImportInputStub',
      props: {
        modelValue: {
          type: String,
          default: '',
        },
      },
      emits: ['update:modelValue'],
      setup(props, { emit }) {
        return () => h('textarea', {
          value: props.modelValue,
          onInput: (event: Event) => emit('update:modelValue', (event.target as HTMLTextAreaElement).value),
        })
      },
    }),
  }
})

vi.mock('@/composables/useToast', () => ({
  useToast: () => ({
    success: vi.fn(),
    error: vi.fn(),
  }),
}))

vi.mock('@/composables/useConfirm', () => ({
  useConfirm: () => ({
    confirmWarning: vi.fn().mockResolvedValue(true),
  }),
}))

vi.mock('lucide-vue-next', async () => {
  const { defineComponent, h } = await import('vue')
  const Icon = defineComponent({
    name: 'IconStub',
    setup() {
      return () => h('span')
    },
  })

  return {
    ChevronDown: Icon,
    CircleHelp: Icon,
    Key: Icon,
    SquarePen: Icon,
  }
})

const mountedApps: Array<{ app: App, root: HTMLElement }> = []

function createProviderKey(overrides: Partial<EndpointAPIKey> = {}): EndpointAPIKey {
  return {
    id: 'provider-key-1',
    provider_id: 'provider-1',
    api_formats: ['openai:chat'],
    api_key_masked: 'sk-***',
    auth_type: 'api_key',
    name: 'Primary key',
    rate_multipliers: null,
    rpm_limit: 30,
    concurrent_limit: null,
    allowed_models: null,
    capabilities: null,
    cache_ttl_minutes: 5,
    max_probe_interval_minutes: 32,
    health_score: 100,
    consecutive_failures: 0,
    request_count: 0,
    success_count: 0,
    error_count: 0,
    success_rate: 1,
    avg_response_time_ms: 0,
    is_active: true,
    note: '',
    created_at: '2026-04-27T00:00:00Z',
    updated_at: '2026-04-27T00:00:00Z',
    auto_fetch_models: false,
    model_include_patterns: [],
    model_exclude_patterns: [],
    ...overrides,
  }
}

function mountDialog(component: Component, props: Record<string, unknown>) {
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp(component, props)
  app.mount(root)
  mountedApps.push({ app, root })
  return root
}

async function settle() {
  await nextTick()
  await Promise.resolve()
  await nextTick()
}

function findInput(root: HTMLElement, id: string) {
  const input = root.querySelector<HTMLInputElement>(`#${id}`)
  expect(input).not.toBeNull()
  return input as HTMLInputElement
}

function findAdvancedSettingsTrigger(root: HTMLElement) {
  const trigger = root.querySelector<HTMLButtonElement>('[data-collapsible-trigger="true"]')
  expect(trigger).not.toBeNull()
  return trigger as HTMLButtonElement
}

function updateInput(input: HTMLInputElement, value: string) {
  input.value = value
  input.dispatchEvent(new Event('input', { bubbles: true }))
}

async function submit(root: HTMLElement) {
  const form = root.querySelector('form')
  expect(form).not.toBeNull()
  form?.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))
  await settle()
}

function lastUpdatePayload() {
  const calls = endpointMocks.updateProviderKey.mock.calls
  expect(calls.length).toBeGreaterThan(0)
  return calls[calls.length - 1][1] as Record<string, unknown>
}

beforeEach(() => {
  endpointMocks.addProviderKey.mockReset()
  endpointMocks.updateProviderKey.mockReset()
  endpointMocks.getAllCapabilities.mockReset()
  endpointMocks.sortApiFormats.mockClear()

  endpointMocks.addProviderKey.mockResolvedValue(createProviderKey())
  endpointMocks.updateProviderKey.mockResolvedValue(createProviderKey())
  endpointMocks.getAllCapabilities.mockResolvedValue([])
})

afterEach(() => {
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
})

describe('provider key concurrent_limit form behavior', () => {
  it('keeps default advanced settings collapsed when creating a key and lets the user expand them', async () => {
    const root = mountDialog(KeyFormDialog, {
      open: true,
      endpoint: null,
      editingKey: null,
      providerId: 'provider-1',
      providerType: 'openai',
      availableApiFormats: ['openai:chat'],
    })
    await settle()

    const trigger = findAdvancedSettingsTrigger(root)
    expect(trigger.getAttribute('aria-expanded')).toBe('false')
    expect(root.querySelector('#rpm_limit')).toBeNull()

    trigger.click()
    await settle()

    expect(trigger.getAttribute('aria-expanded')).toBe('true')
    expect(root.querySelector('#rpm_limit')).not.toBeNull()
  })

  it('keeps default advanced settings collapsed when editing a key', async () => {
    const root = mountDialog(KeyFormDialog, {
      open: true,
      endpoint: null,
      editingKey: createProviderKey({ rpm_limit: null, concurrent_limit: null }),
      providerId: 'provider-1',
      providerType: 'openai',
      availableApiFormats: ['openai:chat'],
    })
    await settle()

    expect(findAdvancedSettingsTrigger(root).getAttribute('aria-expanded')).toBe('false')
    expect(root.querySelector('#concurrent_limit')).toBeNull()
  })

  it('expands advanced settings when an edited key has a custom stability value', async () => {
    const root = mountDialog(KeyFormDialog, {
      open: true,
      endpoint: null,
      editingKey: createProviderKey({
        rpm_limit: null,
        concurrent_limit: null,
        cache_ttl_minutes: 0,
      }),
      providerId: 'provider-1',
      providerType: 'openai',
      availableApiFormats: ['openai:chat'],
    })
    await settle()

    expect(findAdvancedSettingsTrigger(root).getAttribute('aria-expanded')).toBe('true')
    expect(findInput(root, 'cache_ttl_minutes').value).toBe('0')
  })

  it('hydrates and serializes a positive concurrent_limit number from the normal key form', async () => {
    const saved = vi.fn()
    const updatedKey = createProviderKey({ rpm_limit: 42, concurrent_limit: 5 })
    endpointMocks.updateProviderKey.mockResolvedValue(updatedKey)
    const root = mountDialog(KeyFormDialog, {
      open: true,
      endpoint: null,
      editingKey: createProviderKey({ rpm_limit: 42, concurrent_limit: 3 }),
      providerId: 'provider-1',
      providerType: 'openai',
      availableApiFormats: ['openai:chat'],
      onSaved: saved,
    })
    await settle()

    const concurrentLimitInput = findInput(root, 'concurrent_limit')
    expect(concurrentLimitInput.value).toBe('3')
    expect(findInput(root, 'rpm_limit').value).toBe('42')

    updateInput(concurrentLimitInput, '5')
    await submit(root)

    const payload = lastUpdatePayload()
    expect(payload.concurrent_limit).toBe(5)
    expect(typeof payload.concurrent_limit).toBe('number')
    expect(payload.concurrent_limit).not.toBe('')
    expect(payload.rpm_limit).toBe(42)
    expect(saved).toHaveBeenCalledWith(updatedKey)
  })

  it('serializes cleared normal key concurrent_limit as null instead of an empty string', async () => {
    const root = mountDialog(KeyFormDialog, {
      open: true,
      endpoint: null,
      editingKey: createProviderKey({ rpm_limit: 24, concurrent_limit: 6 }),
      providerId: 'provider-1',
      providerType: 'openai',
      availableApiFormats: ['openai:chat'],
    })
    await settle()

    updateInput(findInput(root, 'concurrent_limit'), '')
    await submit(root)

    const payload = lastUpdatePayload()
    expect(payload).toHaveProperty('concurrent_limit', null)
    expect(payload.concurrent_limit).not.toBe('')
    expect(payload.rpm_limit).toBe(24)
  })
})
