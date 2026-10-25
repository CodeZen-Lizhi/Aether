import { afterEach, describe, expect, it, vi } from 'vitest'
import { createApp, defineComponent, h, nextTick, ref, type App } from 'vue'

import ModelMappingDialog, { type AliasGroup } from '../ModelMappingDialog.vue'
import type { Model } from '@/api/endpoints'
import { updateModel } from '@/api/endpoints/models'

const upstreamModelMocks = vi.hoisted(() => ({
  fetchModels: vi.fn(),
}))

vi.mock('@/components/ui', async () => {
  const { defineComponent, h } = await import('vue')

  const passthrough = (name: string, tag = 'div') => defineComponent({
    name,
    setup(_, { slots }) {
      return () => h(tag, [slots.default?.(), slots.footer?.()])
    },
  })

  return {
    Button: defineComponent({
      name: 'ButtonStub',
      setup(_, { attrs, slots }) {
        return () => h('button', { ...attrs, type: 'button' }, slots.default?.())
      },
    }),
    Dialog: passthrough('DialogStub'),
    Input: defineComponent({
      name: 'InputStub',
      props: { modelValue: String },
      emits: ['update:modelValue'],
      setup(props, { attrs, emit }) {
        return () => h('input', {
          ...attrs,
          value: props.modelValue ?? '',
          onInput: (event: Event) => emit(
            'update:modelValue',
            (event.target as HTMLInputElement).value,
          ),
        })
      },
    }),
    Label: passthrough('LabelStub', 'label'),
    Select: passthrough('SelectStub'),
    SelectContent: passthrough('SelectContentStub'),
    SelectItem: passthrough('SelectItemStub'),
    SelectTrigger: passthrough('SelectTriggerStub'),
    SelectValue: passthrough('SelectValueStub', 'span'),
  }
})

vi.mock('lucide-vue-next', async () => {
  const { defineComponent, h } = await import('vue')
  const Icon = defineComponent({
    name: 'IconStub',
    setup() {
      return () => h('span')
    },
  })
  return {
    Check: Icon,
    ChevronDown: Icon,
    Loader2: Icon,
    Plus: Icon,
    RefreshCw: Icon,
    Search: Icon,
    Tag: Icon,
    Zap: Icon,
  }
})

vi.mock('@/api/endpoints/models', () => ({
  updateModel: vi.fn().mockResolvedValue(undefined),
}))

vi.mock('@/composables/useToast', () => ({
  useToast: () => ({
    error: vi.fn(),
    success: vi.fn(),
    warning: vi.fn(),
  }),
}))

vi.mock('../../composables/useUpstreamModelsCache', () => ({
  useUpstreamModelsCache: () => ({
    fetchModels: upstreamModelMocks.fetchModels,
  }),
}))

const mountedApps: Array<{ app: App, root: HTMLElement }> = []

afterEach(() => {
  vi.mocked(updateModel).mockClear()
  upstreamModelMocks.fetchModels.mockReset()
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
})

