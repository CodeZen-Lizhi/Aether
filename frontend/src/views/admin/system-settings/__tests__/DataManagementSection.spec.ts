import { afterEach, describe, expect, it, vi } from 'vitest'
import { createApp, h, reactive, nextTick, type App } from 'vue'
import DataManagementSection from '../DataManagementSection.vue'

const mounted: Array<{ app: App; root: HTMLElement }> = []

afterEach(() => {
  for (const { app, root } of mounted.splice(0)) {
    app.unmount()
    root.remove()
  }
})

describe('backup and migration presentation', () => {
  it.each([
    { view: 'backup' as const, kind: 'aggregate', exportLabel: '创建备份', importLabel: '恢复备份' },
    { view: 'migration' as const, kind: 'config', exportLabel: '导出配置', importLabel: '导入配置' },
  ])('routes $view actions to the matching operation and owns only one file input', async ({ view, kind, exportLabel, importLabel }) => {
    const onExport = vi.fn()
    const onFileSelect = vi.fn()
    const props = reactive({
      view,
      configExportLoading: false,
      configImportLoading: false,
      aggregateExportLoading: false,
      aggregateImportLoading: false,
      onExport,
      onFileSelect,
    })
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp({ setup: () => () => h(DataManagementSection, props) })
    app.mount(root)
    mounted.push({ app, root })

    const buttons = Array.from(root.querySelectorAll<HTMLButtonElement>('button'))
    expect(buttons).toHaveLength(2)
    buttons.find(button => button.textContent?.trim() === exportLabel)!.click()
    expect(onExport).toHaveBeenCalledWith(kind)
    const inputs = root.querySelectorAll<HTMLInputElement>('input[type="file"]')
    expect(inputs).toHaveLength(1)
    const input = inputs[0]!
    const click = vi.spyOn(input, 'click').mockImplementation(() => undefined)
    buttons.find(button => button.textContent?.trim() === importLabel)!.click()
    expect(click).toHaveBeenCalledOnce()
    input.dispatchEvent(new Event('change', { bubbles: true }))
    expect(onFileSelect).toHaveBeenCalledWith(kind, expect.any(Event))

    props.configExportLoading = props.configImportLoading = true
    props.aggregateExportLoading = props.aggregateImportLoading = true
    await nextTick()
    expect(buttons.every(button => button.disabled)).toBe(true)
  })
})
