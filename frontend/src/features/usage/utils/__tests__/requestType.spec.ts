import { describe, expect, it } from 'vitest'

import type { UsageRecord } from '../../types'
import {
  buildUsageRequestTypeOptions,
  formatUsageRequestType,
  normalizeUsageRequestType,
} from '../requestType'

describe('usage request type presentation', () => {
  it('normalizes missing request types to the filterable unknown bucket', () => {
    expect(normalizeUsageRequestType(null)).toBe('unknown')
    expect(normalizeUsageRequestType('  COMPACT  ')).toBe('compact')
    expect(formatUsageRequestType('compact')).toBe('会话压缩')
    expect(formatUsageRequestType('future-operation')).toBe('future-operation')
  })

  it('keeps known, server-aggregated, current-page and selected types in stable order', () => {
    const records = [
      { request_type: 'legacy-custom' },
      { request_type: 'compact' },
      { request_type: null },
    ] as Pick<UsageRecord, 'request_type'>[]

    expect(buildUsageRequestTypeOptions(
      ['chat', 'video'],
      records,
      'selected-history',
    )).toEqual([
      { value: 'chat', label: '对话' },
      { value: 'compact', label: '会话压缩' },
      { value: 'video', label: '视频' },
      { value: 'unknown', label: '未知类型' },
      { value: 'legacy-custom', label: 'legacy-custom' },
      { value: 'selected-history', label: 'selected-history' },
    ])
  })
})
