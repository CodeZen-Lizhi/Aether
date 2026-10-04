<template>
  <div class="min-w-0 space-y-4">
    <DashboardOverview
      :today="today"
      :today-loading="loadingToday"
      :today-error="todayError"
      :lifetime="lifetime"
      :lifetime-loading="loadingLifetime"
      :lifetime-error="lifetimeError"
      @retry-today="loadToday"
      @retry-lifetime="loadLifetime"
    />

    <DashboardUsageTrend
      :series="usageTimeSeries"
      :daily-stats="dailyStats"
      :granularity="dailyTimeRange.granularity ?? 'day'"
      :loading="loadingDaily"
      :error="usageTrendError"
      @retry="loadDailyStats"
    >
      <template #controls>
        <TimeRangePicker
          v-model="dailyTimeRange"
          :show-granularity="true"
          :allow-hourly="true"
        />
      </template>
    </DashboardUsageTrend>

  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  dashboardApi,
  type DailyStat,
  type DashboardLifetimeStats,
  type DashboardStatsResponse,
} from '@/api/dashboard'
import { adminApi, type UsageTimeSeriesPoint } from '@/api/admin'
import { TimeRangePicker } from '@/components/common'
import { getDateRangeFromPeriod } from '@/features/usage/composables'
import type { DateRangeParams } from '@/features/usage/types'
import DashboardOverview from './DashboardOverview.vue'
import DashboardUsageTrend from './DashboardUsageTrend.vue'

const today = ref<DashboardStatsResponse | null>(null)
const lifetime = ref<DashboardLifetimeStats | null>(null)
const loadingToday = ref(true)
const loadingLifetime = ref(true)
const todayError = ref(false)
const lifetimeError = ref(false)
let disposed = false

const dailyTimeRange = ref<DateRangeParams>({
  ...getDateRangeFromPeriod('today'), granularity: 'hour',
})
const dailyStats = ref<DailyStat[]>([])
const usageTimeSeries = ref<UsageTimeSeriesPoint[]>([])
const loadingDaily = ref(true)
const usageTrendError = ref(false)
let dailyStatsRequestId = 0
let dailyStatsLoadPromise: Promise<void> | null = null
let hasPendingDailyStatsLoad = false
let dailyStatsDebounceTimer: ReturnType<typeof setTimeout> | null = null

async function loadToday() {
  loadingToday.value = true
  todayError.value = false
  try {
    // The response-time summary follows this range, independently of the trend filters.
    const result = await dashboardApi.getStats(getDateRangeFromPeriod('today'))
    if (!disposed) today.value = result
  } catch {
    if (!disposed) {
      today.value = null
      todayError.value = true
    }
  } finally {
    if (!disposed) loadingToday.value = false
  }
}

async function loadLifetime() {
  loadingLifetime.value = true
  lifetimeError.value = false
  const { timezone, tz_offset_minutes } = getDateRangeFromPeriod('today')
  try {
    const result = await dashboardApi.getLifetimeStats({ timezone, tz_offset_minutes })
    if (!disposed) lifetime.value = result
  } catch {
    if (!disposed) {
      lifetime.value = null
      lifetimeError.value = true
    }
  } finally {
    if (!disposed) loadingLifetime.value = false
  }
}

async function loadDailyStats() {
  if (dailyStatsLoadPromise) {
    hasPendingDailyStatsLoad = true
    return dailyStatsLoadPromise
  }
  const requestId = ++dailyStatsRequestId
  const params = { ...dailyTimeRange.value }
  loadingDaily.value = true
  usageTrendError.value = false
  dailyStatsLoadPromise = (async () => {
    const [dailyResult, seriesResult] = await Promise.allSettled([
      dashboardApi.getDailyStats(params),
      adminApi.getTimeSeries(params),
    ])
    if (requestId !== dailyStatsRequestId) return
    dailyStats.value = dailyResult.status === 'fulfilled' ? dailyResult.value.daily_stats : []
    usageTimeSeries.value = seriesResult.status === 'fulfilled' ? seriesResult.value : []
    usageTrendError.value = dailyResult.status === 'rejected' || seriesResult.status === 'rejected'
  })().catch(() => {
    if (requestId !== dailyStatsRequestId) return
    dailyStats.value = []
    usageTimeSeries.value = []
    usageTrendError.value = true
  }).finally(() => {
    if (requestId === dailyStatsRequestId) loadingDaily.value = false
    dailyStatsLoadPromise = null
    if (hasPendingDailyStatsLoad) {
      hasPendingDailyStatsLoad = false
      void loadDailyStats()
    }
  })
  return dailyStatsLoadPromise
}

function scheduleDailyStatsLoad() {
  // Invalidate old results immediately, including during the debounce wait.
  dailyStatsRequestId += 1
  loadingDaily.value = true
  if (dailyStatsDebounceTimer) clearTimeout(dailyStatsDebounceTimer)
  dailyStatsDebounceTimer = setTimeout(() => {
    dailyStatsDebounceTimer = null
    void loadDailyStats()
  }, 120)
}

watch([
  () => dailyTimeRange.value.preset,
  () => dailyTimeRange.value.start_date,
  () => dailyTimeRange.value.end_date,
  () => dailyTimeRange.value.granularity ?? 'day',
  () => dailyTimeRange.value.timezone,
  () => dailyTimeRange.value.tz_offset_minutes,
], scheduleDailyStatsLoad)

onMounted(() => {
  void Promise.all([loadToday(), loadLifetime(), loadDailyStats()])
})
onBeforeUnmount(() => {
  disposed = true
  if (dailyStatsDebounceTimer) clearTimeout(dailyStatsDebounceTimer)
  dailyStatsDebounceTimer = null
  hasPendingDailyStatsLoad = false
  dailyStatsRequestId += 1
})
</script>
