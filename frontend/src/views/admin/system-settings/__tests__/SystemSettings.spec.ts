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

describe('settings categories', () => {
  it('preserves drafts across navigation and browser history, keeping unrelated query values', async () => {
    const { root, router } = await mountSettings('/admin/system?filter=retained')
    expect(root.querySelector('a[aria-current="page"]')?.textContent).toContain('网络连接')
    expect(desktopMock).not.toHaveBeenCalled()
    const recordsLink = root.querySelector<HTMLAnchorElement>('a[href*="tab=records"]')!
    recordsLink.click()
    await flush()
    expect(router.currentRoute.value.query.filter).toBe('retained')
    setInput(root, 'compressed-log-retention-days', '60')
    await nextTick()
    root.querySelector<HTMLAnchorElement>('a[href*="tab=advanced"]')!.click()
    await flush()
    setInput(root, 'rate-limit', '200')
    await nextTick()
    router.back()
    await flush()
    expect(root.querySelector('a[aria-current="page"]')?.textContent).toContain('记录与存储')
    expect(root.querySelector<HTMLInputElement>('#compressed-log-retention-days')?.value).toBe('60')
    router.forward()
    await flush()
    expect(root.querySelector<HTMLInputElement>('#rate-limit')?.value).toBe('200')
    expect(updateMock).not.toHaveBeenCalled()
    expect(cleanupRunsMock).not.toHaveBeenCalled()
  })

  it('opens legacy hash targets and removes the hash on explicit category navigation', async () => {
    const { root, router } = await mountSettings('/admin/system?tab=backup#section-cleanup')
    expect(root.querySelector('a[aria-current="page"]')?.textContent).toContain('记录与存储')
    expect(root.querySelector<HTMLElement>('#section-cleanup details')?.hasAttribute('open')).toBe(true)
    expect(nodesMock).not.toHaveBeenCalled()
    root.querySelector<HTMLAnchorElement>('a[href*="tab=connection"]')!.click()
    await flush()
    expect(router.currentRoute.value.hash).toBe('')
    expect(root.querySelector('a[aria-current="page"]')?.textContent).toContain('网络连接')
    expect(nodesMock).toHaveBeenCalledOnce()
    router.back()
    await flush()
    expect(root.querySelector('a[aria-current="page"]')?.textContent).toContain('记录与存储')
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
    expect(root.querySelector('a[aria-current="page"]')?.textContent).toContain('网络连接')
    const links = [
      ['desktop-gateway', '网络连接'], ['proxy', '网络连接'],
      ['request-log', '记录与存储'], ['cleanup', '记录与存储'],
      ['data-mgmt', '备份与恢复'], ['basic', '高级与诊断'], ['sysinfo', '高级与诊断'],
    ]
    for (const [section, title] of links) {
      await router.push(`/admin/system#section-${section}`)
      await flush()
      expect(root.querySelector('a[aria-current="page"]')?.textContent).toContain(title)
    }
    expect(desktopMock).not.toHaveBeenCalled()
    expect(cleanupRunsMock).not.toHaveBeenCalled()
  })
})
