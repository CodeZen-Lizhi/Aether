<template>
  <Card
    class="min-w-0 p-4 sm:p-5"
    aria-labelledby="usage-trend-title"
    :aria-busy="loading"
  >
    <div class="mb-5 flex flex-wrap items-center justify-between gap-3">
      <div>
        <h2
          id="usage-trend-title"
          class="text-sm font-semibold text-foreground"
        >
          使用趋势
        </h2>
        <p class="mt-1 text-xs tabular-nums text-muted-foreground">
          {{ rangeLabel }}
        </p>
      </div>
      <slot name="controls" />
    </div>

    <div class="relative h-64 min-w-0 w-full sm:h-72 xl:h-80">
      <Skeleton
        v-if="loading"
        class="h-full w-full"
      />
      <div
        v-else-if="error"
        role="status"
        class="flex h-full flex-col items-center justify-center gap-3 text-sm text-muted-foreground"
      >
        <p>使用趋势加载失败，请重试。</p>
        <Button
          variant="outline"
          size="sm"
          @click="$emit('retry')"
        >
          重试
        </Button>
      </div>
      <LineChart
        v-else-if="trend.hasActivity && trend.points.length"
        :data="chartData"
        :options="chartOptions"
        role="img"
        :aria-label="legacyT('使用趋势：费用、缓存创建、缓存命中、输入和输出。完整数值可在下方查看趋势数据。')"
      />
      <div
        v-else
        class="flex h-full items-center justify-center text-sm text-muted-foreground"
      >
        所选时间内暂无使用数据
      </div>
    </div>

    <template v-if="!loading && !error && trend.hasActivity">
      <div class="mt-3 flex flex-wrap items-center justify-center gap-x-2 gap-y-1">
        <button
          v-for="metric in metrics"
          :key="metric.key"
          type="button"
          :aria-pressed="!hiddenMetrics.has(metric.key)"
          class="inline-flex min-h-9 items-center gap-2 rounded-md px-2 py-1 text-xs transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          :class="hiddenMetrics.has(metric.key) ? 'text-muted-foreground line-through opacity-60' : 'text-foreground'"
          @click="toggleMetric(metric.key)"
        >
          <svg
            width="24"
            height="12"
            viewBox="0 0 24 12"
            aria-hidden="true"
            :style="{ color: metric.color }"
          >
            <path
              d="M1 6H23"
              stroke="currentColor"
              stroke-width="2"
              :stroke-dasharray="metric.key === 'cost' ? '4 3' : undefined"
            />
            <circle
              cx="12"
              cy="6"
              r="3"
              fill="var(--card)"
              stroke="currentColor"
              stroke-width="2"
            />
          </svg>
          {{ metric.label }}
        </button>
      </div>

      <p
        v-if="trend.hasMissingDetails"
        role="status"
        class="mt-3 text-xs leading-relaxed text-muted-foreground [overflow-wrap:anywhere]"
      >
        {{ granularity === 'hour'
          ? '部分历史记录未保留请求明细，无法还原小时趋势；切换为按天可查看保留的费用。'
          : '部分历史记录未保留 Token 明细，对应曲线留空，已保留的费用仍完整展示。' }}
      </p>

      <details class="mt-3 text-xs">
        <summary class="w-fit cursor-pointer rounded py-1 text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
          查看趋势数据
        </summary>
        <p class="mt-2 leading-relaxed text-muted-foreground">
          输入 Token 沿用上游口径，部分格式包含缓存，各项不直接相加。
        </p>
        <div class="responsive-list mt-3">
          <Table class="responsive-list-table">
            <TableHeader>
              <TableRow>
                <TableHead>时段</TableHead>
                <TableHead
                  v-for="metric in metrics"
                  :key="metric.key"
                  class="text-right"
                >
                  {{ metric.label }}
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow
                v-for="point in trend.points"
                :key="point.date"
              >
                <TableCell>{{ displayDate(point.date) }}</TableCell>
                <TableCell
                  v-for="metric in metrics"
                  :key="metric.key"
                  class="text-right tabular-nums"
                >
                  {{ displayValue(point[metric.key], metric.key) }}
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
          <div class="responsive-list-cards space-y-2">
            <div
              v-for="point in trend.points"
              :key="point.date"
              class="rounded-lg border border-border/60 p-3"
            >
              <p class="mb-2 font-medium">
                {{ displayDate(point.date) }}
              </p>
              <dl class="grid grid-cols-2 gap-x-4 gap-y-2 sm:grid-cols-3">
                <div
                  v-for="metric in metrics"
                  :key="metric.key"
                  class="min-w-0"
                >
                  <dt class="text-muted-foreground">
                    {{ metric.label }}
                  </dt>
                  <dd class="mt-0.5 tabular-nums [overflow-wrap:anywhere]">
                    {{ displayValue(point[metric.key], metric.key) }}
                  </dd>
                </div>
              </dl>
            </div>
          </div>
        </div>
      </details>
    </template>
  </Card>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { ChartData, ChartOptions } from 'chart.js'
