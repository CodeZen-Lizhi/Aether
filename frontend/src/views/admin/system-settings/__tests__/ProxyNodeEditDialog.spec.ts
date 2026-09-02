import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, reactive, type App, type Component } from 'vue'
import { createPinia } from 'pinia'

import ProxyNodeEditDialog from '../ProxyNodeEditDialog.vue'
import type { ProxyNode } from '@/api/proxy-nodes'

const apiMocks = vi.hoisted(() => ({
  listProxyNodes: vi.fn(),
  getNode: vi.fn(),
  createManualNode: vi.fn(),
  updateManualNode: vi.fn(),
  deleteProxyNode: vi.fn(),
  testProxyUrl: vi.fn(),
}))

const { successMock, errorMock, confirmDangerMock } = vi.hoisted(() => ({
  successMock: vi.fn(),
  errorMock: vi.fn(),
  confirmDangerMock: vi.fn(),
}))

vi.mock('@/api/proxy-nodes', async () => {
  const actual = await vi.importActual<typeof import('@/api/proxy-nodes')>('@/api/proxy-nodes')
  return {
    ...actual,
    proxyNodesApi: {
      listProxyNodes: apiMocks.listProxyNodes,
      getNode: apiMocks.getNode,
      createManualNode: apiMocks.createManualNode,
      updateManualNode: apiMocks.updateManualNode,
      deleteProxyNode: apiMocks.deleteProxyNode,
      testProxyUrl: apiMocks.testProxyUrl,
    },
  }
})

vi.mock('@/composables/useToast', () => ({
  useToast: () => ({
    success: successMock,
    error: errorMock,
    warning: vi.fn(),
    info: vi.fn(),
  }),
}))

vi.mock('@/composables/useConfirm', () => ({
  useConfirm: () => ({
    confirmDanger: confirmDangerMock,
  }),
}))

vi.mock('@/components/ui', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    Dialog: defineComponent({
      name: 'DialogStub',
      props: {
        open: { type: Boolean, default: false },
        title: { type: String, default: undefined },
        description: { type: String, default: undefined },
      },
      setup(props, { slots }) {
        return () => props.open
          ? h('div', { class: 'dialog-stub' }, [
              h('h3', { class: 'dialog-stub-title' }, props.title),
              slots.default?.(),
              slots.footer?.(),
            ])
          : null
      },
    }),
  }
})

