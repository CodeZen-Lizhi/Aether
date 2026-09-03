import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, defineComponent, h, nextTick, type App } from 'vue'

import Dashboard from '../Dashboard.vue'

const dashboardApiMocks = vi.hoisted(() => ({
  getStats: vi.fn(),
  getDailyStats: vi.fn(),
}))

vi.mock('@/api/dashboard', () => ({
  dashboardApi: dashboardApiMocks,
}))

vi.mock('@/api/announcements', () => ({
  announcementApi: {
    getAnnouncements: vi.fn().mockResolvedValue({ items: [] }),
    markAsRead: vi.fn().mockResolvedValue({}),
  },
}))

vi.mock('@/components/charts/BarChart.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'BarChartStub', setup: () => () => h('div') }) }
})

vi.mock('@/components/charts/DoughnutChart.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'DoughnutChartStub', setup: () => () => h('div') }) }
})

vi.mock('@/components/common', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    TimeRangePicker: defineComponent({
      name: 'TimeRangePickerStub',
      setup() {
        return () => h('div')
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

beforeEach(() => {
  dashboardApiMocks.getStats.mockReset()
  dashboardApiMocks.getDailyStats.mockReset()
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

describe('Dashboard refresh controls', () => {
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

    const primaryCosts = [...root.querySelectorAll<HTMLElement>('.text-primary')]
      .map(element => element.textContent?.trim())
    const settledCosts = [...root.querySelectorAll<HTMLElement>('.text-muted-foreground')]
      .map(element => element.textContent?.trim())

    expect(primaryCosts).toEqual(expect.arrayContaining(['$0.7500', '$0.3000']))
    expect(settledCosts).toEqual(expect.arrayContaining(['$0.0750', '$0.0300']))
  })
})
