import { afterEach, describe, expect, it, vi } from 'vitest'
import { createApp, defineComponent, h, inject, nextTick, provide } from 'vue'
import KeyMultiplierControl from '../KeyMultiplierControl.vue'
import { createI18n, setI18nLocale } from '@/i18n'
import type { EndpointAPIKey } from '@/api/endpoints'

const mocks = vi.hoisted(() => ({ sync: vi.fn(), error: vi.fn(), success: vi.fn() }))
vi.mock('@/api/providerOps', () => ({ syncProviderMultiplier: mocks.sync }))
vi.mock('@/composables/useToast', () => ({ useToast: () => ({ error: mocks.error, success: mocks.success }) }))
vi.mock('@/components/ui', async () => {
  const SelectContextKey = Symbol('SelectContext')

  const passthrough = (name: string, tag = 'div') => defineComponent({
    name,
    setup(_, { attrs, slots }) {
      return () => h(tag, attrs, slots.default?.())
    },
  })

  const Button = defineComponent({
    name: 'ButtonStub',
    props: { disabled: Boolean },
    setup(props, { attrs, slots }) {
      return () => h('button', {
        ...attrs,
        type: attrs.type ?? 'button',
        disabled: props.disabled,
      }, slots.default?.())
    },
  })

  const Input = defineComponent({
    name: 'InputStub',
    props: {
      modelValue: {
        type: [String, Number],
        default: '',
      },
      disabled: Boolean,
    },
    emits: ['update:modelValue'],
    setup(props, { attrs, emit }) {
      return () => h('input', {
        ...attrs,
        value: props.modelValue ?? '',
        disabled: props.disabled,
        onInput: (event: Event) => emit('update:modelValue', (event.target as HTMLInputElement).value),
      })
    },
  })

  const Select = defineComponent({
    name: 'SelectStub',
    props: {
      modelValue: {
        type: String,
        default: '',
      },
      disabled: Boolean,
    },
    emits: ['update:modelValue'],
    setup(props, { emit, slots }) {
      provide(SelectContextKey, {
        select: (value: string) => emit('update:modelValue', value),
      })
      return () => h('div', {
        'data-select-value': props.modelValue,
        'data-select-disabled': props.disabled ? 'true' : 'false',
      }, slots.default?.())
    },
  })

  const SelectItem = defineComponent({
    name: 'SelectItemStub',
    props: {
      value: {
        type: String,
        required: true,
      },
    },
    setup(props, { attrs, slots }) {
      const context = inject<{ select: (value: string) => void }>(SelectContextKey)
      return () => h('button', {
        ...attrs,
        type: 'button',
        'data-select-item': props.value,
        onClick: () => context?.select(props.value),
      }, slots.default?.())
    },
  })

  return {
    Button,
    Input,
    Select,
    SelectContent: passthrough('SelectContentStub'),
    SelectItem,
    SelectTrigger: passthrough('SelectTriggerStub', 'button'),
    SelectValue: passthrough('SelectValueStub', 'span'),
  }
})
vi.mock('lucide-vue-next', async () => {
  const Icon = defineComponent({ setup: () => () => h('span') })
  return {
    BadgePercent: Icon,
    Loader2: Icon,
  }
})

const disposers: (() => void)[] = []
afterEach(() => { disposers.splice(0).forEach(fn => fn()); vi.clearAllMocks() })

/** 挂载真实倍率控件，接口仅返回隔离响应。 */
function mountControl(source: 'manual' | 'upstream' = 'manual', actionOnly = false) {
  setI18nLocale('zh-CN')
  const root = document.createElement('div')
  document.body.append(root)
  const apiKey = {
    id: 'key',
    name: '测试',
    default_rate_multiplier: 0.3,
    multiplier_sync: { source, status: 'failed', error: '后台错误不能展示' },
  } as EndpointAPIKey
  const app = createApp({ render: () => h(KeyMultiplierControl, { apiKey, providerId: 'provider', actionOnly }) }).use(createI18n())
  app.mount(root)
  disposers.push(() => { app.unmount(); root.remove() })
  return root
}

async function settle() {
  await nextTick()
  await Promise.resolve()
  await nextTick()
}

function openEditor(root: HTMLElement) {
  const trigger = root.querySelector('button[aria-label^="倍率设置"]') as HTMLButtonElement | null
  expect(trigger).toBeTruthy()
  trigger!.click()
}

/** 聚焦倍率输入框并写入新值，随后触发失焦保存。 */
async function editAndBlur(root: HTMLElement, value: string) {
  const input = root.querySelector('input[type="number"]') as HTMLInputElement
  input.focus()
  input.value = value
  input.dispatchEvent(new Event('input', { bubbles: true }))
  await settle()
  input.blur()
  await settle()
  return input
}

