import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useProxyNodesStore } from '../proxy-nodes'

const { listProxyNodes, createManualNode } = vi.hoisted(() => ({
  listProxyNodes: vi.fn(), createManualNode: vi.fn(),
}))
vi.mock('@/api/proxy-nodes', () => ({ proxyNodesApi: { listProxyNodes, createManualNode } }))

beforeEach(() => {
  setActivePinia(createPinia())
  listProxyNodes.mockReset()
  createManualNode.mockReset()
})

describe('proxy nodes after backup import', () => {
  it('shows an existing node after an attempted duplicate creation', async () => {
    listProxyNodes.mockResolvedValueOnce({ items: [], total: 0 })
    const store = useProxyNodesStore()
    await store.ensureLoaded()
    const existingNode = { id: 'already-imported' }
    listProxyNodes.mockResolvedValueOnce({ items: [existingNode], total: 1 })
    createManualNode.mockRejectedValueOnce(new Error('代理节点已存在'))

    await expect(store.createManualNode({ name: 'Existing', proxy_url: 'http://proxy.test:8080' }))
      .rejects.toThrow('代理节点已存在')

    expect(store.nodes).toEqual([existingNode])
    expect(store.error).toBeNull()
  })

  it('loads every page so nodes after the first thousand remain visible', async () => {
    const firstPage = Array.from({ length: 1000 }, (_, i) => ({ id: `node-${i}` }))
    const lastNode = { id: 'node-1000' }
    listProxyNodes.mockResolvedValueOnce({ items: firstPage, total: 1001 })
      .mockResolvedValueOnce({ items: [lastNode], total: 1001 })
    const store = useProxyNodesStore()

    await store.fetchNodes()

    expect(store.nodes).toHaveLength(1001)
    expect(store.nodes[1000]).toEqual(lastNode)
    expect(listProxyNodes).toHaveBeenLastCalledWith({ skip: 1000, limit: 1000 })
  })

  it('ignores an older empty response that arrives after imported nodes were loaded', async () => {
    let resolveOld!: (value: unknown) => void
    listProxyNodes.mockReturnValueOnce(new Promise(resolve => { resolveOld = resolve }))
    const store = useProxyNodesStore()
    const oldFetch = store.fetchNodes()
    store.invalidate()
    const importedNode = { id: 'imported-node' }
    listProxyNodes.mockResolvedValueOnce({ items: [importedNode], total: 1 })
    await store.fetchNodes()

    resolveOld({ items: [], total: 0 })
    await oldFetch

    expect(store.nodes).toEqual([importedNode])
    expect(store.total).toBe(1)
    expect(store.fetched).toBe(true)
    expect(store.loading).toBe(false)
  })

  it('reports an incomplete page sequence and permits retry', async () => {
    listProxyNodes.mockResolvedValueOnce({ items: [{ id: 'first' }], total: 2 })
      .mockResolvedValueOnce({ items: [], total: 2 })
    const store = useProxyNodesStore()
    await store.fetchNodes()
    expect(store.error).toBe('代理节点列表不完整，请重试')
    expect(store.fetched).toBe(false)
    expect(store.nodes).toEqual([])

    listProxyNodes.mockResolvedValueOnce({ items: [{ id: 'first' }, { id: 'second' }], total: 2 })
    await store.ensureLoaded()
    expect(store.nodes).toHaveLength(2)
    expect(store.error).toBeNull()
  })
})
