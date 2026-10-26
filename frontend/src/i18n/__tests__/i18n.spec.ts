import { beforeEach, describe, expect, it } from 'vitest'
import { createApp, defineComponent, h, nextTick, ref } from 'vue'

import { createI18n, useI18n, useLocaleOptions } from '@/i18n'
import { translateLegacyText } from '@/i18n/messages'
import { transformLegacyTemplateI18n } from '@/i18n/legacy-template-transform'

describe('i18n infrastructure', () => {
  beforeEach(() => {
    localStorage.clear()
    document.documentElement.lang = ''
  })

  it('installs a single app-level translator and updates document language', () => {
    const app = createApp(defineComponent({ setup: () => () => h('div') }))

    app.use(createI18n())

    expect(['zh-CN', 'en-US']).toContain(document.documentElement.lang)
    expect(app.config.globalProperties.$t('site.home.login')).toBeTruthy()
    expect(app.config.globalProperties.$legacyT('保存')).toBeTruthy()
    expect(localStorage.getItem('aether_locale')).toBe(document.documentElement.lang)
  })

  it('switches locale, persists it, and interpolates params', async () => {
    const Probe = defineComponent({
      setup() {
        const { t, setLocale } = useI18n()
        const { currentLocaleLabel } = useLocaleOptions()

        return { t, setLocale, currentLocaleLabel }
      },
      render() {
        return h('div', [
          h('span', { id: 'label' }, this.currentLocaleLabel),
          h('span', { id: 'message' }, this.t('site.privacy.currentVersion', { version: '2' })),
        ])
      },
    })

    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(Probe)
    app.use(createI18n())
    const vm = app.mount(root) as InstanceType<typeof Probe> & {
      setLocale: (locale: 'zh-CN' | 'en-US') => void
    }

    vm.setLocale('en-US')
    await nextTick()

    expect(document.documentElement.lang).toBe('en-US')
    expect(localStorage.getItem('aether_locale')).toBe('en-US')
    expect(root.textContent).toContain('English')
    expect(root.textContent).toContain('Current version: 2')

    app.unmount()
    root.remove()
  })

  it('exposes legacy text translation through the app-level helper', async () => {
    const Probe = defineComponent({
      setup() {
        const { legacyT, setLocale } = useI18n()
        const source = ref('保存')

        return { legacyT, setLocale, source }
      },
      render() {
        return h('button', { title: this.legacyT('关闭') }, this.legacyT(this.source))
      },
    })

    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(Probe)
    app.use(createI18n())
    const vm = app.mount(root) as InstanceType<typeof Probe> & {
      setLocale: (locale: 'zh-CN' | 'en-US') => void
      source: string
    }

    vm.setLocale('en-US')
    await nextTick()

    expect(root.querySelector('button')?.textContent).toBe('Save')
    expect(root.querySelector('button')?.getAttribute('title')).toBe('Close')

    vm.source = '保存中...'
    await nextTick()

    expect(root.querySelector('button')?.textContent).toBe('Saving...')

    app.unmount()
    root.remove()
  })

  it('rewrites template text and static attributes without touching code blocks', () => {
    const result = transformLegacyTemplateI18n(`
      <button title="关闭">取消</button>
      <input placeholder="全部状态">
      <code>复制 配置</code>
    `)

    expect(result.changed).toBe(true)
    expect(result.needsHelper).toBe(true)
    expect(result.code).toContain(`:title='__aetherLegacyT("关闭")'`)
    expect(result.code).toContain(`{{ __aetherLegacyT("取消") }}`)
    expect(result.code).toContain(`:placeholder='__aetherLegacyT("全部状态")'`)
    expect(result.code).toContain('<code>复制 配置</code>')
  })

  it('translates common legacy phrases without adding new message entry points', () => {
    expect(translateLegacyText('请求记录清理策略', 'en-US')).toBe('Request log cleanup policy')
    expect(translateLegacyText('最大转移次数', 'en-US')).toBe('Max transfers')
    expect(translateLegacyText('最大转移超时', 'en-US')).toBe('Max transfer timeout')
    expect(translateLegacyText('0 (不限制)', 'en-US')).toBe('0 (unlimited)')
    expect(translateLegacyText('  发布于 2026-01-01  ', 'en-US')).toBe('  Published at 2026-01-01  ')
    expect(translateLegacyText('git clone https://github.com/CodeZen-Lizhi/Aether.git', 'en-US')).toBe('git clone https://github.com/CodeZen-Lizhi/Aether.git')
    expect(translateLegacyText('已保存 1 个客户端模型，2 个客户端模型的更改仍待保存。', 'en-US')).toBe('1 client models saved; 2 client models still have pending changes.')
    expect(translateLegacyText('2 个客户端模型的更改保存失败，仍待保存。', 'en-US')).toBe('Changes for 2 client models failed to save and remain pending.')
    expect(translateLegacyText('拖动 OpenAI 调整优先级', 'en-US')).toBe('Drag OpenAI to change priority')
  })

  it('translates dashboard scopes and dynamic request and token summaries', () => {
    expect(translateLegacyText('本地限流冷却中，未发送到上游', 'en-US')).toBe(
      'Local rate-limit cooldown; request not sent upstream',
    )
    expect(translateLegacyText('Key 3425 命中限流冷却', 'en-US')).toBe(
      'Key 3425 is in rate-limit cooldown',
    )
    expect(translateLegacyText('2 个 Key 命中限流冷却', 'en-US')).toBe(
      '2 keys are in rate-limit cooldown',
    )
    expect(translateLegacyText('预计 20:42:27 恢复', 'en-US')).toBe(
      'Expected recovery at 20:42:27',
    )
    expect(translateLegacyText('本地冷却时间 7分59秒', 'en-US')).toBe(
      'Local cooldown 7m 59s',
    )
    expect(translateLegacyText('本地冷却时间 1小时2分3秒', 'en-US')).toBe(
      'Local cooldown 1h 2m 3s',
    )
    expect(translateLegacyText('累计 Token', 'en-US')).toBe('Lifetime tokens')
    expect(translateLegacyText('今日平均响应', 'en-US')).toBe('Average response today')
    expect(translateLegacyText('重试累计统计', 'en-US')).toBe('Retry lifetime statistics')
    expect(translateLegacyText('按所选统计周期汇总', 'en-US')).toBe('Totals for the selected period')
    expect(translateLegacyText('成功率 83.3%', 'en-US')).toBe('Success rate 83.3%')
    expect(translateLegacyText('输入 1.2M / 输出 30.0K · 写缓存 0 / 读缓存 5.0M', 'en-US'))
      .toBe('Input 1.2M / Output 30.0K · Cache creation 0 / Cache read 5.0M')
  })

  it('translates every batch model mapping workbench status and action', () => {
    expect(translateLegacyText('已隐藏', 'en-US')).toBe('Hidden')
    expect(translateLegacyText('未映射', 'en-US')).toBe('Unmapped')
    expect(translateLegacyText('已映射 2', 'en-US')).toBe('Mapped 2')
    expect(translateLegacyText('只看已修改', 'en-US')).toBe('Modified only')
    expect(translateLegacyText('显示已隐藏（2）', 'en-US')).toBe('Show hidden (2)')
    expect(translateLegacyText('映射状态筛选', 'en-US')).toBe('Mapping status filters')
    expect(translateLegacyText('搜索客户端或提供商模型…', 'en-US')).toBe('Search client or provider models...')
    expect(translateLegacyText('为每个客户端模型维护提供商模型集合，更改统一保存', 'en-US'))
      .toBe('Maintain provider model sets for each client model and save your changes together')
    expect(translateLegacyText('共 4 个客户端模型 · 3 个已映射 · 1 个未映射', 'en-US'))
      .toBe('4 client models · 3 mapped · 1 unmapped')
    expect(translateLegacyText('3 个已映射 · 1 个未映射 · 5 个提供商模型映射', 'en-US'))
      .toBe('3 mapped · 1 unmapped · 5 provider model mappings')
    expect(translateLegacyText('1 项待保存', 'en-US')).toBe('1 pending change')
    expect(translateLegacyText('2 项待保存', 'en-US')).toBe('2 pending changes')
    expect(translateLegacyText('保存 1 项更改', 'en-US')).toBe('Save 1 change')
    expect(translateLegacyText('恢复为已保存状态', 'en-US')).toBe('Restore saved state')
    expect(translateLegacyText('为 GPT-6 Astra 添加提供商模型', 'en-US'))
      .toBe('Add a provider model for GPT-6 Astra')
    expect(translateLegacyText('GPT-6 Astra 的提供商模型', 'en-US'))
      .toBe('Provider models for GPT-6 Astra')
    expect(translateLegacyText('清空 GPT-6 Astra 的映射', 'en-US'))
      .toBe('Clear mappings for GPT-6 Astra')
    expect(translateLegacyText('隐藏 GPT-6 Astra', 'en-US')).toBe('Hide GPT-6 Astra')
    expect(translateLegacyText('显示 GPT-6 Astra', 'en-US')).toBe('Show GPT-6 Astra')
    expect(translateLegacyText('创建 “custom-model”', 'en-US')).toBe('Create "custom-model"')
  })
})
