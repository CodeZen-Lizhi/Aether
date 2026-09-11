import { describe, expect, it } from 'vitest'
import type { UsageTimeSeriesPoint } from '@/api/admin'
import type { DailyStat } from '@/api/dashboard'
import { buildUsageTrend, calculateCacheHitRate } from '../usageTrend'

function series(date: string, overrides: Partial<UsageTimeSeriesPoint> = {}): UsageTimeSeriesPoint {
  return {
    date, total_requests: 2, input_tokens: 100, output_tokens: 40,
    total_input_context: 420, cache_creation_tokens: 20, cache_read_tokens: 300, total_cost: 0.4,
    ...overrides,
  }
}

function daily(date: string, overrides: Partial<DailyStat> = {}): DailyStat {
  return {
    date, requests: 2, tokens: 460, cost: 0.4, actual_cost: 0.2,
    avg_response_time: 0, unique_models: 1, model_breakdown: [],
    ...overrides,
  }
}

describe('usage trend data', () => {
  it('does not double-count OpenAI cached tokens in the cache hit rate denominator', () => {
    expect(calculateCacheHitRate({ totalInputContext: 26_569_775, cacheRead: 25_197_056 }))
      .toBeCloseTo(94.83, 2)
    expect(calculateCacheHitRate({ totalInputContext: 420, cacheRead: 300 }))
      .toBeCloseTo(71.43, 2)
    expect(calculateCacheHitRate({ totalInputContext: null, cacheRead: 10 })).toBeNull()
    expect(calculateCacheHitRate({ totalInputContext: 0, cacheRead: 0 })).toBeNull()
  })

  it('keeps all four token categories separate and uses retained daily costs', () => {
    const trend = buildUsageTrend([series('2026-09-09')], [daily('2026-09-09', { cost: 0.5 })])
    expect(trend.points).toEqual([{
      date: '2026-09-09', cost: 0.5, input: 100, totalInputContext: 420,
      output: 40, cacheCreation: 20, cacheRead: 300,
    }])
    expect(trend.hasMissingDetails).toBe(false)
  })

  it('leaves partial historical token details blank instead of plotting zero or incomplete totals', () => {
    const trend = buildUsageTrend([series('2026-09-09')], [daily('2026-09-09', { requests: 5, cost: 1.25 })])
    expect(trend.points[0]).toEqual({
      date: '2026-09-09', cost: 1.25, input: null, totalInputContext: null,
      output: null, cacheCreation: null, cacheRead: null,
    })
    expect(trend.hasMissingDetails).toBe(true)
  })

  it('preserves dates that have only historical totals and distinguishes a zero-usage date', () => {
    const trend = buildUsageTrend([], [
      daily('2026-09-09'),
      daily('2026-09-08', { requests: 0, cost: 0 }),
    ])
    expect(trend.points.map(point => [point.date, point.cost, point.input])).toEqual([
      ['2026-09-08', 0, 0], ['2026-09-09', 0.4, null],
    ])
    expect(trend.hasActivity).toBe(true)
  })

  it('aligns ISO weeks across calendar years and calendar months with daily totals', () => {
    const days = [daily('2025-12-31'), daily('2026-01-01')]
    const weekly = buildUsageTrend([series('2025-12-31', { total_requests: 4 })], days, 'week')
    expect(weekly.points).toHaveLength(1)
    expect(weekly.points[0]).toMatchObject({ cost: 0.8, input: 100 })
    const monthly = buildUsageTrend([
      series('2025-12-01'), series('2026-01-01'),
    ], days, 'month')
    expect(monthly.points.map(point => point.cost)).toEqual([0.4, 0.4])
    expect(monthly.hasMissingDetails).toBe(false)
  })

  it('does not shift already-local hourly labels or allocate a daily cost across missing hours', () => {
    const hours = [
      series('2026-09-09T00:00:00+00:00', { total_requests: 1, total_cost: 0.1 }),
      series('2026-09-09T01:00:00+00:00', { total_requests: 1, total_cost: 0.3 }),
    ]
    const complete = buildUsageTrend(hours, [daily('2026-09-09')], 'hour')
    expect(complete.points.map(point => point.date)).toEqual(hours.map(point => point.date))
    expect(complete.points.map(point => point.cost)).toEqual([0.1, 0.3])
    const incomplete = buildUsageTrend(hours, [daily('2026-09-09', { requests: 3 })], 'hour')
    expect(incomplete.points.every(point => point.cost === null && point.cacheRead === null)).toBe(true)
    expect(incomplete.hasMissingDetails).toBe(true)
  })

  it('marks invalid token values as missing rather than treating them as zero', () => {
    const trend = buildUsageTrend([series('2026-09-09', { input_tokens: Number.NaN })], [daily('2026-09-09')])
    expect(trend.points[0]?.input).toBeNull()
    expect(trend.hasMissingDetails).toBe(true)
  })
})
