import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, defineComponent, h, nextTick, type App } from 'vue'
import type { UsageTimeSeriesPoint } from '@/api/admin'
import type { DailyStatsResponse, DashboardStatsResponse, ModelBreakdown, ModelSummary } from '@/api/dashboard'
import { setI18nLocale } from '@/i18n'

import Dashboard from '../Dashboard.vue'

const dashboardApiMocks = vi.hoisted(() => ({
  getStats: vi.fn(),
  getLifetimeStats: vi.fn(),
  getDailyStats: vi.fn(),
  getTimeSeries: vi.fn(),
}))

vi.mock('@/api/dashboard', () => ({
  dashboardApi: dashboardApiMocks,
}))

vi.mock('@/api/admin', () => ({
  adminApi: dashboardApiMocks,
}))
vi.mock('@/api/usage', () => ({ usageApi: {
  getUsageByModel: vi.fn().mockResolvedValue([]),
  getUsageByProvider: vi.fn().mockResolvedValue([]),
} }))

vi.mock('@/components/charts/LineChart.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({
    name: 'LineChartStub', props: { data: { type: Object, required: true } },
    setup: props => () => h('div', { 'data-chart': 'line', 'data-chart-data': JSON.stringify(props.data) }),
  }) }
})

vi.mock('@/components/common', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    TimeRangePicker: defineComponent({
      name: 'TimeRangePickerStub',
      props: ['modelValue'],
      emits: ['update:modelValue'],
      setup(props, { emit }) {
        return () => h('button', {
          'data-change-range': '',
          'data-preset': props.modelValue?.preset,
          onClick: () => emit('update:modelValue', { preset: 'yesterday', granularity: 'hour', timezone: 'Asia/Shanghai', tz_offset_minutes: 480 }),
        })
      },
    }),
  }
})

vi.mock('@/components/ui', async () => {
  const { defineComponent, h } = await import('vue')
  const passthrough = (name: string, tag = 'div') => defineComponent({
    name,
    setup(_, { slots }) {
      return () => h(tag, slots.default?.())
    },
  })
  return {
    Card: passthrough('CardStub', 'section'),
    Badge: passthrough('BadgeStub', 'span'),
    Button: passthrough('ButtonStub', 'button'),
    Skeleton: defineComponent({ name: 'SkeletonStub', setup: () => () => h('div') }),
    Dialog: passthrough('DialogStub'),
    Table: passthrough('TableStub', 'table'),
    TableHeader: passthrough('TableHeaderStub', 'thead'),
    TableBody: passthrough('TableBodyStub', 'tbody'),
    TableRow: passthrough('TableRowStub', 'tr'),
    TableHead: passthrough('TableHeadStub', 'th'),
    TableCell: passthrough('TableCellStub', 'td'),
  }
})

vi.mock('lucide-vue-next', async () => {
  const Icon = defineComponent({
    name: 'IconStub',
    setup() {
      return () => h('span')
    },
  })
  return {
    Users: Icon,
    Activity: Icon,
    TrendingUp: Icon,
    DollarSign: Icon,
    Key: Icon,
    Hash: Icon,
    Zap: Icon,
    Bell: Icon,
    AlertCircle: Icon,
    AlertTriangle: Icon,
    Info: Icon,
    Wrench: Icon,
    Loader2: Icon,
    Clock: Icon,
    Database: Icon,
    Shuffle: Icon,
  }
})

const mountedApps: Array<{ app: App, root: HTMLElement }> = []

function mountDashboard() {
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp(Dashboard)
  app.mount(root)
  mountedApps.push({ app, root })
  return root
}

async function settle() {
  for (let index = 0; index < 8; index += 1) {
    await Promise.resolve()
    await nextTick()
  }
}

function trendSeries(date = '2026-09-09T12:00:00+00:00', input = 100): UsageTimeSeriesPoint[] {
  return [{
    date, total_requests: 1, input_tokens: input, output_tokens: 20,
    total_input_context: input + 430,
    cache_creation_tokens: 30, cache_read_tokens: 400, total_cost: 0.5,
  }]
}

