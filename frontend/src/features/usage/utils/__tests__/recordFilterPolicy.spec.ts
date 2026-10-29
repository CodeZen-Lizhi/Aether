import { describe, expect, it } from 'vitest'

import {
  isUserLocalOnlyRecordStatus,
  matchesUsageLiveFilters,
  shouldUseServerUserRecordFilters,
} from '../recordFilterPolicy'
import type { UsageRecord } from '../../types'

function buildUsageRecord(overrides: Partial<UsageRecord> = {}): UsageRecord {
  return {
    id: 'usage-1',
    model: 'gpt-5',
    request_type: 'compact',
    input_tokens: 1,
    output_tokens: 1,
    total_tokens: 2,
    cost: 0.01,
    is_stream: true,
    client_requested_stream: true,
    upstream_is_stream: true,
    status: 'failed',
    has_retry: true,
    created_at: '2026-10-29T00:00:00Z',
    ...overrides,
  }
}

describe('normal-user usage record filter policy', () => {
  it.each(['has_retry', 'has_fallback'] as const)(
    'keeps %s local even when combined with server-supported filters',
    (status) => {
      expect(isUserLocalOnlyRecordStatus(status)).toBe(true)
      expect(shouldUseServerUserRecordFilters({
        search: 'production',
        apiFormat: 'codex:live',
        status,
      })).toBe(false)
    },
  )

  it.each([
    { search: '', apiFormat: '__all__', status: 'websocket' as const },
    { search: '', apiFormat: 'codex:live', status: '__all__' as const },
    { search: 'production', apiFormat: '__all__', status: '__all__' as const },
    { search: 'production', apiFormat: 'codex:live', status: 'websocket' as const },
  ])('uses server pagination for supported filter combination %#', (filters) => {
    expect(shouldUseServerUserRecordFilters(filters)).toBe(true)
  })

  it('keeps the unfiltered normal-user list on its existing local pagination path', () => {
    expect(shouldUseServerUserRecordFilters({
      search: '   ',
      apiFormat: '__all__',
      status: '__all__',
    })).toBe(false)
  })

  it('combines lifecycle, transport and attempt filters', () => {
    const matching = buildUsageRecord()
    expect(matchesUsageLiveFilters(matching, {
      requestStatus: 'failed',
      transport: 'stream',
      attempt: 'retry',
    })).toBe(true)
    expect(matchesUsageLiveFilters(matching, {
      requestStatus: 'failed',
      transport: 'standard',
      attempt: 'retry',
    })).toBe(false)
  })

  it('does not treat the displayed request type as a filter dimension', () => {
    const record = buildUsageRecord({ request_type: 'compact' })

    expect(matchesUsageLiveFilters(record, {
      requestStatus: '__all__',
      transport: '__all__',
      attempt: '__all__',
    })).toBe(true)
  })

  it('uses the client-visible transport mode for converted HTTP requests', () => {
    const converted = buildUsageRecord({
      request_type: 'chat',
      status: 'completed',
      client_requested_stream: false,
      upstream_is_stream: true,
      has_retry: false,
    })
    expect(matchesUsageLiveFilters(converted, {
      requestStatus: '__all__',
      transport: 'standard',
      attempt: '__all__',
    })).toBe(true)
    expect(matchesUsageLiveFilters(converted, {
      requestStatus: '__all__',
      transport: 'stream',
      attempt: '__all__',
    })).toBe(false)
  })
})
