import { describe, expect, it } from 'vitest'

import {
  getCacheCreationTokens,
  getCacheHitRate,
  getCacheReadTokens,
  getEffectiveInputTokens,
  getTotalInputContextTokens,
} from '../token-normalization'

describe('usage token normalization', () => {
  it('prefers explicit cache creation totals when present', () => {
    expect(getCacheCreationTokens({
      cache_creation_input_tokens: 18,
      cache_creation_ephemeral_5m_input_tokens: 5,
      cache_creation_ephemeral_1h_input_tokens: 7,
    })).toBe(18)
  })

  it('rehydrates cache creation totals from classified fields when legacy total is zero', () => {
    expect(getCacheCreationTokens({
      cache_creation_input_tokens: 0,
      cache_creation_ephemeral_5m_input_tokens: 12,
      cache_creation_ephemeral_1h_input_tokens: 8,
    })).toBe(20)
  })

  it('keeps cache read and cache creation as separate display values', () => {
    const usage = {
      input_tokens: 1,
      cache_creation_input_tokens: 1530,
      cache_read_input_tokens: 104026,
      output_tokens: 591,
      api_format: 'claude:messages',
    }

    expect(getEffectiveInputTokens(usage)).toBe(1)
    expect(getCacheReadTokens(usage)).toBe(104026)
    expect(getCacheCreationTokens(usage)).toBe(1530)
  })

  it('keeps effective input token normalization unchanged', () => {
    expect(getEffectiveInputTokens({
      input_tokens: 100,
      cache_read_input_tokens: 20,
      api_format: 'openai:chat',
    })).toBe(80)
  })

  it('does not subtract cache read tokens for Claude usage', () => {
    expect(getEffectiveInputTokens({
      input_tokens: 4941,
      cache_creation_input_tokens: 687,
      cache_read_input_tokens: 52873,
      output_tokens: 973,
      api_format: 'claude:messages',
    } as unknown as Parameters<typeof getEffectiveInputTokens>[0])).toBe(4941)
  })

  it.each([
    ['openai:chat', 120, 10, 15, 120, 12.5],
    ['gemini:generate_content', 120, 10, 15, 120, 12.5],
    ['claude:messages', 60, 15, 5, 80, 6.25],
    [null, 20, 10, 5, 35, 14.29],
  ])(
    'calculates cache hit rate with backend context semantics for %s',
    (apiFormat, inputTokens, cacheCreation, cacheRead, expectedContext, expectedRate) => {
      const usage = {
        input_tokens: inputTokens,
        cache_creation_input_tokens: cacheCreation,
        cache_read_input_tokens: cacheRead,
        api_format: apiFormat,
      }

      expect(getTotalInputContextTokens(usage)).toBe(expectedContext)
      expect(getCacheHitRate(usage)).toBe(expectedRate)
    },
  )

  it('returns no cache hit rate when the normalized input context is empty', () => {
    expect(getTotalInputContextTokens({
      input_tokens: 0,
      cache_creation_input_tokens: 0,
      cache_read_input_tokens: 0,
    })).toBe(0)
    expect(getCacheHitRate({
      input_tokens: 0,
      cache_creation_input_tokens: 0,
      cache_read_input_tokens: 0,
    })).toBeNull()
  })
})