/** 通过来源下拉选择切换倍率来源。 */
async function selectSource(root: HTMLElement, value: string) {
  const trigger = root.querySelector('button[aria-label="倍率来源"]') as HTMLButtonElement
  trigger.click()
  await settle()
  const option = root.querySelector(`[data-select-item="${value}"]`) as HTMLButtonElement
  option.click()
  await settle()
}

describe('列表内倍率交互', () => {
  it('点击倍率后可在列表内提交手动值', async () => {
    mocks.sync.mockResolvedValue({ status: 'success', data: { failed: 0 } })
    const root = mountControl()

    openEditor(root)
    await settle()
    const input = root.querySelector('input[type="number"]') as HTMLInputElement
    input.focus()
    input.value = '0.7'
    input.dispatchEvent(new Event('input', { bubbles: true }))
    await settle()
    expect(mocks.sync).not.toHaveBeenCalled()

    input.blur()
    await settle()

    expect(mocks.sync).toHaveBeenCalledWith('provider', 'key', 'manual', 0.7)
    expect(root.querySelector('button[aria-label^="倍率设置"]')).not.toBeNull()
    expect(root.querySelector('button[aria-label^="倍率设置"]')?.textContent).toContain('0.7')
  })

  it('切换上游立即提交并直接生效', async () => {
    mocks.sync.mockResolvedValue({ status: 'success', data: { failed: 0 } })
    const root = mountControl()

    await selectSource(root, 'upstream')

    expect(mocks.sync).toHaveBeenCalledWith('provider', 'key', 'upstream', undefined)
    expect(root.querySelector('input[type="number"]')).toBeNull()
    expect(root.querySelector('[data-multiplier-source="upstream"]')).not.toBeNull()
    expect(root.querySelector('button[aria-label^="倍率设置"]')).toBeNull()
    expect(root.textContent).toContain('0.3')
  })

  it('跟随上游时倍率不可点击编辑', async () => {
    mocks.sync.mockResolvedValue({ status: 'success', data: { failed: 0 } })
    const root = mountControl('upstream')
    await settle()

    expect(root.querySelector('button[aria-label^="倍率设置"]')).toBeNull()
    expect(root.querySelector('input[type="number"]')).toBeNull()
    expect(root.textContent).toContain('0.3')
  })

  it('手动保存失败时保留倍率草稿并显示行内错误', async () => {
    mocks.sync.mockResolvedValue({ status: 'error', message: '服务暂不可用' })
    const root = mountControl()

    openEditor(root)
    await settle()
    const input = await editAndBlur(root, '0.7')

    const alert = root.querySelector('[role="alert"]') as HTMLElement
    expect(mocks.error).toHaveBeenCalledWith('服务暂不可用')
    expect(input.value).toBe('0.7')
    expect(root.querySelector('input[type="number"]')).not.toBeNull()
    expect(alert.textContent).toContain('服务暂不可用')
    expect(input.getAttribute('aria-describedby')).toBe(alert.id)
  })

  it('从上游切回手动立即按当前倍率提交', async () => {
    mocks.sync.mockResolvedValue({ status: 'success', data: { failed: 0 } })
    const root = mountControl('upstream')

    await selectSource(root, 'manual')

    expect(mocks.sync).toHaveBeenCalledWith('provider', 'key', 'manual', 0.3)
    expect(root.querySelector('[data-multiplier-source="manual"]')).not.toBeNull()
    expect(root.querySelector('button[aria-label^="倍率设置"]')?.textContent).toContain('0.3')
  })

  it('自动倍率不可用时恢复手动状态并显示当前行错误', async () => {
    mocks.sync.mockResolvedValue({ status: 'not_configured', message: '请先配置用户认证' })
    const root = mountControl()

    await selectSource(root, 'upstream')

    expect(mocks.error).toHaveBeenCalledWith('请先配置用户认证')
    expect(root.querySelector('[data-multiplier-source="manual"]')).not.toBeNull()
    const alert = root.querySelector('[role="alert"]') as HTMLElement
    expect(alert.textContent).toContain('请先配置用户认证')
    expect(root.querySelector('button[aria-label="倍率来源"]')?.getAttribute('aria-describedby')).toBe(alert.id)
  })

  it('单密钥同步展示处理后的失败原因', async () => {
    mocks.sync.mockResolvedValue({ status: 'success', data: { failed: 1, results: [{ error: '该供应商未提供兼容的上游倍率查询接口' }] } })
    const root = mountControl('upstream', true)
    const syncButton = root.querySelector('button[aria-label="立即同步倍率"]') as HTMLButtonElement
    expect(syncButton).toBeTruthy()
    syncButton.click()
    await settle()

    expect(mocks.error).toHaveBeenCalledWith('该供应商未提供兼容的上游倍率查询接口')
    expect(mocks.sync).toHaveBeenCalledWith('provider', 'key', undefined, undefined)
  })
})
