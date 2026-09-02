import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, reactive, type App, type Component } from 'vue'
import { createPinia } from 'pinia'

import ProxyConfigSection from '../ProxyConfigSection.vue'
import type { ProxyNode } from '@/api/proxy-nodes'

const apiMocks = vi.hoisted(() => ({
  listProxyNodes: vi.fn(),
}))

const { clearModelsDevCacheMock, dialogStubState } = vi.hoisted(() => ({
  clearModelsDevCacheMock: vi.fn(),
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
    },
  }
})

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

function mountSection(initial: { proxyNodeId?: string | null, hasChanges?: boolean } = {}) {
  const root = document.createElement('div')
  document.body.appendChild(root)
  const handlers = {
    onSave: vi.fn(),
    'onUpdate:proxyNodeId': vi.fn(),
  }
  const propsState = reactive({
    proxyNodeId: initial.proxyNodeId ?? null,
    loading: false,
    hasChanges: initial.hasChanges ?? false,
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
  return Array.from(root.querySelectorAll('button')).find(
    btn =>
      btn.textContent?.trim() === '编辑'
      && btn.parentElement?.textContent?.includes(nodeName)
  )
}

function findDialogStub(root: HTMLElement): HTMLElement | undefined {
  return root.querySelector<HTMLElement>('[data-testid="proxy-node-edit-dialog"]') ?? undefined
}

beforeEach(() => {
  apiMocks.listProxyNodes.mockReset()
  clearModelsDevCacheMock.mockReset()
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
  it('renders the default proxy save button with a disabled hint until changes exist', async () => {
    const { root } = mountSection({ hasChanges: false })
    await flushAsync()

    const saveButton = findButton(root, '保存默认代理')
    expect(saveButton).toBeTruthy()
    expect(saveButton?.disabled).toBe(true)
    expect(saveButton?.getAttribute('title')).toBe('暂无改动')
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

  it('renders node rows with name, region, status badge, address, and an edit entry', async () => {
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
    expect(findEditButton(root, '美西节点')).toBeTruthy()
    expect(findEditButton(root, '东京隧道节点')).toBeTruthy()
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
