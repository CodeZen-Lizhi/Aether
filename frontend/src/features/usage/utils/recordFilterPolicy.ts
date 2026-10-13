import type { FilterStatusValue, UsageRecord } from '../types'
import { isUsageRecordFailed, isUsageWebSocket, resolveDisplayRequestStatus } from './status'

/** 核对会随轮询变化的类型筛选；HTTP 类型仅保留已完成行，活跃行终结后立即移出。 */
export function matchesUsageLiveTypeFilter(record: UsageRecord, filter: FilterStatusValue): boolean {
  const status = resolveDisplayRequestStatus(record)
  if (filter === 'active') {
    return status === 'pending' || status === 'streaming'
  }
  if (filter === 'stream' || filter === 'standard') {
    return status === 'completed'
      && !isUsageRecordFailed(record)
      && !isUsageWebSocket(record)
      && record.is_stream === (filter === 'stream')
  }
  return true
}

export function isUserLocalOnlyRecordStatus(status: FilterStatusValue): boolean {
  return status === 'has_retry' || status === 'has_fallback'
}

export function shouldUseServerUserRecordFilters(input: {
  search: string
  apiFormat: string
  status: FilterStatusValue
}): boolean {
  if (isUserLocalOnlyRecordStatus(input.status)) return false

  return input.search.trim().length > 0
    || input.apiFormat !== '__all__'
    || input.status !== '__all__'
}
