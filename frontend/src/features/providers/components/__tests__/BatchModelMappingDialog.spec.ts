import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, h, nextTick, ref, type App } from 'vue'

import type { Model } from '@/api/endpoints'
import { updateModel } from '@/api/endpoints/models'
import BatchModelMappingDialog from '../BatchModelMappingDialog.vue'

const feedback = vi.hoisted(() => ({
  confirm: vi.fn(),
  error: vi.fn(),
  success: vi.fn(),
  warning: vi.fn(),
}))
const providerApi = vi.hoisted(() => ({ queryProviderModels: vi.fn() }))

vi.mock('@/api/endpoints/models', () => ({ updateModel: vi.fn() }))
vi.mock('@/composables/useToast', () => ({ useToast: () => feedback }))
vi.mock('@/composables/useConfirm', () => ({ useConfirm: () => feedback }))
vi.mock('@/api/admin', () => ({ adminApi: providerApi }))

const models: Model[] = [1, 2].map(index => ({
  id: `client-${index}`,
  provider_id: 'provider-1',
  global_model_id: `global-${index}`,
  provider_model_name: `client-model-${index}`,
  global_model_display_name: `Client ${index}`,
  provider_model_mappings: [{ name: `existing-${index}`, priority: 1 }],
  is_active: true,
  is_available: true,
  created_at: '2026-09-09T00:00:00Z',
  updated_at: '2026-09-09T00:00:00Z',
}))

const mountedApps: Array<{ app: App, root: HTMLElement }> = []

function mountDialog(canFetchUpstreamModels = false, initialModelId: string | null = null) {
  const open = ref(true)
  const saved = vi.fn()
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp({
    render: () => open.value ? h(BatchModelMappingDialog, {
      open: open.value,
      providerId: 'provider-1',
      models,
      canFetchUpstreamModels,
      initialModelId,
      'onUpdate:open': (value: boolean) => { open.value = value },
      onSaved: saved,
    }) : null,
  })
  app.mount(root)
  mountedApps.push({ app, root })
  return { open, saved }
}

function buttonWithText(text: string): HTMLButtonElement {
  const button = [...document.querySelectorAll('button')]
    .find(item => item.textContent?.trim() === text)
  expect(button).toBeDefined()
  return button!
}

function visibleModelNames(): string[] {
  return [...document.querySelectorAll<HTMLElement>('tbody tr')]
    .map(row => row.textContent ?? '')
    .filter(text => text.trim().length > 0)
}

async function addMapping(index: number) {
  const addButton = document.querySelector<HTMLButtonElement>(
    `[aria-label="为 Client ${index} 添加提供商模型"]`,
  )
  expect(addButton).not.toBeNull()
  addButton!.click()
  await nextTick()

  const input = document.querySelector<HTMLInputElement>(
    `[data-mapping-add-input][aria-label="Client ${index} 的提供商模型"]`,
  )
  expect(input).not.toBeNull()
  input!.value = `new-upstream-${index}`
  input!.dispatchEvent(new Event('input', { bubbles: true }))
  await nextTick()
  input!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
  await nextTick()
}

beforeEach(() => {
  vi.resetAllMocks()
  for (const [index, model] of models.entries()) {
    model.provider_model_mappings = [{ name: `existing-${index + 1}`, priority: 1 }]
  }
})

afterEach(() => {
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
})