import type { UsageTimeSeriesPoint } from '@/api/admin'
import type { DailyStat } from '@/api/dashboard'
import { Button, Card, Skeleton, Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui'
import LineChart from '@/components/charts/LineChart.vue'
import { useDarkMode } from '@/composables/useDarkMode'
import { useI18n } from '@/i18n'
import { buildUsageTrend, type TrendGranularity, type TrendMetric } from './usageTrend'

const props = withDefaults(defineProps<{
  series: UsageTimeSeriesPoint[]
  dailyStats: DailyStat[]
  granularity?: TrendGranularity
  loading: boolean
  error: boolean
}>(), { granularity: 'day' })
defineEmits<{ retry: [] }>()

const { isDark } = useDarkMode()
const { locale, legacyT } = useI18n()
const hiddenMetrics = ref(new Set<TrendMetric>())
const trend = computed(() => buildUsageTrend(props.series, props.dailyStats, props.granularity))
const metrics = computed(() => [
  { key: 'cost' as const, label: '费用', color: isDark.value ? '#fb7185' : '#e11d48', fill: 'transparent', shape: 'circle' as const },
  { key: 'cacheCreation' as const, label: '缓存创建', color: isDark.value ? '#fb923c' : '#ea580c', fill: 'rgba(249,115,22,0.08)', shape: 'rect' as const },
  { key: 'cacheRead' as const, label: '缓存命中', color: isDark.value ? '#a78bfa' : '#7c3aed', fill: 'rgba(139,92,246,0.12)', shape: 'triangle' as const },
  { key: 'input' as const, label: '输入 Token', color: isDark.value ? '#60a5fa' : '#2563eb', fill: 'rgba(59,130,246,0.08)', shape: 'rectRot' as const },
  { key: 'output' as const, label: '输出 Token', color: isDark.value ? '#34d399' : '#059669', fill: 'rgba(16,185,129,0.08)', shape: 'crossRot' as const },
])

const rangeLabel = computed(() => {
  if (props.loading) return ''
  const source = props.granularity === 'hour' || !props.dailyStats.length ? props.series : props.dailyStats
  const dates = source.map(point => point.date).sort()
  const start = dates[0]
  const end = dates[dates.length - 1]
  return start && end ? `${displayDate(start)} – ${displayDate(end)}` : ''
})

function displayDate(value: string): string {
  return value.slice(0, props.granularity === 'hour' ? 16 : 10).replace('T', ' ').replaceAll('-', '/')
}

function displayValue(value: number | null, metric: TrendMetric): string {
  if (value === null) return legacyT('明细缺失')
  return metric === 'cost'
    ? `$${value.toLocaleString(locale.value, { minimumFractionDigits: 2, maximumFractionDigits: 6 })}`
    : value.toLocaleString(locale.value)
}

function toggleMetric(metric: TrendMetric) {
  const next = new Set(hiddenMetrics.value)
  if (next.has(metric)) next.delete(metric)
  else next.add(metric)
  hiddenMetrics.value = next
}

const chartData = computed<ChartData<'line'>>(() => ({
  labels: trend.value.points.map(point => props.granularity === 'hour'
    ? point.date.slice(11, 16)
    : point.date.slice(5, 10).replace('-', '/')),
  datasets: metrics.value.map(metric => ({
    label: legacyT(metric.label),
    data: trend.value.points.map(point => point[metric.key]),
    yAxisID: metric.key === 'cost' ? 'cost' : 'tokens',
    borderColor: metric.color,
    backgroundColor: metric.fill,
    borderWidth: 2,
    borderDash: metric.key === 'cost' ? [6, 5] : [],
    pointStyle: metric.shape,
    pointRadius: trend.value.points.length === 1 ? 4 : 0,
    pointHoverRadius: 4,
    pointHitRadius: 12,
    fill: metric.key === 'cacheRead',
    cubicInterpolationMode: 'monotone',
    spanGaps: false,
    hidden: hiddenMetrics.value.has(metric.key),
  })),
}))

const chartOptions = computed<ChartOptions<'line'>>(() => {
  const textColor = isDark.value ? '#a1a1aa' : '#64748b'
  const gridColor = isDark.value ? 'rgba(161,161,170,0.12)' : 'rgba(100,116,139,0.12)'
  return {
    responsive: true,
    maintainAspectRatio: false,
    animation: false,
    interaction: { mode: 'index', intersect: false },
    scales: {
      x: {
        grid: { display: false },
        border: { display: false },
        ticks: { color: textColor, maxRotation: 0, autoSkip: true, maxTicksLimit: 8, font: { size: 11 } },
      },
      tokens: {
        type: 'linear', position: 'left', beginAtZero: true,
        title: { display: true, text: 'Tokens', color: textColor, font: { size: 11 } },
        grid: { color: gridColor }, border: { display: false, dash: [4, 4] },
        ticks: {
          color: textColor, maxTicksLimit: 5, font: { size: 11 },
          callback: value => Number(value).toLocaleString(locale.value, { notation: 'compact', maximumFractionDigits: 1 }),
        },
      },
      cost: {
        type: 'linear', position: 'right', beginAtZero: true,
        title: { display: true, text: `${legacyT('费用')} ($)`, color: textColor, font: { size: 11 } },
        grid: { drawOnChartArea: false }, border: { display: false },
        ticks: {
          color: textColor, maxTicksLimit: 5, font: { size: 11 },
          callback: value => `$${Number(value).toLocaleString(locale.value, { maximumFractionDigits: 6 })}`,
        },
      },
    },
    plugins: {
      legend: { display: false },
      tooltip: {
        backgroundColor: isDark.value ? '#27272a' : '#ffffff',
        titleColor: isDark.value ? '#fafafa' : '#0f172a',
        bodyColor: isDark.value ? '#e4e4e7' : '#334155',
        borderColor: gridColor, borderWidth: 1, padding: 12, usePointStyle: true,
        callbacks: {
          title: items => displayDate(trend.value.points[items[0]?.dataIndex ?? 0]?.date ?? ''),
          label: item => {
            const metric = metrics.value[item.datasetIndex]
            return metric ? `${item.dataset.label}: ${displayValue(typeof item.raw === 'number' ? item.raw : null, metric.key)}` : ''
          },
        },
      },
    },
  }
})
</script>
