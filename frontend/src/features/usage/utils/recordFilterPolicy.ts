import type {
  FilterStatusValue,
  UsageAttemptFilterValue,
  UsageLifecycleFilterValue,
  UsageRecord,
  UsageTransportFilterValue,
} from '../types'
import {
  hasUsageFallback,
  hasUsageRetry,
  isUsageRecordFailed,
  isUsageWebSocket,
  resolveDisplayRequestStatus,
  resolveUsageStreamModes,
} from './status'

/** 使用记录页四个独立筛选维度；组间同时满足。 */
export interface UsageLiveFilters {
  /** 请求语义类型；`__all__` 表示不限制。 */
  requestType: string
  /** 请求生命周期；`active` 同时覆盖等待中和传输中。 */
  requestStatus: UsageLifecycleFilterValue
  /** 客户端可见传输方式。 */
  transport: UsageTransportFilterValue
  /** 是否发生重试或转移。 */
  attempt: UsageAttemptFilterValue
}

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

/** 核对轮询后的四个独立筛选，避免终态更新后仍留在不匹配的结果中。 */
export function matchesUsageLiveFilters(record: UsageRecord, filters: UsageLiveFilters): boolean {
  if (filters.requestType !== '__all__') {
    const recordType = record.request_type?.trim().toLowerCase() || 'unknown'
    if (recordType !== filters.requestType.trim().toLowerCase()) return false
  }

  if (filters.requestStatus === 'active') {
    const status = resolveDisplayRequestStatus(record)
    if (status !== 'pending' && status !== 'streaming') return false
  } else if (filters.requestStatus === 'failed') {
    if (!isUsageRecordFailed(record)) return false
  } else if (filters.requestStatus === 'cancelled') {
    if (record.status !== 'cancelled') return false
  }

  if (filters.transport !== '__all__') {
    if (filters.transport === 'websocket') {
      if (!isUsageWebSocket(record)) return false
    } else {
      const clientRequestedStream = resolveUsageStreamModes(record).clientRequestedStream
      if (isUsageWebSocket(record) || clientRequestedStream !== (filters.transport === 'stream')) {
        return false
      }
    }
  }

  if (filters.attempt === 'retry' && !hasUsageRetry(record)) return false
  if (filters.attempt === 'fallback' && !hasUsageFallback(record)) return false
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
