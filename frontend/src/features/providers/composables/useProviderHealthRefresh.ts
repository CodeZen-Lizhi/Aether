import { onMounted, onUnmounted, watch, type Ref } from 'vue'
import { getProvidersSummary, type ProviderSummaryQuery } from '@/api/endpoints/providers'
import type { ProviderWithEndpointsSummary } from '@/api/endpoints/types'

/** 健康数据轮询间隔，单位毫秒。 */
const HEALTH_REFRESH_INTERVAL_MS = 5000

/** 在页面可见时刷新当前页的健康展示；不改列表、配置、余额或编辑状态。 */
export function useProviderHealthRefresh(
  providers: Ref<ProviderWithEndpointsSummary[]>,
  query: Readonly<Ref<ProviderSummaryQuery>>,
  loading: Ref<boolean>,
) {
  let timer: ReturnType<typeof setInterval> | undefined
  let disposed = false
  let inFlight = false
  let revision = 0

  // 查询、整表加载或列表实体变化后，正在返回的旧快照不得覆盖新状态。
  watch([query, loading, providers], () => { revision += 1 }, { flush: 'sync' })

  /** 串行读取无缓存摘要，仅合并现有端点的健康展示字段。 */
  async function refreshHealth() {
    if (disposed || document.hidden || loading.value || inFlight || !providers.value.length) return
    inFlight = true
    const requestRevision = revision
    const snapshots = providers.value.map(provider => ({
      provider,
      endpoints: provider.endpoint_health_details,
    }))
    try {
      const response = await getProvidersSummary({ ...query.value }, { cacheTtlMs: 0, timeout: 10000 })
      if (disposed || document.hidden || requestRevision !== revision) return
      const latest = new Map(response.items.map(provider => [provider.id, provider]))
      for (const { provider, endpoints } of snapshots) {
        // 详情保存可能已替换健康数据，此时保留更新后的本地快照。
        if (provider.endpoint_health_details !== endpoints) continue
        const health = latest.get(provider.id)?.endpoint_health_details
        for (const endpoint of endpoints) {
          const updated = health?.find(item => item.api_format === endpoint.api_format)
          if (updated) Object.assign(endpoint, updated)
        }
      }
    } catch {
      // 健康轮询失败时保留上一次显示，下一轮继续尝试，不打扰当前操作。
    } finally {
      inFlight = false
    }
  }

  /** 隐藏时停止定时器，重新可见时立即补取一次健康状态。 */
  function handleVisibility() {
    revision += 1
    clearInterval(timer)
    if (!document.hidden && !disposed) {
      void refreshHealth()
      timer = setInterval(() => { void refreshHealth() }, HEALTH_REFRESH_INTERVAL_MS)
    }
  }

  onMounted(() => {
    document.addEventListener('visibilitychange', handleVisibility)
    handleVisibility()
  })
  onUnmounted(() => {
    disposed = true
    clearInterval(timer)
    document.removeEventListener('visibilitychange', handleVisibility)
  })
}