describe('ModelMappingDialog', () => {
  it('initializes upstream models when lazily mounted in the open state', async () => {
    upstreamModelMocks.fetchModels.mockResolvedValue({
      models: [],
      error: null,
      warning: null,
    })
    const model = {
      id: 'model-1',
      provider_model_name: 'provider-model-1',
      provider_model_mappings: [],
    } as unknown as Model
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(defineComponent({
      setup() {
        return () => h(ModelMappingDialog, {
          open: true,
          providerId: 'provider-1',
          models: [model],
          hasAutoFetchKey: true,
        })
      },
    }))
    app.mount(root)
    mountedApps.push({ app, root })

    await vi.waitFor(() => expect(upstreamModelMocks.fetchModels).toHaveBeenCalledTimes(1))
    expect(upstreamModelMocks.fetchModels).toHaveBeenCalledWith('provider-1', undefined, false)
  })

  it('bypasses the cache and renders the latest models after a manual refresh', async () => {
    upstreamModelMocks.fetchModels
      .mockResolvedValueOnce({ models: [{ id: 'upstream-before' }] })
      .mockResolvedValueOnce({ models: [{ id: 'upstream-after' }] })
    const model = {
      id: 'model-1',
      provider_model_name: 'provider-model-1',
      provider_model_mappings: [],
    } as unknown as Model
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(defineComponent({
      setup() {
        return () => h(ModelMappingDialog, {
          open: true,
          providerId: 'provider-1',
          models: [model],
          hasAutoFetchKey: true,
        })
      },
    }))
    app.mount(root)
    mountedApps.push({ app, root })

    await vi.waitFor(() => {
      expect(root.querySelector<HTMLButtonElement>('[title="刷新上游模型"]')).not.toBeNull()
    })
    root.querySelector<HTMLButtonElement>('[title="刷新上游模型"]')!.click()

    await vi.waitFor(() => expect(root.textContent).toContain('upstream-after'))
    expect(upstreamModelMocks.fetchModels).toHaveBeenNthCalledWith(1, 'provider-1', undefined, false)
    expect(upstreamModelMocks.fetchModels).toHaveBeenNthCalledWith(2, 'provider-1', undefined, true)
  })

  it('replaces stale upstream models when a successful refresh returns an empty list', async () => {
    upstreamModelMocks.fetchModels
      .mockResolvedValueOnce({ models: [{ id: 'upstream-before' }] })
      .mockResolvedValueOnce({ models: [] })
    const model = {
      id: 'model-1',
      provider_model_name: 'provider-model-1',
      provider_model_mappings: [],
    } as unknown as Model
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(defineComponent({
      setup() {
        return () => h(ModelMappingDialog, {
          open: true,
          providerId: 'provider-1',
          models: [model],
          hasAutoFetchKey: true,
        })
      },
    }))
    app.mount(root)
    mountedApps.push({ app, root })

    await vi.waitFor(() => expect(root.textContent).toContain('upstream-before'))
    root.querySelector<HTMLButtonElement>('[title="刷新上游模型"]')!.click()

    await vi.waitFor(() => expect(root.textContent).not.toContain('upstream-before'))
    expect(root.textContent).toContain('暂无可选模型')
  })

  it('keeps the previous models and draft when a manual refresh fails', async () => {
    upstreamModelMocks.fetchModels
      .mockResolvedValueOnce({ models: [{ id: 'upstream-before' }] })
      .mockResolvedValueOnce({ models: [], error: 'upstream refresh failed' })
    const model = {
      id: 'model-1',
      provider_model_name: 'provider-model-1',
      provider_model_mappings: [{ name: 'selected-before', priority: 1 }],
    } as unknown as Model
    const editingGroup: AliasGroup = {
      model,
      apiFormatsKey: '',
      apiFormats: [],
      endpointIdsKey: '',
      endpointIds: [],
      operationsKey: '',
      operations: [],
      aliases: model.provider_model_mappings ?? [],
    }
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(defineComponent({
      setup() {
        return () => h(ModelMappingDialog, {
          open: true,
          providerId: 'provider-1',
          models: [model],
          editingGroup,
          hasAutoFetchKey: true,
        })
      },
    }))
    app.mount(root)
    mountedApps.push({ app, root })

    await vi.waitFor(() => expect(root.textContent).toContain('upstream-before'))
    root.querySelector<HTMLButtonElement>('[title="刷新上游模型"]')!.click()

    await vi.waitFor(() => expect(upstreamModelMocks.fetchModels).toHaveBeenCalledTimes(2))
    await vi.waitFor(() => {
      expect(root.querySelector<HTMLButtonElement>('[title="刷新上游模型"]')!.disabled).toBe(false)
    })
    expect(root.textContent).toContain('upstream-before')
    expect(root.textContent).toContain('selected-before')
    expect(root.textContent).toContain('已选 1 个')
  })

  it('hides all endpoint and request scope controls', async () => {
    const model = {
      id: 'model-sol',
      provider_model_name: 'gpt-5.6-sol',
      global_model_display_name: 'GPT-5.6 Sol',
      provider_model_mappings: [],
    } as unknown as Model
    const open = ref(false)
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(defineComponent({
      setup() {
        return () => h(ModelMappingDialog, {
          open: open.value,
          providerId: 'provider-1',
          models: [model],
          preselectedModelId: model.id,
          'onUpdate:open': (value: boolean) => { open.value = value },
        })
      },
    }))
    app.mount(root)
    mountedApps.push({ app, root })

    open.value = true
    await nextTick()
    await nextTick()

    expect(root.textContent).not.toContain('适用端点')
    expect(root.textContent).not.toContain('适用请求')
    expect(root.textContent).not.toContain('全部端点')
    expect(root.textContent).not.toContain('全部请求')
    expect(root.textContent).not.toContain('仅会话压缩')
  })

  it('saves an edited mapping with only name and priority', async () => {
    const model = {
      id: 'model-sol',
      provider_model_name: 'gpt-5.6-sol',
      global_model_display_name: 'GPT-5.6 Sol',
      provider_model_mappings: [{
        name: 'gpt-5.6-luna',
        priority: 1,
        endpoint_ids: ['endpoint-responses'],
        operations: ['compact'],
      }],
    } as unknown as Model
    const editingGroup: AliasGroup = {
      model,
      apiFormatsKey: '',
      apiFormats: [],
      endpointIdsKey: 'endpoint-responses',
      endpointIds: ['endpoint-responses'],
      operationsKey: 'compact',
      operations: ['compact'],
      aliases: model.provider_model_mappings ?? [],
    }
    const open = ref(false)
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(defineComponent({
      setup() {
        return () => h(ModelMappingDialog, {
          open: open.value,
          providerId: 'provider-1',
          models: [model],
          editingGroup,
          'onUpdate:open': (value: boolean) => { open.value = value },
        })
      },
    }))
    app.mount(root)
    mountedApps.push({ app, root })

    open.value = true
    await nextTick()
    await nextTick()
    expect(root.textContent).not.toContain('仅会话压缩')

    const saveButton = [...root.querySelectorAll('button')]
      .find(button => button.textContent?.includes('保存映射'))
    saveButton?.click()
    await vi.waitFor(() => expect(updateModel).toHaveBeenCalledTimes(1))

    expect(updateModel).toHaveBeenCalledWith('provider-1', 'model-sol', {
      provider_model_mappings: [{
        name: 'gpt-5.6-luna',
        priority: 1,
      }],
    })
  })
})