function trendDays(date = '2026-09-09'): DailyStatsResponse {
  const models = [{ model: 'trend-model', requests: 1, tokens: 550, cost: 0.5 }]
  return {
    daily_stats: [{
      date, requests: 1, tokens: 550, cost: 0.5, actual_cost: 0.2,
      avg_response_time: 0, unique_models: 1,
      model_breakdown: models,
    }],
    model_summary: modelSummaries(models),
    period: { start_date: date, end_date: date, days: 1 },
  }
}

function modelSummaries(rows: ModelBreakdown[]): ModelSummary[] {
  return rows.map(row => ({
    ...row, avg_response_time: 0,
    cost_per_request: row.requests > 0 ? row.cost / row.requests : 0,
    tokens_per_request: row.requests > 0 ? row.tokens / row.requests : 0,
  }))
}

function todayDashboard(responseTime = 16.64, requests = 6): DashboardStatsResponse {
  return {
    stats: [
      { name: '今日请求', value: `${requests}`, icon: 'Activity' },
      { name: '今日 Token', value: '120', icon: 'Zap' },
      { name: '今日费用', value: '$0.7500', subValue: '$0.0750', icon: 'DollarSign' },
      { name: '全站 RPM / TPM', value: '0 / 0', icon: 'Activity' },
    ],
    system_health: {
      avg_response_time: responseTime, total_requests: requests,
      error_rate: 0, error_requests: 0, fallback_count: 0,
    },
  }
}

function todayResponseCard(root: HTMLElement): HTMLElement | null {
  return root.querySelector('[aria-label="今日平均响应"]')
}

function clickButton(root: HTMLElement, text: string) {
  const button = [...root.querySelectorAll('button')].find(button => button.textContent?.trim() === text)
  expect(button, `Button not found: ${text}`).toBeDefined()
  button!.click()
}

function deferred<T>() {
  let resolve: (value: T) => void = () => { throw new Error('Promise not initialized') }
  const promise = new Promise<T>((done) => { resolve = done })
  return { promise, resolve }
}

beforeEach(() => {
  dashboardApiMocks.getStats.mockReset()
  dashboardApiMocks.getStats.mockResolvedValue(todayDashboard())
  dashboardApiMocks.getLifetimeStats.mockReset()
  dashboardApiMocks.getLifetimeStats.mockResolvedValue({
    requests: 39, tokens: 1_569_000_000, cost: 168, actualCost: 117.6, firstActiveDate: '2024-02-10',
  })
  dashboardApiMocks.getDailyStats.mockReset()
  dashboardApiMocks.getTimeSeries.mockReset()
  dashboardApiMocks.getTimeSeries.mockResolvedValue([])
  dashboardApiMocks.getDailyStats.mockResolvedValue({
    daily_stats: [],
    model_summary: [],
    period: { start_date: '2026-05-01', end_date: '2026-05-15', days: 15 },
  })
})

afterEach(() => {
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
  document.body.innerHTML = ''
})

