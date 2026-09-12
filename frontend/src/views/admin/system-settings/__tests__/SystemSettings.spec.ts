import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, type App } from 'vue'
import { createPinia } from 'pinia'
import { createMemoryHistory, createRouter } from 'vue-router'
import SystemSettings from '../../SystemSettings.vue'

const { configsMock, updateMock, nodesMock, desktopMock, cleanupRunsMock } = vi.hoisted(() => ({
  configsMock: vi.fn(), updateMock: vi.fn(), nodesMock: vi.fn(),
  desktopMock: vi.fn(), cleanupRunsMock: vi.fn(),
}))

vi.mock('@/api/admin', () => ({
  adminApi: {
    getAllSystemConfigs: configsMock,
    updateSystemConfig: updateMock,
    getSystemVersion: vi.fn().mockResolvedValue({ version: 'test-version' }),
    getCleanupRuns: cleanupRunsMock,
  },
}))
vi.mock('@/api/proxy-nodes', () => ({ proxyNodesApi: { listProxyNodes: nodesMock } }))
vi.mock('@/desktop/session', () => ({ hasDesktopSession: () => false }))
vi.mock('@/desktop/useDesktopGateway', () => ({ useDesktopGateway: desktopMock }))
vi.mock('../ProxyNodeEditDialog.vue', () => ({ default: { render: () => null } }))
vi.mock('../ConfigImportDialog.vue', () => ({ default: { render: () => null } }))
vi.mock('../AggregateImportDialog.vue', () => ({ default: { render: () => null } }))
vi.mock('../ManualCleanupConfirmDialog.vue', () => ({ default: { render: () => null } }))

const mounted: Array<{ app: App; root: HTMLElement }> = []

async function flush() {
  await new Promise(resolve => setTimeout(resolve, 0))
  await nextTick()
}

async function mountSettings(path = '/admin/system') {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: '/admin/system', component: SystemSettings }],
  })
  await router.push(path)
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp(SystemSettings).use(createPinia()).use(router)
  app.mount(root)
  mounted.push({ app, root })
  await flush()
  return { root, router }
}

function setInput(root: HTMLElement, id: string, value: string) {
  const input = root.querySelector<HTMLInputElement>(`#${  id}`)!
  input.value = value
  input.dispatchEvent(new Event('input', { bubbles: true }))
}

beforeEach(() => {
  configsMock.mockReset().mockResolvedValue([])
  updateMock.mockReset().mockResolvedValue({})
  nodesMock.mockReset().mockResolvedValue({ items: [], total: 0, skip: 0, limit: 1000 })
  cleanupRunsMock.mockReset().mockResolvedValue({ items: [] })
  desktopMock.mockClear()
})

afterEach(() => {
  for (const { app, root } of mounted.splice(0)) {
    app.unmount()
    root.remove()
  }
})

