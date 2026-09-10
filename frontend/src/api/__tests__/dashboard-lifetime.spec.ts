import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { DailyStat } from '@/api/dashboard'

const { getMock } = vi.hoisted(() => ({ getMock: vi.fn() }))
vi.mock('@/api/client', () => ({ default: { get: getMock } }))

import { dashboardApi } from '@/api/dashboard'
import { cache } from '@/utils/cache'

function day(date: string, values: Partial<DailyStat> = {}): DailyStat {
  return {
    date, requests: 0, tokens: 0, cost: 0, actual_cost: 0,
    avg_response_time: 0, unique_models: 0, model_breakdown: [], ...values,
  }
}
function reply(days: DailyStat[]) {
  return { data: { daily_stats: days, model_summary: [], period: { start_date: '1970-01-01', end_date: '2026-09-10', days: 20707 } } }
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2026-09-09T18:00:00Z'))
  getMock.mockReset()
  cache.clear()
})
afterEach(() => vi.useRealTimers())

describe('dashboard lifetime statistics', () => {
  it('includes retained history and raw daily totals exactly once and caches only the summary', async () => {
    getMock.mockResolvedValue(reply([
      day('1970-01-01'),
      day('2024-02-10', { requests: 9, tokens: 105_000_000, cost: 24, actual_cost: 16.8, unattributed_requests: 9, unattributed_cost: 24 }),
      day('2026-09-10', { requests: 30, tokens: 1_464_000_000, cost: 144, actual_cost: 100.8 }),
    ]))
    const params = { timezone: 'Asia/Shanghai', tz_offset_minutes: 480 }
    const result = await dashboardApi.getLifetimeStats(params)
    expect(result).toEqual({ requests: 39, tokens: 1_569_000_000, cost: 168, actualCost: 117.6, firstActiveDate: '2024-02-10' })
    expect(getMock).toHaveBeenCalledWith('/api/dashboard/daily-stats', {
      params: { start_date: '1970-01-01', end_date: '2026-09-10', ...params },
    })
    expect(await dashboardApi.getLifetimeStats(params)).toBe(result)
    expect(getMock).toHaveBeenCalledTimes(1)
    expect(result).not.toHaveProperty('daily_stats')
  })

  it('returns a genuine zero for an empty history', async () => {
    getMock.mockResolvedValue(reply([day('1970-01-01')]))
    expect(await dashboardApi.getLifetimeStats()).toEqual({ requests: 0, tokens: 0, cost: 0, actualCost: 0, firstActiveDate: null })
  })

  it('rejects a partial or invalid total and allows a clean retry', async () => {
    getMock.mockResolvedValueOnce(reply([
      day('2026-09-09', { requests: 1, tokens: 50 }),
      day('2026-09-10', { requests: 2, tokens: Number.NaN }),
    ])).mockResolvedValueOnce(reply([day('2026-09-10', { requests: 3, tokens: 150 })]))
    await expect(dashboardApi.getLifetimeStats()).rejects.toThrow('Invalid dashboard lifetime statistics')
    expect(await dashboardApi.getLifetimeStats()).toMatchObject({ requests: 3, tokens: 150 })
    expect(getMock).toHaveBeenCalledTimes(2)
  })

  it('uses the requested local date and separates cached totals across midnight', async () => {
    getMock.mockResolvedValue(reply([]))
    const params = { timezone: 'America/Los_Angeles', tz_offset_minutes: -420 }
    await dashboardApi.getLifetimeStats(params)
    expect(getMock).toHaveBeenLastCalledWith('/api/dashboard/daily-stats', {
      params: { start_date: '1970-01-01', end_date: '2026-09-09', ...params },
    })
    vi.setSystemTime(new Date('2026-09-10T07:00:01Z'))
    await dashboardApi.getLifetimeStats(params)
    expect(getMock).toHaveBeenLastCalledWith('/api/dashboard/daily-stats', {
      params: { start_date: '1970-01-01', end_date: '2026-09-10', ...params },
    })
    expect(getMock).toHaveBeenCalledTimes(2)
  })
})