describe('BatchModelMappingDialog saving', () => {
  it('keeps the previous options and draft when a manual refresh fails', async () => {
    providerApi.queryProviderModels
      .mockResolvedValueOnce({
        success: true,
        data: { models: [{ id: 'upstream-before' }] },
      })
      .mockResolvedValueOnce({
        success: false,
        data: { models: [], error: 'upstream refresh failed' },
      })
    mountDialog(true)
    await addMapping(1)

    document.querySelector<HTMLButtonElement>('[title="刷新提供商模型"]')!.click()

    await vi.waitFor(() => expect(document.querySelector('[role="alert"]')?.textContent).toContain('upstream refresh failed'))
    expect(document.body.textContent).toContain('new-upstream-1')
    expect(buttonWithText('保存 1 项更改').disabled).toBe(false)

    document.querySelector<HTMLButtonElement>('[aria-label="为 Client 1 添加提供商模型"]')!.click()
    await nextTick()
    const addInput = document.querySelector<HTMLInputElement>(
      '[data-mapping-add-input][aria-label="Client 1 的提供商模型"]',
    )
    expect(addInput).not.toBeNull()
    addInput!.value = 'upstream-before'
    addInput!.dispatchEvent(new Event('input', { bubbles: true }))
    await nextTick()
    expect(document.body.textContent).toContain('upstream-before')
  })

  it('updates only the selected model mappings without presenting scope text', async () => {
    models[0].provider_model_mappings = [{ name: 'existing-1', priority: 1 }]
    vi.mocked(updateModel).mockResolvedValue(models[0])
    const { open } = mountDialog()
    await addMapping(1)

    expect(document.body.textContent).not.toContain('默认范围')
    expect(document.body.textContent).not.toContain('适用端点')
    expect(document.body.textContent).not.toContain('适用请求')
    buttonWithText('保存 1 项更改').click()

    await vi.waitFor(() => expect(open.value).toBe(false))
    expect(updateModel).toHaveBeenCalledWith('provider-1', 'client-1', {
      provider_model_mappings: [
        { name: 'existing-1', priority: 1 },
        { name: 'new-upstream-1', priority: 2 },
      ],
    })
  })

  it('preserves legacy non-default scope mappings when saving default mappings', async () => {
    models[0].provider_model_mappings = [
      { name: 'existing-1', priority: 1 },
      {
        name: 'endpoint-only',
        priority: 2,
        endpoint_ids: ['endpoint-1'],
      },
    ]
    models[1].provider_model_mappings = []
    vi.mocked(updateModel).mockResolvedValue(models[0])
    mountDialog()
    await addMapping(1)

    buttonWithText('保存 1 项更改').click()

    await vi.waitFor(() => expect(updateModel).toHaveBeenCalledOnce())
    expect(updateModel).toHaveBeenCalledWith('provider-1', 'client-1', {
      provider_model_mappings: [
        {
          name: 'endpoint-only',
          priority: 2,
          endpoint_ids: ['endpoint-1'],
        },
        { name: 'existing-1', priority: 1 },
        { name: 'new-upstream-1', priority: 2 },
      ],
    })
  })

  it('uses the cache initially and bypasses it for a manual refresh', async () => {
    providerApi.queryProviderModels.mockResolvedValue({
      success: true,
      data: { models: [{ id: 'upstream-model' }] },
    })
    mountDialog(true)
    await vi.waitFor(() => expect(providerApi.queryProviderModels).toHaveBeenCalledTimes(1))
    await vi.waitFor(() => {
      expect(document.querySelector<HTMLButtonElement>('[title="刷新提供商模型"]')!.disabled).toBe(false)
    })

    document.querySelector<HTMLButtonElement>('[title="刷新提供商模型"]')!.click()

    await vi.waitFor(() => expect(providerApi.queryProviderModels).toHaveBeenCalledTimes(2))
    expect(providerApi.queryProviderModels).toHaveBeenNthCalledWith(1, 'provider-1', undefined, false)
    expect(providerApi.queryProviderModels).toHaveBeenNthCalledWith(2, 'provider-1', undefined, true)
  })

  it.each([false, true])('keeps every upstream error visible without the removed single-item guidance (partial models: %s)', async (partialModels) => {
    const error = `Key 渠道 A: openai:responses: failed to execute upstream request: error sending request for url (https://upstream.example.test/${'long-path-'.repeat(20)}/models): connection refused; Key 渠道 B: certificate verification failed\n最后一行错误详情`
    providerApi.queryProviderModels.mockResolvedValue({
      success: partialModels,
      data: { models: partialModels ? [{ id: 'upstream-model' }] : [], error },
    })
    mountDialog(true)

    await vi.waitFor(() => expect(document.querySelector(partialModels ? '[role="status"]' : '[role="alert"]')?.textContent).toContain('最后一行错误详情'))
    expect(document.querySelector('pre')?.textContent).toBe(error)
    expect(document.body.textContent).not.toContain('单条编辑')
    expect(document.body.textContent).not.toContain('默认作用于全部端点和请求')
  })

  it('shows each failed client model and its complete save error', async () => {
    const errors = [
      `failed for https://upstream.example.test/${'path-'.repeat(30)}/models: first failure details`,
      'second failure\nretry after updating the model',
    ]
    vi.mocked(updateModel)
      .mockRejectedValueOnce(new Error(errors[0]))
      .mockRejectedValueOnce(new Error(errors[1]))
    const { open } = mountDialog()
    await addMapping(1)
    await addMapping(2)

    buttonWithText('保存 2 项更改').click()
    await vi.waitFor(() => expect(feedback.error).toHaveBeenCalledOnce())

    const details = [...document.querySelectorAll('[role="alert"] pre')].map(item => item.textContent)
    expect(details).toEqual(errors.map((error, index) => `Client ${index + 1}: ${error}`))
    expect(open.value).toBe(true)
    expect(buttonWithText('保存 2 项更改').disabled).toBe(false)
  })

  it('closes automatically after every mapping is saved successfully', async () => {
    let finishSecondSave: ((model: Model) => void) | undefined
    vi.mocked(updateModel)
      .mockResolvedValueOnce(models[0])
      .mockImplementationOnce(() => new Promise<Model>(resolve => { finishSecondSave = resolve }))
    const { open, saved } = mountDialog()
    await addMapping(1)
    await addMapping(2)

    buttonWithText('保存 2 项更改').click()
    await vi.waitFor(() => expect(updateModel).toHaveBeenCalledTimes(2))
    expect(open.value).toBe(true)
    expect(saved).not.toHaveBeenCalled()
    expect(buttonWithText('保存中...').disabled).toBe(true)
    expect(document.querySelector<HTMLButtonElement>('[aria-label="清空 Client 1 的映射"]')?.disabled).toBe(true)
    expect(updateModel).toHaveBeenCalledWith('provider-1', 'client-1', {
      provider_model_mappings: [
        { name: 'existing-1', priority: 1 },
        { name: 'new-upstream-1', priority: 2 },
      ],
    })

    finishSecondSave!(models[1])
    await vi.waitFor(() => expect(open.value).toBe(false))
    expect(document.body.textContent).not.toContain('统一管理客户端模型的提供商映射与优先级')
    expect(saved).toHaveBeenCalledOnce()
    expect(feedback.success).toHaveBeenCalledWith('已成功保存 2 个客户端模型的映射。')
    expect(feedback.confirm).not.toHaveBeenCalled()
  })

  it.each([false, true])('retains failed changes and closes after a successful retry (partial success: %s)', async (partialSuccess) => {
    vi.mocked(updateModel).mockRejectedValue(new Error('保存失败'))
    if (partialSuccess) {
      vi.mocked(updateModel).mockResolvedValueOnce(models[0])
    }
    const { open, saved } = mountDialog()
    await addMapping(1)
    await addMapping(2)

    buttonWithText('保存 2 项更改').click()
    await vi.waitFor(() => expect(feedback.error).toHaveBeenCalledOnce())
    expect(open.value).toBe(true)
    expect(document.body.textContent).toContain('统一管理客户端模型的提供商映射与优先级')
    expect(document.body.textContent).toContain('new-upstream-2')
    expect(document.querySelector('[role="alert"]')?.textContent).toContain('仍待保存')
    expect(saved).toHaveBeenCalledTimes(partialSuccess ? 1 : 0)
    expect(feedback.success).not.toHaveBeenCalled()

    const pendingCount = partialSuccess ? 1 : 2
    vi.mocked(updateModel).mockClear().mockResolvedValue(models[1])
    buttonWithText(`保存 ${pendingCount} 项更改`).click()
    await vi.waitFor(() => expect(open.value).toBe(false))
    expect(updateModel).toHaveBeenCalledTimes(pendingCount)
    if (partialSuccess) {
      expect(updateModel).not.toHaveBeenCalledWith('provider-1', 'client-1', expect.anything())
    }
    expect(updateModel).toHaveBeenCalledWith('provider-1', 'client-2', {
      provider_model_mappings: [
        { name: 'existing-2', priority: 1 },
        { name: 'new-upstream-2', priority: 2 },
      ],
    })
    expect(feedback.confirm).not.toHaveBeenCalled()
  })

  it('reorders mappings and saves contiguous priorities', async () => {
    models[0].provider_model_mappings = [
      { name: 'existing-1', priority: 1 },
      { name: 'secondary-1', priority: 2 },
    ]
    models[1].provider_model_mappings = []
    vi.mocked(updateModel).mockResolvedValue(models[0])
    mountDialog()
    await vi.waitFor(() => expect(document.body.textContent).toContain('secondary-1'))

    expect(document.querySelector('[data-batch-mapping-order-row][draggable="true"]')).not.toBeNull()
    document.querySelector<HTMLButtonElement>('[aria-label="下移 existing-1"]')!.click()
    await nextTick()

    buttonWithText('保存 1 项更改').click()
    await vi.waitFor(() => expect(updateModel).toHaveBeenCalledOnce())
    expect(updateModel).toHaveBeenCalledWith('provider-1', 'client-1', {
      provider_model_mappings: [
        { name: 'secondary-1', priority: 1 },
        { name: 'existing-1', priority: 2 },
      ],
    })
  })

  it('opens focused on the requested client model', async () => {
    mountDialog(false, 'client-2')
    await vi.waitFor(() => expect(visibleModelNames()).toHaveLength(1))

    expect(visibleModelNames()[0]).toContain('Client 2')
    const search = document.querySelector<HTMLInputElement>(
      'input[placeholder="搜索客户端或提供商模型…"]',
    )
    expect(search?.value).toBe('client-2')
  })

  it('filters by model status without changing the draft', async () => {
    models[0].provider_model_mappings = [{ name: 'existing-1', priority: 1 }]
    models[1].provider_model_mappings = null
    mountDialog()
    await vi.waitFor(() => expect(document.body.textContent).toContain('Client 2'))

    expect(buttonWithText('全部 2')).toBeDefined()
    expect(buttonWithText('已映射 1')).toBeDefined()
    expect(buttonWithText('未映射 1')).toBeDefined()
    buttonWithText('已映射 1').click()
    await nextTick()
    expect(visibleModelNames()).toHaveLength(1)
    expect(visibleModelNames()[0]).toContain('Client 1')

    buttonWithText('未映射 1').click()
    await nextTick()
    expect(visibleModelNames()).toHaveLength(1)
    expect(visibleModelNames()[0]).toContain('Client 2')

    buttonWithText('全部 2').click()
    await nextTick()
    const search = document.querySelector<HTMLInputElement>(
      'input[placeholder="搜索客户端或提供商模型…"]',
    )
    expect(search).not.toBeNull()
    search!.value = 'existing-1'
    search!.dispatchEvent(new Event('input', { bubbles: true }))
    await nextTick()
    expect(visibleModelNames()).toHaveLength(1)
    expect(visibleModelNames()[0]).toContain('Client 1')
    expect(buttonWithText('保存').disabled).toBe(true)
  })

  it('hides and restores a model without changing or saving its mappings', async () => {
    models[0].provider_model_mappings = [{ name: 'existing-1', priority: 1 }]
    models[1].provider_model_mappings = [{ name: 'existing-2', priority: 1 }]
    vi.mocked(updateModel).mockResolvedValue(models[0])
    const { open } = mountDialog()
    await vi.waitFor(() => expect(document.body.textContent).toContain('Client 1'))

    const hideButton = document.querySelector<HTMLButtonElement>('[aria-label="隐藏 Client 1"]')
    expect(hideButton).not.toBeNull()
    hideButton!.click()
    await nextTick()

    expect(document.body.textContent).not.toContain('Client 1')
    expect(buttonWithText('保存').disabled).toBe(true)
    expect(document.body.textContent).toContain('显示已隐藏（1）')

    const showHidden = [...document.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')]
      .find(input => input.parentElement?.textContent?.includes('显示已隐藏'))
    expect(showHidden).toBeDefined()
    showHidden!.checked = true
    showHidden!.dispatchEvent(new Event('change', { bubbles: true }))
    await nextTick()

    expect(document.body.textContent).toContain('Client 1')
    const restoreButton = document.querySelector<HTMLButtonElement>('[aria-label="显示 Client 1"]')
    expect(restoreButton).not.toBeNull()
    restoreButton!.click()
    await nextTick()
    expect(document.body.textContent).toContain('existing-1')
    expect(buttonWithText('保存').disabled).toBe(true)
    expect(open.value).toBe(true)
    expect(updateModel).not.toHaveBeenCalled()
  })
})