describe('settings pages', () => {
  it.each(['constructor', 'toString', '__proto__'])('falls back to common settings for the unknown query %s', async tab => {
    const { root } = await mountSettings(`/admin/system?tab=${tab}`)
    expect(root.querySelector('#default-proxy')).not.toBeNull()
    expect(root.textContent).toContain('创建备份')
    expect(root.querySelector('a[href*="tab=advanced"]')).not.toBeNull()
  })

  it('shows the common Web settings and keeps advanced controls off the homepage', async () => {
    const { root } = await mountSettings()
    expect(root.querySelector('h1')?.textContent).toBe('系统设置')
    expect(root.textContent).toContain('出站代理')
    expect(root.textContent).toContain('创建备份')
    expect(root.textContent).toContain('恢复备份')
    expect(root.querySelector('[aria-label="管理代理节点"]')).not.toBeNull()
    expect(root.querySelector('a[href*="tab=advanced"]')).not.toBeNull()
    expect(root.querySelector('#rate-limit')).toBeNull()
    expect(root.querySelector('#gateway-port')).toBeNull()
    expect(root.querySelector('[role="switch"]')).toBeNull()
    expect(root.querySelectorAll('input[type="file"]')).toHaveLength(1)
    expect(root.querySelector('.settings-version')?.textContent).toContain('test-version')
    expect(desktopMock).not.toHaveBeenCalled()
  })

  it('preserves drafts across navigation and browser history, keeping unrelated query values', async () => {
    const { root, router } = await mountSettings('/admin/system?filter=retained')
    expect(root.querySelector('h1')?.textContent).toBe('系统设置')
    expect(desktopMock).not.toHaveBeenCalled()
    root.querySelector<HTMLAnchorElement>('a[href*="tab=advanced"]')!.click()
    await flush()
    expect(router.currentRoute.value.query.filter).toBe('retained')
    const records = root.querySelector<HTMLDetailsElement>('#section-records')!
    records.open = true
    setInput(root, 'compressed-log-retention-days', '60')
    setInput(root, 'rate-limit', '200')
    await nextTick()
    root.querySelector<HTMLAnchorElement>('a[aria-label="返回系统设置"]')!.click()
    await flush()
    root.querySelector<HTMLButtonElement>('[aria-label="管理代理节点"]')!.click()
    await flush()
    expect(root.querySelector('h1')?.textContent).toBe('代理管理')
    router.back()
    await flush()
    router.back()
    await flush()
    expect(root.querySelector('h1')?.textContent).toBe('高级设置')
    expect(root.querySelector<HTMLInputElement>('#compressed-log-retention-days')?.value).toBe('60')
    expect(records.open).toBe(true)
    router.forward()
    await flush()
    expect(root.querySelector<HTMLInputElement>('#rate-limit')?.value).toBe('200')
    expect(updateMock).not.toHaveBeenCalled()
    expect(cleanupRunsMock).not.toHaveBeenCalled()
  })

  it('opens legacy hash targets and removes the hash on explicit category navigation', async () => {
    const { root, router } = await mountSettings('/admin/system?tab=backup#section-cleanup')
    expect(root.querySelector('h1')?.textContent).toBe('高级设置')
    expect(root.querySelector<HTMLDetailsElement>('#section-records')?.open).toBe(true)
    expect(root.querySelector<HTMLElement>('#section-cleanup details')?.hasAttribute('open')).toBe(true)
    expect(nodesMock).not.toHaveBeenCalled()
    root.querySelector<HTMLAnchorElement>('a[aria-label="返回系统设置"]')!.click()
    await flush()
    expect(router.currentRoute.value.hash).toBe('')
    expect(root.querySelector('h1')?.textContent).toBe('系统设置')
    expect(nodesMock).toHaveBeenCalledOnce()
    router.back()
    await flush()
    expect(root.querySelector('h1')?.textContent).toBe('高级设置')
  })

  it('saves common retention fields without hidden batch controls or advanced drafts', async () => {
    const { root, router } = await mountSettings('/admin/system?tab=advanced')
    setInput(root, 'rate-limit', '200')
    await router.push('/admin/system?tab=records')
    await flush()
    setInput(root, 'compressed-log-retention-days', '60')
    setInput(root, 'log-retention-days', '730')
    await nextTick()
    expect(root.querySelector('[id*="batch-size"]')).toBeNull()
    const save = Array.from(root.querySelectorAll<HTMLButtonElement>('#section-cleanup button')).find(button => button.textContent?.trim() === '保存')
    save!.click()
    await flush()
    expect(updateMock.mock.calls.map(call => call[0])).toEqual(['compressed_log_retention_days', 'log_retention_days'])
    expect(root.querySelector<HTMLInputElement>('#rate-limit')?.value).toBe('200')
    expect(root.querySelector('#section-basic .settings-save')).not.toBeNull()
    expect(root.querySelector('#section-cleanup .settings-save')).toBeNull()
  })

  it('reveals a conflicting compression field and keeps invalid input unsaved', async () => {
    const { root } = await mountSettings('/admin/system?tab=records')
    setInput(root, 'compressed-log-retention-days', '3')
    await nextTick()
    Array.from(root.querySelectorAll<HTMLButtonElement>('#section-cleanup button')).find(button => button.textContent?.trim() === '保存')!.click()
    await flush()
    expect(updateMock).not.toHaveBeenCalled()
    expect(root.querySelector('#section-cleanup [role="alert"]')?.textContent).toContain('开始压缩内容')
    expect(root.querySelector('#detail-log-retention-days')).toBe(document.activeElement)
  })

  it('supports every former section link and falls back from an unknown category', async () => {
    const { root, router } = await mountSettings('/admin/system?tab=unknown')
    expect(root.querySelector('h1')?.textContent).toBe('系统设置')
    const links = [
      ['desktop-gateway', '系统设置'], ['proxy', '系统设置'],
      ['request-log', '高级设置'], ['cleanup', '高级设置'],
      ['data-mgmt', '系统设置'], ['basic', '高级设置'], ['sysinfo', '高级设置'],
    ]
    for (const [section, title] of links) {
      await router.push(`/admin/system#section-${section}`)
      await flush()
      expect(root.querySelector('h1')?.textContent).toBe(title)
    }
    expect(desktopMock).not.toHaveBeenCalled()
    expect(cleanupRunsMock).not.toHaveBeenCalled()
  })

  it('opens old query destinations and keeps both basic disclosures in one save group', async () => {
    const { root, router } = await mountSettings('/admin/system?tab=connection')
    expect(root.querySelector('h1')?.textContent).toBe('系统设置')
    await router.push('/admin/system?tab=backup')
    await flush()
    expect(root.querySelector('h1')?.textContent).toBe('系统设置')
    await router.push('/admin/system?tab=records')
    await flush()
    expect(root.querySelector<HTMLDetailsElement>('#section-records')?.open).toBe(true)
    expect(root.querySelectorAll('.settings-advanced-group')).toHaveLength(6)
    expect(root.querySelectorAll('input[type="file"]')).toHaveLength(2)
    setInput(root, 'rate-limit', '200')
    root.querySelector<HTMLButtonElement>('#enable-format-conversion')!.click()
    await nextTick()
    expect(root.querySelectorAll('#section-basic .settings-save')).toHaveLength(1)
    const save = Array.from(root.querySelectorAll<HTMLButtonElement>('#section-basic button'))
      .find(button => button.textContent?.trim() === '保存兼容与密钥设置')!
    save.click()
    await flush()
    expect(updateMock.mock.calls.map(call => call[0]).sort()).toEqual(['enable_format_conversion', 'rate_limit_per_minute'])
    expect(root.querySelector('#section-basic .settings-save')).toBeNull()
  })
})
