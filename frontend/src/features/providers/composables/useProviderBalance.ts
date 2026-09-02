import { onUnmounted, ref } from 'vue'
import type { ProviderWithEndpointsSummary } from '@/api/endpoints'
import { batchQueryBalance, getArchitectures, type ActionResultResponse, type ArchitectureInfo } from '@/api/providerOps'
import { formatBalanceExtraFromSchema, type CredentialsSchema } from '@/features/providers/auth-templates/schema-utils'
import type { BalanceExtraItem } from '@/features/providers/auth-templates'
import { log } from '@/utils/logger'

const MAX_BALANCE_RETRIES = 2
const PENDING_BALANCE_RETRY_BASE_DELAY_MS = 12_000
const PENDING_BALANCE_RETRY_MAX_DELAY_MS = 60_000

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null ? value as Record<string, unknown> : null
}

function isCredentialsSchema(value: unknown): value is CredentialsSchema {
  const schema = asRecord(value)
  return schema?.type === 'object' && asRecord(schema.properties) !== null
}

export function useProviderBalance() {
  const balanceCache = ref<Record<string, ActionResultResponse>>({})
  let balanceLoadVersion = 0
  const pendingTimers = new Set<ReturnType<typeof setTimeout>>()

  const architectureSchemas = ref<Record<string, CredentialsSchema>>({})
  const architectureSchemasLoaded = ref(false)

  const tickCounter = ref(0)
  let tickInterval: ReturnType<typeof setInterval> | null = null

  function startTick() {
    if (tickInterval) return
    tickInterval = setInterval(() => {
      tickCounter.value++
    }, 1000)
  }

  function stopTick() {
    if (!tickInterval) return
    clearInterval(tickInterval)
    tickInterval = null
  }

  async function loadArchitectureSchemas() {
    if (architectureSchemasLoaded.value) return
    try {
      const architectures: ArchitectureInfo[] = await getArchitectures()
      const schemas: Record<string, CredentialsSchema> = {}
      for (const architecture of architectures) {
        if (isCredentialsSchema(architecture.credentials_schema)) {
          schemas[architecture.architecture_id] = architecture.credentials_schema
        }
      }
      architectureSchemas.value = schemas
      architectureSchemasLoaded.value = true
    } catch {
      // 加载架构描述失败不影响余额查询。
    }
  }

  async function loadBalances(providers: ProviderWithEndpointsSummary[], fullReload = true) {
    if (fullReload) {
      balanceCache.value = {}
    }
    const currentVersion = ++balanceLoadVersion
    try {
      const providerIds = providers.filter(provider => provider.ops_configured).map(provider => provider.id)
      if (providerIds.length === 0) return

      const results = await batchQueryBalance(providerIds)
      if (currentVersion !== balanceLoadVersion) return

      const pendingProviderIds: string[] = []
      for (const [providerId, result] of Object.entries(results)) {
        // 保留全部状态：失败态让余额列显示横杠，而不是回落到 monthly_quota 等展示分支。
        balanceCache.value[providerId] = result
        if (result.status === 'pending') {
          pendingProviderIds.push(providerId)
        }
      }

      if (pendingProviderIds.length > 0) {
        const timerId = setTimeout(() => {
          pendingTimers.delete(timerId)
          if (currentVersion === balanceLoadVersion) {
            void retryPendingBalances(pendingProviderIds, currentVersion, 0)
          }
        }, PENDING_BALANCE_RETRY_BASE_DELAY_MS)
        pendingTimers.add(timerId)
      }
    } catch (error) {
      log.warn('[useProviderBalance] 加载余额数据失败', error)
    }
  }

  async function retryPendingBalances(providerIds: string[], loadVersion: number, retryCount: number) {
    try {
      const results = await batchQueryBalance(providerIds)
      if (loadVersion !== balanceLoadVersion) return

      const stillPending: string[] = []

      for (const [providerId, result] of Object.entries(results)) {
        if (result.status !== 'pending') {
          balanceCache.value[providerId] = result
        } else {
          stillPending.push(providerId)
        }
      }

      if (stillPending.length > 0 && retryCount < MAX_BALANCE_RETRIES) {
        const delay = Math.min(
          PENDING_BALANCE_RETRY_BASE_DELAY_MS * Math.pow(2, retryCount),
          PENDING_BALANCE_RETRY_MAX_DELAY_MS,
        )
        const timerId = setTimeout(() => {
          pendingTimers.delete(timerId)
          if (loadVersion === balanceLoadVersion) {
            void retryPendingBalances(stillPending, loadVersion, retryCount + 1)
          }
        }, delay)
        pendingTimers.add(timerId)
      }
    } catch (error) {
      log.warn('[useProviderBalance] 重试加载余额失败', error)
    }
  }

  function isBalanceInfo(data: unknown): data is { total_available: number | null; currency: string } {
    const value = asRecord(data)
    if (!value) return false
    if (value.total_available !== null && typeof value.total_available !== 'number') return false
    return typeof value.currency === 'string'
  }

  function getProviderBalance(providerId: string): { available: number | null; currency: string } | null {
    const result = balanceCache.value[providerId]
    if (!result || (result.status !== 'success' && result.status !== 'auth_expired') || !isBalanceInfo(result.data)) {
      return null
    }
    return {
      available: result.data.total_available,
      currency: result.data.currency || 'USD',
    }
  }

  function getProviderBalanceBreakdown(providerId: string): { balance: number; points: number; currency: string } | null {
    const result = balanceCache.value[providerId]
    if (!result || (result.status !== 'success' && result.status !== 'auth_expired')) return null

    const data = asRecord(result.data)
    const extra = data && asRecord(data.extra)
    if (!data || !extra || typeof extra.balance !== 'number' || typeof extra.points !== 'number') return null

    return {
      balance: extra.balance,
      points: extra.points,
      currency: typeof data.currency === 'string' ? data.currency : 'USD',
    }
  }

  function getProviderBalanceError(providerId: string): { status: string; message: string } | null {
    const result = balanceCache.value[providerId]
    if (!result || result.status === 'pending') return null

    if (result.status === 'auth_failed' || result.status === 'auth_expired') {
      return { status: result.status, message: result.message || '认证失败' }
    }
    if (result.status !== 'success') {
      return { status: result.status, message: result.message || '查询失败' }
    }
    return null
  }

  function isBalanceLoading(providerId: string): boolean {
    return balanceCache.value[providerId]?.status === 'pending'
  }

  function getProviderCheckin(providerId: string): { success: boolean | null; message: string } | null {
    const result = balanceCache.value[providerId]
    if (!result || result.status !== 'success') return null

    const data = asRecord(result.data)
    const extra = data && asRecord(data.extra)
    const success = extra?.checkin_success
    if (success !== true && success !== false && success !== null) return null

    return {
      success,
      message: typeof extra?.checkin_message === 'string' ? extra.checkin_message : '',
    }
  }

  function formatBalanceDisplay(balance: { available: number | null; currency: string } | null): string {
    if (!balance || balance.available === null) return '-'
    const symbol = balance.currency === 'USD' ? '$' : balance.currency
    return `${symbol}${balance.available.toFixed(2)}`
  }

  function formatResetCountdown(resetsAt: number): string {
    void tickCounter.value

    const diff = resetsAt - Date.now() / 1000
    if (diff <= 0) return '即将重置'

    const totalHours = Math.floor(diff / 3600)
    const minutes = Math.floor((diff % 3600) / 60)
    const seconds = Math.floor(diff % 60)
    const pad = (value: number) => value.toString().padStart(2, '0')

    return totalHours > 0
      ? `${totalHours}:${pad(minutes)}:${pad(seconds)}`
      : `${minutes}:${pad(seconds)}`
  }

  function getProviderBalanceExtra(providerId: string, architectureId?: string): BalanceExtraItem[] {
    if (!architectureId) return []

    const result = balanceCache.value[providerId]
    if (!result || (result.status !== 'success' && result.status !== 'auth_expired')) return []

    const data = asRecord(result.data)
    const extra = data && asRecord(data.extra)
    const schema = architectureSchemas.value[architectureId]
    if (!extra || !schema) return []

    return formatBalanceExtraFromSchema(schema, extra)
  }

  function getQuotaUsedColorClass(provider: ProviderWithEndpointsSummary): string {
    const used = provider.monthly_used_usd ?? 0
    const quota = provider.monthly_quota_usd ?? 0
    if (quota <= 0) return 'text-foreground'

    const ratio = used / quota
    if (ratio >= 0.9) return 'text-red-600 dark:text-red-400'
    if (ratio >= 0.7) return 'text-amber-600 dark:text-amber-400'
    return 'text-foreground'
  }

  function cleanup() {
    stopTick()
    pendingTimers.forEach(clearTimeout)
    pendingTimers.clear()
  }

  onUnmounted(cleanup)

  return {
    loadArchitectureSchemas,
    loadBalances,
    getProviderBalance,
    getProviderBalanceBreakdown,
    getProviderBalanceError,
    isBalanceLoading,
    getProviderCheckin,
    formatBalanceDisplay,
    formatResetCountdown,
    getProviderBalanceExtra,
    getQuotaUsedColorClass,
    startTick,
    stopTick,
  }
}
