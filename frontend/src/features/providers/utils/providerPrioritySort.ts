import type { ProviderWithEndpointsSummary } from '@/api/endpoints/types'

/**
 * 供应商列表排序：启用状态 → 调度优先级 → 创建时间。
 *
 * providerPriorities 来自系统默认路由分组的调度策略
 * （parseSchedulingStrategy().providerPriorities，1 起越小越靠前）；
 * 未配置优先级的供应商用 MAX_SAFE_INTEGER 缀尾，与调度策略页口径一致。
 * priorities 为空表时退化为原有的「启用状态 → 创建时间」排序。
 */
export const UNCONFIGURED_PROVIDER_PRIORITY = Number.MAX_SAFE_INTEGER

export function sortProvidersByActiveAndPriority(
  items: ProviderWithEndpointsSummary[],
  providerPriorities: Record<string, number> = {},
): ProviderWithEndpointsSummary[] {
  return [...items].sort((left, right) => {
    if (left.is_active !== right.is_active) {
      return left.is_active ? -1 : 1
    }
    const leftPriority = providerPriorities[left.id] ?? UNCONFIGURED_PROVIDER_PRIORITY
    const rightPriority = providerPriorities[right.id] ?? UNCONFIGURED_PROVIDER_PRIORITY
    if (leftPriority !== rightPriority) {
      return leftPriority - rightPriority
    }
    return new Date(left.created_at).getTime() - new Date(right.created_at).getTime()
  })
}
