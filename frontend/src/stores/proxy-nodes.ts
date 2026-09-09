import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import {
  proxyNodesApi,
  type ManualProxyNodeCreateRequest,
  type ProxyNode,
} from '@/api/proxy-nodes'
import { parseApiError } from '@/utils/errorParser'

export const useProxyNodesStore = defineStore('proxy-nodes', () => {
  const nodes = ref<ProxyNode[]>([])
  const total = ref(0)
  const loading = ref(false)
  const error = ref<string | null>(null)
  /** 标记是否已加载过（避免重复请求） */
  const fetched = ref(false)
  let fetchGeneration = 0

  function invalidate() {
    fetchGeneration += 1
    nodes.value = []
    total.value = 0
    fetched.value = false
    loading.value = false
    error.value = null
  }

  /** 在线节点（可用于代理选择） */
  const onlineNodes = computed(() =>
    nodes.value.filter(n =>
      n.status === 'online'
      && n.remote_config?.scheduling_state !== 'draining'
      && n.remote_config?.scheduling_state !== 'cordoned'
    )
  )

  async function fetchNodes(params?: { status?: string }) {
    const generation = ++fetchGeneration
    loading.value = true
    error.value = null

    try {
      const items: ProxyNode[] = []
      let expectedTotal = 0
      do {
        const data = await proxyNodesApi.listProxyNodes({ ...params, skip: items.length, limit: 1000 })
        if (generation !== fetchGeneration) return
        items.push(...data.items)
        expectedTotal = data.total
        if (data.items.length === 0 && items.length < expectedTotal) {
          throw new Error('代理节点列表不完整，请重试')
        }
      } while (items.length < expectedTotal)
      nodes.value = items
      total.value = expectedTotal
      fetched.value = !params?.status
    } catch (err: unknown) {
      if (generation === fetchGeneration) {
        fetched.value = false
        error.value = parseApiError(err, '获取代理节点列表失败')
      }
    } finally {
      if (generation === fetchGeneration) loading.value = false
    }
  }

  /** 确保节点列表已加载（懒加载，不重复请求） */
  async function ensureLoaded() {
    if (!fetched.value && !loading.value) {
      await fetchNodes()
    }
  }

  async function createManualNode(data: ManualProxyNodeCreateRequest) {
    loading.value = true
    error.value = null

    try {
      const result = await proxyNodesApi.createManualNode(data)
      // 重新获取列表以保持排序一致
      await fetchNodes()
      return result
    } catch (err: unknown) {
      // A duplicate (or a lost create response) may already exist in the database.
      // Reconcile the list while the dialog reports the original mutation error.
      await fetchNodes()
      throw err
    } finally {
      loading.value = false
    }
  }

  async function deleteNode(nodeId: string) {
    loading.value = true
    error.value = null

    try {
      await proxyNodesApi.deleteProxyNode(nodeId)
      nodes.value = nodes.value.filter(n => n.id !== nodeId)
      total.value = Math.max(0, total.value - 1)
    } finally {
      loading.value = false
    }
  }

  return {
    nodes,
    total,
    loading,
    error,
    fetched,
    invalidate,
    onlineNodes,
    fetchNodes,
    ensureLoaded,
    createManualNode,
    deleteNode,
  }
})
