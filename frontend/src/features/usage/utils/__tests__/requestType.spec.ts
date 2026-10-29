import { describe, expect, it } from 'vitest'

import {
  formatUsageRequestType,
  normalizeUsageRequestType,
} from '../requestType'

describe('usage request type presentation', () => {
  it('normalizes missing request types to the display-only unknown bucket', () => {
    expect(normalizeUsageRequestType(null)).toBe('unknown')
    expect(normalizeUsageRequestType('  COMPACT  ')).toBe('compact')
    expect(formatUsageRequestType('compact')).toBe('会话压缩')
    expect(formatUsageRequestType('future-operation')).toBe('future-operation')
  })
})
