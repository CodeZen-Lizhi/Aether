import type { UsageTimeSeriesPoint } from '@/api/admin'
import type { DailyStat, TimeRangeParams } from '@/api/dashboard'

export type TrendGranularity = NonNullable<TimeRangeParams['granularity']>
export type TrendMetric = 'cost' | 'cacheCreation' | 'cacheRead' | 'input' | 'output'

export interface UsageTrendPoint {
  date: string
  cost: number | null
  cacheCreation: number | null
  cacheRead: number | null
  input: number | null
  output: number | null
}

export function calculateCacheHitRate(
  point: Pick<UsageTrendPoint, 'input' | 'cacheCreation' | 'cacheRead'>,
): number | null {
  if (point.input === null || point.cacheCreation === null || point.cacheRead === null) {
    return null
  }

  const totalInputContext = point.input
    + (point.cacheCreation > 0 ? point.cacheCreation : 0)
    + point.cacheRead
  if (totalInputContext <= 0) return null

  return point.cacheRead / totalInputContext * 100
}

interface DailyTotal {
  date: string
  requests: number
  cost: number
}

function dateGroup(date: string, granularity: TrendGranularity): string {
  // Hour labels carry +00:00 but are already offset by the backend. Do not
  // convert them through the browser timezone a second time.
  const localDate = date.slice(0, 10)
  if (granularity === 'month') return localDate.slice(0, 7)
  if (granularity !== 'week') return localDate
  const monday = new Date(`${localDate}T00:00:00Z`)
  monday.setUTCDate(monday.getUTCDate() - (monday.getUTCDay() + 6) % 7)
  return monday.toISOString().slice(0, 10)
}

function tokenValue(value: number): number | null {
  return Number.isFinite(value) && value >= 0 ? value : null
}

export function buildUsageTrend(
  series: readonly UsageTimeSeriesPoint[],
  dailyStats: readonly DailyStat[],
  granularity: TrendGranularity = 'day',
) {
  const totals = new Map<string, DailyTotal>()
  for (const day of dailyStats) {
    const key = dateGroup(day.date, granularity)
    const total = totals.get(key) ?? { date: day.date, requests: 0, cost: 0 }
    total.date = total.date < day.date ? total.date : day.date
    total.requests += day.requests
    total.cost += day.cost
    totals.set(key, total)
  }

  const requestCounts = new Map<string, number>()
  for (const point of series) {
    const key = dateGroup(point.date, granularity)
    requestCounts.set(key, (requestCounts.get(key) ?? 0) + point.total_requests)
  }
  const incompleteGroups = new Set([...totals].filter(([key, total]) =>
    total.requests > (requestCounts.get(key) ?? 0),
  ).map(([key]) => key))

  const points: UsageTrendPoint[] = series.map((point) => {
    const key = dateGroup(point.date, granularity)
    const incomplete = incompleteGroups.has(key)
    const rawCost = Number.isFinite(point.total_cost) ? point.total_cost : null
    return {
      date: point.date,
      // A saved daily cost cannot be allocated to individual hours.
      cost: granularity === 'hour'
        ? (incomplete ? null : rawCost)
        : (totals.get(key)?.cost ?? rawCost),
      cacheCreation: incomplete ? null : tokenValue(point.cache_creation_tokens),
      cacheRead: incomplete ? null : tokenValue(point.cache_read_tokens),
      input: incomplete ? null : tokenValue(point.input_tokens),
      output: incomplete ? null : tokenValue(point.output_tokens),
    }
  })

  if (granularity !== 'hour') {
    for (const [key, total] of totals) {
      if (requestCounts.has(key)) continue
      const tokens = total.requests > 0 ? null : 0
      points.push({
        date: granularity === 'month' ? `${key}-01` : total.date,
        cost: total.cost,
        cacheCreation: tokens,
        cacheRead: tokens,
        input: tokens,
        output: tokens,
      })
    }
  }
  points.sort((left, right) => left.date.localeCompare(right.date))

  return {
    points,
    hasActivity: dailyStats.some(day => day.requests > 0 || day.cost > 0)
      || series.some(point => point.total_requests > 0 || point.total_cost > 0),
    hasMissingDetails: incompleteGroups.size > 0 || points.some(point =>
      point.input === null || point.output === null
      || point.cacheCreation === null || point.cacheRead === null,
    ),
  }
}