describe('Dashboard overview and trends', () => {
  it('defaults to hourly today and keeps today and lifetime totals independent of trend filters', async () => {
    vi.useFakeTimers()
    dashboardApiMocks.getStats.mockResolvedValue(todayDashboard())
    const days = trendDays()
    days.daily_stats[0].avg_response_time = 99.9
    dashboardApiMocks.getDailyStats.mockResolvedValue(days)
    try {
      const root = mountDashboard()
      await settle()
      expect(dashboardApiMocks.getStats).toHaveBeenCalledWith({
        preset: 'today',
        timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
        tz_offset_minutes: -new Date().getTimezoneOffset(),
      })
      expect(todayResponseCard(root)?.textContent).toContain('16.64s')
      expect(todayResponseCard(root)?.textContent).toContain('今日请求的平均耗时')
      const todayOverview = root.querySelector('[data-dashboard-section="today"]')
      const lifetime = root.querySelector('[aria-labelledby="lifetime-usage-title"]')
      expect(todayOverview?.compareDocumentPosition(lifetime!)).toBe(Node.DOCUMENT_POSITION_FOLLOWING)
      expect(lifetime?.textContent).toContain('1,569,000,000')
      expect(lifetime?.textContent).toContain('39')
      expect(lifetime?.textContent).toContain('2024/02/10')
      expect(lifetime?.textContent).toContain('$168.00')
      expect(lifetime?.textContent).toContain('$117.60')
      expect(dashboardApiMocks.getTimeSeries).toHaveBeenCalledWith({
        preset: 'today', granularity: 'hour',
        timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
        tz_offset_minutes: -new Date().getTimezoneOffset(),
      })
      expect(root.textContent).not.toContain('本月系统健康')
      expect(root.textContent).not.toContain('按平台拆分')
      expect(root.textContent).not.toContain('请求日志')
      expect(root.querySelector('details[open]')).toBeNull()
      expect([...root.querySelectorAll('table')].every(table => table.closest('details'))).toBe(true)
      root.querySelector<HTMLButtonElement>('[data-change-range]')?.click()
      await vi.advanceTimersByTimeAsync(130)
      await settle()
      expect(todayResponseCard(root)?.textContent).toContain('16.64s')
      expect(todayResponseCard(root)?.textContent).not.toContain('99.90s')
      expect(dashboardApiMocks.getStats).toHaveBeenCalledTimes(1)
      expect(dashboardApiMocks.getLifetimeStats).toHaveBeenCalledTimes(1)
      expect(lifetime?.textContent).toContain('1,569,000,000')
      expect(dashboardApiMocks.getDailyStats).toHaveBeenCalledTimes(2)
    } finally {
      vi.useRealTimers()
    }
  })

  it.each([
    [0.4321, '0.43s'],
    [0, '0.00s'],
  ])('shows an average of %s seconds in seconds with two decimals', async (responseTime, expected) => {
    dashboardApiMocks.getStats.mockResolvedValue(todayDashboard(responseTime))
    const root = mountDashboard()
    await settle()
    expect(todayResponseCard(root)?.textContent).toContain(expected)
    expect(todayResponseCard(root)?.textContent).not.toContain('ms')
  })

  it('distinguishes no requests from an unavailable response time without dropping other cards', async () => {
    const missing = todayDashboard()
    delete missing.system_health
    for (const [payload, message] of [
      [todayDashboard(0, 0), '今日暂无请求'],
      [missing, '暂无响应时间数据'],
      [todayDashboard(-1), '暂无响应时间数据'],
      [todayDashboard(Number.NaN), '暂无响应时间数据'],
    ] as const) {
      dashboardApiMocks.getStats.mockResolvedValue(payload)
      const root = mountDashboard()
      await settle()
      expect(todayResponseCard(root)?.textContent).toContain('--')
      expect(todayResponseCard(root)?.textContent).toContain(message)
      expect(todayResponseCard(root)?.textContent).not.toContain('0.00s')
      expect(root.textContent).toContain('$0.7500')
    }
  })

  it('preserves all five today placeholders and other sections if today fails, and retries independently', async () => {
    dashboardApiMocks.getStats.mockRejectedValueOnce(new Error('offline')).mockResolvedValue(todayDashboard())
    dashboardApiMocks.getDailyStats.mockResolvedValue(trendDays())
    dashboardApiMocks.getTimeSeries.mockResolvedValue(trendSeries())
    const root = mountDashboard()
    await settle()
    expect(todayResponseCard(root)?.textContent).toContain('--')
    expect(root.textContent).toContain('今日统计加载失败')
    for (const label of ['今日请求', '今日 Token', '今日费用', '全站 RPM / TPM', '今日平均响应']) {
      expect(root.querySelector(`[aria-label="${label}"]`)?.textContent).toContain('--')
    }
    expect(root.querySelector('[data-chart="line"]')).not.toBeNull()
    expect(root.textContent).toContain('1,569,000,000')
    clickButton(root, '重试今日统计')
    await settle()
    expect(root.textContent).not.toContain('今日统计加载失败')
    expect(todayResponseCard(root)?.textContent).toContain('16.64s')
    expect(dashboardApiMocks.getStats).toHaveBeenCalledTimes(2)
    expect(dashboardApiMocks.getLifetimeStats).toHaveBeenCalledTimes(1)
    expect(dashboardApiMocks.getTimeSeries).toHaveBeenCalledTimes(1)
  })

  it('keeps today and trends available when lifetime totals fail and retries only lifetime', async () => {
    dashboardApiMocks.getLifetimeStats.mockRejectedValueOnce(new Error('offline'))
    dashboardApiMocks.getDailyStats.mockResolvedValue(trendDays())
    dashboardApiMocks.getTimeSeries.mockResolvedValue(trendSeries())
    const root = mountDashboard()
    await settle()
    expect(root.textContent).toContain('累计统计加载失败')
    expect(root.querySelector('[aria-labelledby="lifetime-usage-title"]')?.textContent).toContain('--')
    expect(todayResponseCard(root)?.textContent).toContain('16.64s')
    expect(root.querySelector('[data-chart="line"]')).not.toBeNull()
    clickButton(root, '重试累计统计')
    await settle()
    expect(root.textContent).not.toContain('累计统计加载失败')
    expect(root.textContent).toContain('1,569,000,000')
    expect(dashboardApiMocks.getLifetimeStats).toHaveBeenCalledTimes(2)
    expect(dashboardApiMocks.getStats).toHaveBeenCalledTimes(1)
    expect(dashboardApiMocks.getTimeSeries).toHaveBeenCalledTimes(1)
  })

  it('shows five trend metrics with separate axes and keyboard-accessible legend toggles', async () => {
    dashboardApiMocks.getStats.mockResolvedValue({ stats: [] })
    dashboardApiMocks.getDailyStats.mockResolvedValue(trendDays())
    dashboardApiMocks.getTimeSeries.mockResolvedValue(trendSeries())
    const root = mountDashboard()
    await settle()
    const readChart = () => JSON.parse(root.querySelector('[data-chart="line"]')?.getAttribute('data-chart-data') ?? '{}')
    expect(readChart().datasets.map((dataset: { label: string }) => dataset.label)).toEqual([
      '费用', '缓存创建', '缓存命中', '输入 Token', '输出 Token',
    ])
    expect(readChart().datasets[0]).toMatchObject({ yAxisID: 'cost', data: [0.5] })
    expect(readChart().datasets[2]).toMatchObject({ yAxisID: 'tokens', data: [400], spanGaps: false, fill: true })
    expect(readChart().datasets.filter((dataset: { fill: boolean }) => dataset.fill)).toHaveLength(1)
    expect(root.textContent).not.toContain('查看趋势数据')
    const toggle = [...root.querySelectorAll('button')].find(button => button.textContent?.trim() === '缓存命中')
    expect(toggle?.getAttribute('aria-pressed')).toBe('true')
    toggle?.click()
    await settle()
    expect(toggle?.getAttribute('aria-pressed')).toBe('false')
    expect(readChart().datasets[2].hidden).toBe(true)
    expect(dashboardApiMocks.getTimeSeries).toHaveBeenCalledTimes(1)
  })

  it('keeps other dashboard data when the trend request fails and allows retry', async () => {
    dashboardApiMocks.getStats.mockResolvedValue({ stats: [] })
    dashboardApiMocks.getDailyStats.mockResolvedValue(trendDays())
    dashboardApiMocks.getTimeSeries.mockRejectedValueOnce(new Error('offline')).mockResolvedValue(trendSeries())
    const root = mountDashboard()
    await settle()
    expect(root.textContent).toContain('使用趋势加载失败，请重试。')
    expect(root.querySelector('[data-dashboard-section="lifetime"]')).not.toBeNull()
    clickButton(root, '重试')
    await settle()
    expect(root.textContent).not.toContain('使用趋势加载失败，请重试。')
    expect(root.querySelector('[data-chart="line"]')).not.toBeNull()
    expect(dashboardApiMocks.getTimeSeries).toHaveBeenCalledTimes(2)
  })

  it('discards an older range while a newer hourly range is loading', async () => {
    vi.useFakeTimers()
    const oldDays = deferred<DailyStatsResponse>()
    const oldSeries = deferred<UsageTimeSeriesPoint[]>()
    const newDays = deferred<DailyStatsResponse>()
    const newSeries = deferred<UsageTimeSeriesPoint[]>()
    dashboardApiMocks.getStats.mockResolvedValue({ stats: [] })
    dashboardApiMocks.getDailyStats.mockReturnValueOnce(oldDays.promise).mockReturnValueOnce(newDays.promise)
    dashboardApiMocks.getTimeSeries.mockReturnValueOnce(oldSeries.promise).mockReturnValueOnce(newSeries.promise)
    try {
      const root = mountDashboard()
      await settle()
      root.querySelector<HTMLButtonElement>('[data-change-range]')?.click()
      await settle()
      await vi.advanceTimersByTimeAsync(130)
      oldDays.resolve(trendDays())
      oldSeries.resolve(trendSeries())
      await settle()
      expect(root.querySelector('[data-chart="line"]')).toBeNull()
      expect(root.textContent).not.toContain('2026/09/09')
      expect(dashboardApiMocks.getTimeSeries).toHaveBeenLastCalledWith({ preset: 'yesterday', granularity: 'hour', timezone: 'Asia/Shanghai', tz_offset_minutes: 480 })
      newDays.resolve(trendDays('2026-09-10'))
      newSeries.resolve(trendSeries('2026-09-10T00:00:00+00:00', 700))
      await settle()
      const chart = JSON.parse(root.querySelector('[data-chart="line"]')?.getAttribute('data-chart-data') ?? '{}')
      expect(chart.labels).toEqual(['00:00'])
      expect(chart.datasets[3].data).toEqual([700])
    } finally {
      vi.useRealTimers()
    }
  })

  it('only keeps the trend range picker and removes distribution', async () => {
    vi.useFakeTimers()
    try {
      const root = mountDashboard()
      await settle()
      const controls = root.querySelectorAll<HTMLButtonElement>('[data-change-range]')
      expect(controls).toHaveLength(1)
      expect(root.textContent).not.toContain('用量分布')
      expect(root.textContent).not.toContain('用量分析')
      controls[0].click()
      await vi.advanceTimersByTimeAsync(130)
      await settle()
      expect(controls[0].dataset.preset).toBe('yesterday')
      expect(dashboardApiMocks.getTimeSeries).toHaveBeenLastCalledWith(expect.objectContaining({ preset: 'yesterday', granularity: 'hour' }))
    } finally { vi.useRealTimers() }
  })

  it('does not render or run automatic refresh', async () => {
    vi.useFakeTimers()
    dashboardApiMocks.getStats.mockResolvedValue({ stats: [] })

    try {
      const root = mountDashboard()
      await settle()

      expect(root.textContent).not.toContain('自动刷新')
      expect(dashboardApiMocks.getStats).toHaveBeenCalledTimes(1)
      expect(dashboardApiMocks.getDailyStats).toHaveBeenCalledTimes(1)

      await vi.advanceTimersByTimeAsync(60_000)
      await settle()

      expect(dashboardApiMocks.getStats).toHaveBeenCalledTimes(1)
      expect(dashboardApiMocks.getDailyStats).toHaveBeenCalledTimes(1)
    } finally {
      vi.useRealTimers()
    }
  })

  it('shows original costs as primary values and settled costs as muted secondary values', async () => {
    dashboardApiMocks.getStats.mockResolvedValue({
      stats: [{ name: '今日费用', value: '$0.7500', subValue: '$0.0750', icon: 'DollarSign' }],
      system_health: {
        avg_response_time: 0.5,
        error_rate: 0,
        error_requests: 0,
        fallback_count: 0,
        total_requests: 1,
      },
      cost_stats: {
        total_cost: 0.75,
        total_actual_cost: 0.075,
        cost_savings: 0,
      },
    })
    dashboardApiMocks.getDailyStats.mockResolvedValue({
      daily_stats: [{
        date: '2026-05-15',
        requests: 1,
        tokens: 20,
        cost: 0.3,
        actual_cost: 0.03,
        avg_response_time: 0.5,
        unique_models: 1,
        model_breakdown: [],
      }],
      model_summary: [],
      period: { start_date: '2026-05-15', end_date: '2026-05-15', days: 1 },
    })

    const root = mountDashboard()
    await settle()

    const costCard = root.querySelector('[aria-label="今日费用"]')!
    expect(costCard.querySelector('.text-primary')?.textContent?.trim()).toBe('$0.7500')
    expect([...costCard.querySelectorAll('.text-muted-foreground')].some(element => element.textContent?.includes('实际结算 $0.0750'))).toBe(true)
  })
})
