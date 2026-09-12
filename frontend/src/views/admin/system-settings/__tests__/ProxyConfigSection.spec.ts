import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, reactive, type App, type Component } from 'vue'
import { createPinia } from 'pinia'

import ProxyConfigSection from '../ProxyConfigSection.vue'
import type { ProxyNode } from '@/api/proxy-nodes'

const apiMocks = vi.hoisted(() => ({
  listProxyNodes: vi.fn(),
  testProxyNode: vi.fn(),
}))

const { clearModelsDevCacheMock, successMock, errorMock, dialogStubState } = vi.hoisted(() => ({
  clearModelsDevCacheMock: vi.fn(),
  successMock: vi.fn(),
  errorMock: vi.fn(),
  dialogStubState: {
    deletedPayload: null as Record<string, unknown> | null,
  },
}))

vi.mock('@/api/proxy-nodes', async () => {
  const actual = await vi.importActual<typeof import('@/api/proxy-nodes')>('@/api/proxy-nodes')
  return {
    ...actual,
    proxyNodesApi: {
      ...actual.proxyNodesApi,
      listProxyNodes: apiMocks.listProxyNodes,
      testProxyNode: apiMocks.testProxyNode,
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

vi.mock('@/api/models-dev', () => ({
  clearModelsDevCache: clearModelsDevCacheMock,
}))

vi.mock('@/components/layout', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    CardSection: defineComponent({
      name: 'CardSectionStub',
      props: {
        title: String,
        description: String,
      },
      setup(props, { slots }) {
        return () => h('section', [
          h('h2', props.title),
          h('p', props.description),
          slots.actions?.(),
          slots.default?.(),
        ])
      },
    }),
  }
})

vi.mock('../ProxyNodeEditDialog.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    default: defineComponent({
      name: 'ProxyNodeEditDialogStub',
      props: {
        open: { type: Boolean, default: false },
        node: { type: Object, default: null },
      },
      emits: ['update:open', 'saved', 'deleted'],
      setup(props, { emit }) {
        return () => h('div', {
          'data-testid': 'proxy-node-edit-dialog',
          'data-open': String(props.open),
          'data-node-id': props.node ? String((props.node as { id?: string }).id ?? '') : '',
        }, [
          h('button', { class: 'stub-emit-saved', onClick: () => emit('saved') }),
          h('button', {
            class: 'stub-emit-deleted',
            onClick: () => emit('deleted', dialogStubState.deletedPayload),
          }),
        ])
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

function mountSection(initial: { proxyNodeId?: string | null, hasChanges?: boolean, view?: 'all' | 'selector' | 'management' } = {}) {
  const root = document.createElement('div')
  document.body.appendChild(root)
  const handlers = {
    onSave: vi.fn(),
    onManage: vi.fn(),
    'onUpdate:proxyNodeId': vi.fn(),
  }
  const propsState = reactive({
    proxyNodeId: initial.proxyNodeId ?? null,
    loading: false,
    hasChanges: initial.hasChanges ?? false,
    view: initial.view ?? 'all',
    ...handlers,
  })

  const app = createApp(ProxyConfigSection as unknown as Component, propsState)
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

function findButton(root: HTMLElement, text: string): HTMLButtonElement | undefined {
  return Array.from(root.querySelectorAll('button')).find(
    btn => btn.textContent?.trim() === text
  )
}

function findEditButton(root: HTMLElement, nodeName: string): HTMLButtonElement | undefined {
  return findNodeActionButton(root, nodeName, '编辑')
}

function findNodeActionButton(root: HTMLElement, nodeName: string, text: string): HTMLButtonElement | undefined {
  return Array.from(root.querySelectorAll('button')).find(
    btn => (btn.textContent?.trim() === text || btn.getAttribute('aria-label') === text)
      && btn.parentElement?.parentElement?.textContent?.includes(nodeName)
  )
}

function findDialogStub(root: HTMLElement): HTMLElement | undefined {
  return root.querySelector<HTMLElement>('[data-testid="proxy-node-edit-dialog"]') ?? undefined
}

beforeEach(() => {
  for (const mock of Object.values(apiMocks)) {
    mock.mockReset()
  }
  clearModelsDevCacheMock.mockReset()
  successMock.mockReset()
  errorMock.mockReset()
  dialogStubState.deletedPayload = null
  apiMocks.listProxyNodes.mockResolvedValue({ items: [], total: 0, skip: 0, limit: 1000 })
})

afterEach(() => {
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
  document.body.innerHTML = ''
})

describe('ProxyConfigSection', () => {
  it('keeps the selector separate from node management and its edit dialog', async () => {
    apiMocks.listProxyNodes.mockResolvedValue({ items: [makeNode()], total: 1, skip: 0, limit: 1000 })
    const { root, handlers } = mountSection({ view: 'selector' })
    await flushAsync()
    expect(root.querySelector('#default-proxy')).not.toBeNull()
    expect(root.querySelector('[data-proxy-node]')).toBeNull()
    expect(findDialogStub(root)).toBeUndefined()
    root.querySelector<HTMLButtonElement>('[aria-label="管理代理节点"]')!.click()
    expect(handlers.onManage).toHaveBeenCalledOnce()
  })

  it('mounts one management dialog without a duplicate default selector or save action', async () => {
    apiMocks.listProxyNodes.mockResolvedValue({ items: [makeNode()], total: 1, skip: 0, limit: 1000 })
    const { root } = mountSection({ view: 'management', hasChanges: true })
    await flushAsync()
    expect(root.querySelector('#default-proxy')).toBeNull()
    expect(root.querySelectorAll('[data-proxy-node]')).toHaveLength(1)
    expect(root.querySelectorAll('[data-testid="proxy-node-edit-dialog"]')).toHaveLength(1)
    expect(findButton(root, '保存默认代理')).toBeUndefined()
  })

  it('keeps an unavailable selected proxy explicit when loading its details fails', async () => {
    apiMocks.listProxyNodes.mockRejectedValueOnce(new Error('节点列表请求失败'))
    const { root, handlers } = mountSection({ view: 'selector', proxyNodeId: 'saved-proxy' })
    await flushAsync()
    expect(root.querySelector('#default-proxy')?.textContent).toContain('当前代理信息不可用')
    expect(root.querySelector('[role="alert"]')?.textContent).toContain('节点列表请求失败')
    expect(handlers['onUpdate:proxyNodeId']).not.toHaveBeenCalled()
    expect(root.querySelector('[aria-label="管理代理节点"]')).not.toBeNull()
  })

  it('does not show an empty list while nodes are still loading', async () => {
    let resolveList!: (value: unknown) => void
    apiMocks.listProxyNodes.mockReturnValueOnce(new Promise(resolve => { resolveList = resolve }))
    const { root } = mountSection()
    await nextTick()

    expect(root.querySelector('[role="status"]')?.textContent).toContain('正在加载代理节点')
    expect(root.textContent).not.toContain('暂无代理节点')
    resolveList({ items: [makeNode()], total: 1, skip: 0, limit: 1000 })
    await flushAsync()
    expect(root.textContent).toContain('美西节点')
  })

  it('shows a list failure and can retry instead of reporting no nodes', async () => {
    apiMocks.listProxyNodes.mockRejectedValueOnce(new Error('节点列表请求失败'))
    const { root } = mountSection()
    await flushAsync()

    expect(root.querySelector('[role="alert"]')?.textContent).toContain('节点列表请求失败')
    expect(root.textContent).not.toContain('暂无代理节点')
    apiMocks.listProxyNodes.mockResolvedValueOnce({ items: [makeNode()], total: 1, skip: 0, limit: 1000 })
    findButton(root, '重试')?.click()
    await flushAsync()
    expect(root.textContent).toContain('美西节点')
    expect(root.querySelector('[role="alert"]')).toBeNull()
  })

  it('hides default proxy save and cancel actions until changes exist', async () => {
    const { root } = mountSection({ hasChanges: false })
    await flushAsync()

    const saveButton = findButton(root, '保存默认代理')
    expect(saveButton).toBeUndefined()
    expect(findButton(root, '取消')).toBeUndefined()
  })

  it('enables the save button and drops the hint once there are changes', async () => {
    const { root, handlers } = mountSection({ hasChanges: true })
    await flushAsync()

    const saveButton = findButton(root, '保存默认代理')
    expect(saveButton?.disabled).toBe(false)
    expect(saveButton?.getAttribute('title')).toBeNull()

    saveButton?.click()
    await flushAsync()
    expect(handlers.onSave).toHaveBeenCalledTimes(1)
  })

  it('renders node rows with name, region, status badge, address, and test/edit entries', async () => {
    apiMocks.listProxyNodes.mockResolvedValue({
      items: [
        makeNode(),
        makeNode({
          id: 'node-2',
          name: '东京隧道节点',
          ip: '5.6.7.8',
          port: 443,
          region: null,
          status: 'offline',
          tunnel_mode: true,
        }),
      ],
      total: 2,
      skip: 0,
      limit: 1000,
    })
    const { root } = mountSection()
    await flushAsync()

    expect(root.textContent).toContain('代理节点')
    expect(root.textContent).toContain('(2)')
    expect(root.textContent).toContain('美西节点')
    expect(root.textContent).toContain('US-West')
    expect(root.textContent).toContain('在线')
    expect(root.textContent).toContain('1.2.3.4:8080')
    expect(root.textContent).toContain('东京隧道节点')
    expect(root.textContent).toContain('离线')
    expect(root.textContent).toContain('5.6.7.8')
    expect(findNodeActionButton(root, '美西节点', '测试')).toBeTruthy()
    expect(findNodeActionButton(root, '东京隧道节点', '测试')).toBeTruthy()
    expect(findEditButton(root, '美西节点')).toBeTruthy()
    expect(findEditButton(root, '东京隧道节点')).toBeTruthy()
  })

  it('tests a saved node and reports successful probe details through the toast', async () => {
    apiMocks.listProxyNodes.mockResolvedValue({
      items: [makeNode()],
      total: 1,
      skip: 0,
      limit: 1000,
    })
    apiMocks.testProxyNode.mockResolvedValue({
      success: true,
      latency_ms: 120,
      exit_ip: '1.2.3.4',
      error: null,
      probe_url: 'https://example.com',
      timeout_secs: 10,
    })
    const { root } = mountSection()
    await flushAsync()

    findNodeActionButton(root, '美西节点', '测试')?.click()
    await flushAsync()

    expect(apiMocks.testProxyNode).toHaveBeenCalledWith('node-1')
    expect(successMock).toHaveBeenCalledWith('测试通过：延迟 120ms · 出口 IP 1.2.3.4')
  })

  it('keeps a successful test clear when the probe omits latency and exit IP', async () => {
    apiMocks.listProxyNodes.mockResolvedValue({
      items: [makeNode()],
      total: 1,
      skip: 0,
      limit: 1000,
    })
    apiMocks.testProxyNode.mockResolvedValue({
      success: true,
      latency_ms: null,
      exit_ip: null,
      error: null,
      probe_url: 'https://example.com',
      timeout_secs: 10,
    })
    const { root } = mountSection()
    await flushAsync()

    findNodeActionButton(root, '美西节点', '测试')?.click()
    await flushAsync()

    expect(successMock).toHaveBeenCalledWith('测试通过')
  })

  it('reports a failed saved-node test through the error toast', async () => {
    apiMocks.listProxyNodes.mockResolvedValue({
      items: [makeNode()],
      total: 1,
      skip: 0,
      limit: 1000,
    })
    apiMocks.testProxyNode.mockResolvedValue({
      success: false,
      latency_ms: null,
      exit_ip: null,
      error: 'connection refused',
      probe_url: 'https://example.com',
      timeout_secs: 10,
    })
    const { root } = mountSection()
    await flushAsync()

    findNodeActionButton(root, '美西节点', '测试')?.click()
    await flushAsync()

    expect(errorMock).toHaveBeenCalledWith('测试失败: connection refused')
  })

  it('reports a rejected saved-node test through the error toast', async () => {
    apiMocks.listProxyNodes.mockResolvedValue({
      items: [makeNode()],
      total: 1,
      skip: 0,
      limit: 1000,
    })
    apiMocks.testProxyNode.mockRejectedValue(new Error('network unavailable'))
    const { root } = mountSection()
    await flushAsync()

    findNodeActionButton(root, '美西节点', '测试')?.click()
    await flushAsync()

    expect(errorMock).toHaveBeenCalledWith('network unavailable')
  })

  it('isolates testing state by node and prevents duplicate tests for the same node', async () => {
    let resolveFirst!: (value: unknown) => void
    let resolveSecond!: (value: unknown) => void
    apiMocks.listProxyNodes.mockResolvedValue({
      items: [makeNode(), makeNode({ id: 'node-2', name: '东京节点' })],
      total: 2,
      skip: 0,
      limit: 1000,
    })
    apiMocks.testProxyNode
      .mockReturnValueOnce(new Promise(resolve => { resolveFirst = resolve }))
      .mockReturnValueOnce(new Promise(resolve => { resolveSecond = resolve }))
    const { root } = mountSection()
    await flushAsync()

    const firstTestButton = findNodeActionButton(root, '美西节点', '测试')
    const secondTestButton = findNodeActionButton(root, '东京节点', '测试')
    const secondEditButton = findEditButton(root, '东京节点')
    firstTestButton?.click()
    firstTestButton?.click()
    secondTestButton?.click()
    await nextTick()

    expect(apiMocks.testProxyNode).toHaveBeenCalledTimes(2)
    expect(apiMocks.testProxyNode).toHaveBeenNthCalledWith(1, 'node-1')
    expect(apiMocks.testProxyNode).toHaveBeenNthCalledWith(2, 'node-2')
    expect(firstTestButton?.disabled).toBe(true)
    expect(firstTestButton?.textContent?.trim()).toBe('测试中...')
    expect(secondTestButton?.disabled).toBe(true)
    expect(secondEditButton?.disabled).toBe(false)

    resolveFirst({ success: true, latency_ms: 10, exit_ip: null, error: null, probe_url: '', timeout_secs: 10 })
    resolveSecond({ success: true, latency_ms: null, exit_ip: '5.6.7.8', error: null, probe_url: '', timeout_secs: 10 })
    await flushAsync()

    expect(firstTestButton?.disabled).toBe(false)
    expect(secondTestButton?.disabled).toBe(false)
  })

  it('shows the empty state with an add entry when there are no nodes', async () => {
    const { root } = mountSection()
    await flushAsync()

    expect(root.textContent).toContain('暂无代理节点')
    expect(findButton(root, '添加节点')).toBeTruthy()
    expect(
      Array.from(root.querySelectorAll('button')).filter(
        btn => btn.textContent?.trim() === '添加节点'
      )
    ).toHaveLength(2)
  })

  it('opens the dialog in add mode from the add entries', async () => {
    const { root } = mountSection()
    await flushAsync()

    const addButtons = Array.from(root.querySelectorAll('button')).filter(
      btn => btn.textContent?.trim() === '添加节点'
    )
    addButtons[1]?.click()
    await flushAsync()

    const stub = findDialogStub(root)
    expect(stub?.dataset.open).toBe('true')
    expect(stub?.dataset.nodeId).toBe('')
  })

  it('opens the dialog in edit mode for the chosen node', async () => {
    apiMocks.listProxyNodes.mockResolvedValue({
      items: [makeNode()],
      total: 1,
      skip: 0,
      limit: 1000,
    })
    const { root } = mountSection()
    await flushAsync()

    findEditButton(root, '美西节点')?.click()
    await flushAsync()

    const stub = findDialogStub(root)
    expect(stub?.dataset.open).toBe('true')
    expect(stub?.dataset.nodeId).toBe('node-1')
  })

  it('clears the default proxy when the backend reports the system proxy was cleared', async () => {
    dialogStubState.deletedPayload = {
      nodeId: 'node-1',
      clearedSystemProxy: true,
      clearedExternalModelsProxy: false,
    }
    const { root, handlers } = mountSection({ proxyNodeId: null })
    await flushAsync()

    findDialogStub(root)?.querySelector<HTMLButtonElement>('.stub-emit-deleted')?.click()
    await flushAsync()

    expect(handlers['onUpdate:proxyNodeId']).toHaveBeenCalledWith(null)
    expect(clearModelsDevCacheMock).not.toHaveBeenCalled()
  })

  it('clears the default proxy when the deleted node was the selected default', async () => {
    dialogStubState.deletedPayload = {
      nodeId: 'node-1',
      clearedSystemProxy: false,
      clearedExternalModelsProxy: false,
    }
    const { root, handlers } = mountSection({ proxyNodeId: 'node-1' })
    await flushAsync()

    findDialogStub(root)?.querySelector<HTMLButtonElement>('.stub-emit-deleted')?.click()
    await flushAsync()

    expect(handlers['onUpdate:proxyNodeId']).toHaveBeenCalledWith(null)
    expect(clearModelsDevCacheMock).not.toHaveBeenCalled()
  })

  it('clears the models.dev cache when the backend reports the external models proxy was cleared', async () => {
    dialogStubState.deletedPayload = {
      nodeId: 'node-1',
      clearedSystemProxy: false,
      clearedExternalModelsProxy: true,
    }
    const { root, handlers } = mountSection({ proxyNodeId: 'other-node' })
    await flushAsync()

    findDialogStub(root)?.querySelector<HTMLButtonElement>('.stub-emit-deleted')?.click()
    await flushAsync()

    expect(clearModelsDevCacheMock).toHaveBeenCalledTimes(1)
    expect(handlers['onUpdate:proxyNodeId']).not.toHaveBeenCalled()
  })

  it('keeps the default proxy untouched when the deleted node is unrelated', async () => {
    dialogStubState.deletedPayload = {
      nodeId: 'node-1',
      clearedSystemProxy: false,
      clearedExternalModelsProxy: false,
    }
    const { root, handlers } = mountSection({ proxyNodeId: 'other-node' })
    await flushAsync()

    findDialogStub(root)?.querySelector<HTMLButtonElement>('.stub-emit-deleted')?.click()
    await flushAsync()

    expect(handlers['onUpdate:proxyNodeId']).not.toHaveBeenCalled()
    expect(clearModelsDevCacheMock).not.toHaveBeenCalled()
  })
})
