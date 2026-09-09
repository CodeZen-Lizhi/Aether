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

vi.mock('@/api/endpoints/models', () => ({ updateModel: vi.fn() }))
vi.mock('@/composables/useToast', () => ({ useToast: () => feedback }))
vi.mock('@/composables/useConfirm', () => ({ useConfirm: () => feedback }))
vi.mock('../../composables/useUpstreamModelsCache', () => ({
  useUpstreamModelsCache: () => ({ fetchModels: vi.fn() }),
}))

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

function mountDialog() {
  const open = ref(true)
  const saved = vi.fn()
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp({
    render: () => open.value ? h(BatchModelMappingDialog, {
      open: open.value,
      providerId: 'provider-1',
      models,
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

async function addMapping(index: number) {
  const editButton = document.querySelector<HTMLButtonElement>(`[title="继续编辑 Client ${index}"]`)
  expect(editButton).not.toBeNull()
  editButton!.click()
  await nextTick()

  const input = document.querySelector<HTMLInputElement>('#batch-custom-upstream-model')
  expect(input).not.toBeNull()
  input!.value = `new-upstream-${index}`
  input!.dispatchEvent(new Event('input', { bubbles: true }))
  await nextTick()
  buttonWithText('添加并选中').click()
  await nextTick()
}

beforeEach(() => {
  vi.resetAllMocks()
})

afterEach(() => {
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
})

describe('BatchModelMappingDialog saving', () => {
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
    expect(document.querySelector<HTMLInputElement>('#batch-custom-upstream-model')?.disabled).toBe(true)
    expect(document.querySelector<HTMLButtonElement>('[title="清空映射"]')?.disabled).toBe(true)
    expect(updateModel).toHaveBeenCalledWith('provider-1', 'client-1', {
      provider_model_mappings: [
        { name: 'existing-1', priority: 1 },
        { name: 'new-upstream-1', priority: 1 },
      ],
    })

    finishSecondSave!(models[1])
    await vi.waitFor(() => expect(open.value).toBe(false))
    expect(document.body.textContent).not.toContain('批量添加模型映射')
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
    expect(document.body.textContent).toContain('批量添加模型映射')
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
        { name: 'new-upstream-2', priority: 1 },
      ],
    })
    expect(feedback.confirm).not.toHaveBeenCalled()
  })
})