function makeNode(overrides: Partial<ProxyNode> = {}): ProxyNode {
  return {
    id: 'node-1',
    name: '美西节点',
    ip: '1.2.3.4',
    port: 8080,
    region: 'US-West',
    status: 'online',
    is_manual: true,
    tunnel_mode: false,
    tunnel_connected: false,
    tunnel_connected_at: null,
    hardware_info: null,
    estimated_max_concurrency: null,
    remote_config: null,
    config_version: 1,
    registered_by: null,
    last_heartbeat_at: null,
    heartbeat_interval: 10,
    active_connections: 0,
    total_requests: 0,
    avg_latency_ms: null,
    failed_requests: 0,
    dns_failures: 0,
    stream_errors: 0,
    proxy_metadata: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

const mountedApps: Array<{ app: App, root: HTMLElement }> = []

function mountDialog(initial: { open?: boolean, node?: ProxyNode | null } = {}) {
  const root = document.createElement('div')
  document.body.appendChild(root)
  const handlers = {
    onSaved: vi.fn(),
    onDeleted: vi.fn(),
    'onUpdate:open': vi.fn(),
  }
  const propsState = reactive({
    open: initial.open ?? false,
    node: initial.node ?? null,
    ...handlers,
  })

  const app = createApp(ProxyNodeEditDialog as unknown as Component, propsState)
  app.use(createPinia())
  app.mount(root)
  mountedApps.push({ app, root })
  return { root, handlers }
}

async function flushAsync(rounds = 6) {
  for (let i = 0; i < rounds; i += 1) {
    await Promise.resolve()
    await nextTick()
  }
}

function findInputs(root: HTMLElement): HTMLInputElement[] {
  return Array.from(root.querySelectorAll('input'))
}

function findButton(root: HTMLElement, text: string): HTMLButtonElement | undefined {
  return Array.from(root.querySelectorAll('button')).find(
    btn => btn.textContent?.trim() === text
  )
}

function setInputValue(input: HTMLInputElement, value: string) {
  input.value = value
  input.dispatchEvent(new Event('input', { bubbles: true }))
}

beforeEach(() => {
  for (const mock of Object.values(apiMocks)) {
    mock.mockReset()
  }
  successMock.mockReset()
  errorMock.mockReset()
  confirmDangerMock.mockReset()
  apiMocks.listProxyNodes.mockResolvedValue({ items: [], total: 0, skip: 0, limit: 1000 })
})

afterEach(() => {
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
  document.body.innerHTML = ''
})

describe('ProxyNodeEditDialog', () => {
  it('initializes an empty form in add mode', async () => {
    const { root } = mountDialog({ open: true, node: null })
    await flushAsync()

    expect(root.querySelector('.dialog-stub-title')?.textContent).toBe('添加节点')
    const inputs = findInputs(root)
    expect(inputs).toHaveLength(5)
    for (const input of inputs) {
      expect(input.value).toBe('')
    }
  })

  it('fills the form from the node detail endpoint in edit mode and hints the password placeholder', async () => {
    apiMocks.getNode.mockResolvedValue({
      node: makeNode({
        proxy_url: 'http://1.2.3.4:8080',
        proxy_username: 'user',
        proxy_password: 'secret',
      }),
    })
    const { root } = mountDialog({ open: true, node: makeNode() })
    await flushAsync()

    expect(apiMocks.getNode).toHaveBeenCalledWith('node-1')
    expect(root.querySelector('.dialog-stub-title')?.textContent).toBe('编辑节点')
    const [name, proxyUrl, username, password, region] = findInputs(root)
    expect(name.value).toBe('美西节点')
    expect(proxyUrl.value).toBe('http://1.2.3.4:8080')
    expect(username.value).toBe('user')
    expect(password.value).toBe('secret')
    expect(region.value).toBe('US-West')
    expect(password.placeholder).toBe('留空保持不变')
  })

  it('disables save with an inline error for an invalid proxy prefix and enables it once valid', async () => {
    const { root } = mountDialog({ open: true, node: null })
    await flushAsync()

    const [name, proxyUrl] = findInputs(root)
    const saveButton = findButton(root, '保存')
    expect(saveButton).toBeTruthy()

    setInputValue(name, '测试节点')
    setInputValue(proxyUrl, 'ftp://not-a-proxy')
    await flushAsync()

    expect(root.textContent).toContain('代理地址必须以 http://、https:// 或 socks5:// 开头')
    expect(saveButton?.disabled).toBe(true)

    setInputValue(proxyUrl, 'http://1.2.3.4:8080')
    await flushAsync()

    expect(root.textContent).not.toContain('代理地址必须以 http://、https:// 或 socks5:// 开头')
    expect(saveButton?.disabled).toBe(false)
  })

  it('omits the password field when it is left empty on update', async () => {
    apiMocks.getNode.mockResolvedValue({
      node: makeNode({ proxy_url: 'http://1.2.3.4:8080', proxy_password: 'secret' }),
    })
    const { root, handlers } = mountDialog({ open: true, node: makeNode() })
    await flushAsync()

    const [, , , password] = findInputs(root)
    setInputValue(password, '')
    await flushAsync()

    const saveButton = findButton(root, '保存')
    saveButton?.click()
    await flushAsync()

    expect(apiMocks.updateManualNode).toHaveBeenCalledTimes(1)
    const [nodeId, payload] = apiMocks.updateManualNode.mock.calls[0]
    expect(nodeId).toBe('node-1')
    expect(payload).toMatchObject({
      name: '美西节点',
      proxy_url: 'http://1.2.3.4:8080',
      username: undefined,
      region: 'US-West',
    })
    expect(payload.password).toBeUndefined()
    expect(successMock).toHaveBeenCalledWith('代理节点已更新')
    expect(handlers.onSaved).toHaveBeenCalledTimes(1)
    expect(handlers['onUpdate:open']).toHaveBeenLastCalledWith(false)
  })

  it('renders the successful connectivity test inline', async () => {
    apiMocks.testProxyUrl.mockResolvedValue({
      success: true,
      latency_ms: 120,
      exit_ip: '1.2.3.4',
      error: null,
      probe_url: 'https://example.com',
      timeout_secs: 10,
    })
    const { root } = mountDialog({ open: true, node: null })
    await flushAsync()

    const [, proxyUrl] = findInputs(root)
    setInputValue(proxyUrl, 'http://1.2.3.4:8080')
    await flushAsync()

    findButton(root, '测试连接')?.click()
    await flushAsync()

    expect(apiMocks.testProxyUrl).toHaveBeenCalledWith({
      proxy_url: 'http://1.2.3.4:8080',
      username: undefined,
      password: undefined,
    })
    expect(root.textContent).toContain('测试通过：延迟 120ms · 出口 IP 1.2.3.4')
  })

  it('clears the inline test result when the tested fields change afterwards', async () => {
    apiMocks.testProxyUrl.mockResolvedValue({
      success: true,
      latency_ms: 120,
      exit_ip: '1.2.3.4',
      error: null,
      probe_url: 'https://example.com',
      timeout_secs: 10,
    })
    const { root } = mountDialog({ open: true, node: null })
    await flushAsync()

    const [, proxyUrl] = findInputs(root)
    setInputValue(proxyUrl, 'http://1.2.3.4:8080')
    await flushAsync()

    findButton(root, '测试连接')?.click()
    await flushAsync()
    expect(root.textContent).toContain('测试通过：延迟 120ms · 出口 IP 1.2.3.4')

    setInputValue(proxyUrl, 'http://1.2.3.4:8081')
    await flushAsync()
    expect(root.textContent).not.toContain('测试通过：延迟 120ms · 出口 IP 1.2.3.4')
  })

  it('renders the failed connectivity test inline', async () => {
    apiMocks.testProxyUrl.mockResolvedValue({
      success: false,
      latency_ms: null,
      exit_ip: null,
      error: 'connection refused',
      probe_url: 'https://example.com',
      timeout_secs: 10,
    })
    const { root } = mountDialog({ open: true, node: null })
    await flushAsync()

    const [, proxyUrl] = findInputs(root)
    setInputValue(proxyUrl, 'socks5://1.2.3.4:1080')
    await flushAsync()

    findButton(root, '测试连接')?.click()
    await flushAsync()

    expect(root.textContent).toContain('测试失败: connection refused')
  })

  it('does not call deleteProxyNode when the confirmation is dismissed', async () => {
    apiMocks.getNode.mockResolvedValue({ node: makeNode() })
    confirmDangerMock.mockResolvedValue(false)
    const { root, handlers } = mountDialog({ open: true, node: makeNode() })
    await flushAsync()

    findButton(root, '删除')?.click()
    await flushAsync()

    expect(confirmDangerMock).toHaveBeenCalledTimes(1)
    expect(apiMocks.deleteProxyNode).not.toHaveBeenCalled()
    expect(handlers.onDeleted).not.toHaveBeenCalled()
    expect(handlers['onUpdate:open']).not.toHaveBeenCalledWith(false)
  })

  it('emits the deleted payload and closes after confirmed deletion', async () => {
    apiMocks.getNode.mockResolvedValue({ node: makeNode() })
    confirmDangerMock.mockResolvedValue(true)
    apiMocks.deleteProxyNode.mockResolvedValue({
      message: 'ok',
      node_id: 'node-1',
      cleared_system_proxy: true,
      cleared_external_models_proxy: false,
    })
    const { root, handlers } = mountDialog({ open: true, node: makeNode() })
    await flushAsync()

    findButton(root, '删除')?.click()
    await flushAsync()

    expect(apiMocks.deleteProxyNode).toHaveBeenCalledWith('node-1')
    expect(handlers.onDeleted).toHaveBeenCalledWith({
      nodeId: 'node-1',
      clearedSystemProxy: true,
      clearedExternalModelsProxy: false,
    })
    expect(successMock).toHaveBeenCalledWith('代理节点已删除')
    expect(handlers['onUpdate:open']).toHaveBeenLastCalledWith(false)
  })

  it('creates a node through the store and closes on success', async () => {
    apiMocks.createManualNode.mockResolvedValue({ node_id: 'node-2', node: makeNode({ id: 'node-2' }) })
    const { root, handlers } = mountDialog({ open: true, node: null })
    await flushAsync()

    const [name, proxyUrl] = findInputs(root)
    setInputValue(name, '新节点')
    setInputValue(proxyUrl, 'socks5h://5.6.7.8:1080')
    await flushAsync()

    findButton(root, '保存')?.click()
    await flushAsync()

    expect(apiMocks.createManualNode).toHaveBeenCalledWith({
      name: '新节点',
      proxy_url: 'socks5h://5.6.7.8:1080',
      username: undefined,
      password: undefined,
      region: undefined,
    })
    expect(successMock).toHaveBeenCalledWith('代理节点已添加')
    expect(handlers.onSaved).toHaveBeenCalledTimes(1)
    expect(handlers['onUpdate:open']).toHaveBeenLastCalledWith(false)
  })
})
