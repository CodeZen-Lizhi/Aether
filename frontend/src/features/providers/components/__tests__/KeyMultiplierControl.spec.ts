import { afterEach, describe, expect, it, vi } from 'vitest'
import { createApp, h, nextTick } from 'vue'
import KeyMultiplierControl from '../KeyMultiplierControl.vue'
import { createI18n, setI18nLocale } from '@/i18n'
import type { EndpointAPIKey } from '@/api/endpoints'

const mocks = vi.hoisted(() => ({ sync: vi.fn(), error: vi.fn(), success: vi.fn() }))
vi.mock('@/api/providerOps', () => ({ syncProviderMultiplier: mocks.sync }))
vi.mock('@/composables/useToast', () => ({ useToast: () => ({ error: mocks.error, success: mocks.success }) }))
vi.mock('@/components/ui', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    Dialog: defineComponent({ props: ['modelValue'], setup: (props, { slots }) => () => props.modelValue ? h('section', [slots.default?.(), slots.footer?.()]) : null }),
    Button: defineComponent({ setup: (_, { slots, attrs }) => () => h('button', attrs, slots.default?.()) }),
  }
})
vi.mock('lucide-vue-next', async () => {
  const { defineComponent, h } = await import('vue')
  const Icon = defineComponent({ setup: () => () => h('span') })
  return { ChevronDown: Icon, WifiSync: Icon }
})
const disposers: (() => void)[] = []
afterEach(() => { disposers.splice(0).forEach(fn => fn()); vi.clearAllMocks() })
/** 挂载真实倍率控件，接口仅返回隔离响应。 */
function mountControl(source: 'manual' | 'upstream' = 'manual', actionOnly = false) {
  setI18nLocale('zh-CN')
  const root = document.createElement('div')
  document.body.append(root)
  const apiKey = { id: 'key', name: '测试', default_rate_multiplier: 0.3, multiplier_sync: { source, status: 'failed', error: '后台错误不能展示' } } as EndpointAPIKey
  const app = createApp({ render: () => h(KeyMultiplierControl, { apiKey, providerId: 'provider', actionOnly }) }).use(createI18n())
  app.mount(root)
  disposers.push(() => { app.unmount(); root.remove() })
  return root
}
/** 按用户看到的文字点击并等待响应状态刷新。 */
async function click(root: HTMLElement, text: string) {
  const button = [...root.querySelectorAll('button')].find(item => item.textContent?.trim() === text)
  expect(button).toBeTruthy()
  button!.click()
  await nextTick(); await Promise.resolve(); await nextTick()
}
describe('A 方案倍率交互', () => {
  it('后台错误保持安静；手动输入与来源通过同一请求保存', async () => {
    mocks.sync.mockResolvedValue({ status: 'success', data: { failed: 0 } })
    const root = mountControl()
    expect(root.textContent).not.toContain('后台错误')
    root.querySelector('button')!.click(); await nextTick()
    const input = root.querySelector('input')!
    input.value = '0.7'; input.dispatchEvent(new Event('input'))
    await click(root, '保存设置')
    expect(mocks.sync).toHaveBeenCalledWith('provider', 'key', 'manual', 0.7)
  })
  it('首次开启失败仍显示原倍率与手动来源，保留设置弹窗', async () => {
    mocks.sync.mockResolvedValue({ status: 'not_configured', message: '请先配置用户认证' })
    const root = mountControl()
    root.querySelector('button')!.click(); await nextTick()
    await click(root, '跟随上游'); await click(root, '保存设置')
    expect(mocks.error).toHaveBeenCalledWith('请先配置用户认证')
    expect(root.querySelector('button')!.textContent).toContain('手动')
    expect(root.querySelector('button')!.textContent).toContain('0.3')
    expect(root.querySelector('section')).not.toBeNull()
  })
  it('单密钥同步展示处理后的失败原因', async () => {
    mocks.sync.mockResolvedValue({ status: 'success', data: { failed: 1, results: [{ error: '该供应商未提供兼容的上游倍率查询接口' }] } })
    const root = mountControl('upstream', true)
    const syncButton = root.querySelector('button[aria-label="立即同步倍率"]') as HTMLButtonElement
    expect(syncButton).toBeTruthy()
    syncButton.click()
    await nextTick(); await Promise.resolve(); await nextTick()
    expect(mocks.error).toHaveBeenCalledWith('该供应商未提供兼容的上游倍率查询接口')
    expect(mocks.sync).toHaveBeenCalledWith('provider', 'key', undefined, undefined)
  })
})
