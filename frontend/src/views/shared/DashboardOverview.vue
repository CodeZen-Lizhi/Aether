<template>
  <section class="dashboard-overview min-w-0 space-y-3">
    <div
      data-dashboard-section="today"
      class="min-w-0 space-y-2"
    >
      <div class="flex min-h-8 flex-wrap items-center justify-between gap-2">
        <h2 class="text-sm font-semibold text-foreground">
          今日概览
        </h2>
        <div
          v-if="todayError"
          role="status"
          class="flex flex-wrap items-center justify-end gap-2 text-xs text-muted-foreground"
        >
          <span>今日统计加载失败</span>
          <Button
            variant="outline"
            size="sm"
            @click="$emit('retryToday')"
          >
            重试今日统计
          </Button>
        </div>
        <span
          v-else
          class="text-xs text-muted-foreground"
        >{{ todayLabel }}</span>
      </div>

      <div class="dashboard-today-grid grid min-w-0 grid-cols-2 gap-2">
        <Card
          v-for="stat in cards"
          :key="stat.name"
          role="group"
          :aria-label="stat.name"
          :aria-busy="todayLoading"
          class="dashboard-today-card min-w-0 p-3"
        >
          <div class="flex min-h-8 items-start gap-2">
            <div class="flex h-6 w-6 shrink-0 items-center justify-center rounded-md border border-border bg-muted text-muted-foreground">
              <component
                :is="stat.icon"
                class="h-3.5 w-3.5"
                aria-hidden="true"
              />
            </div>
            <h3 class="min-w-0 pt-0.5 text-xs font-medium leading-4 text-muted-foreground [overflow-wrap:anywhere]">
              {{ stat.name }}
            </h3>
          </div>
          <template v-if="todayLoading">
            <Skeleton class="mt-1.5 h-7 w-24 max-w-full" />
            <Skeleton class="mt-1.5 h-3 w-28 max-w-full" />
          </template>
          <template v-else>
            <p
              class="mt-1.5 text-xl font-semibold leading-7 tabular-nums [overflow-wrap:anywhere]"
              :class="stat.isCost ? 'text-primary' : 'text-foreground'"
            >
              {{ stat.value }}
            </p>
            <p
              v-if="stat.subValue"
              class="mt-1 text-xs leading-4 text-muted-foreground [overflow-wrap:anywhere]"
            >
              <span v-if="stat.isCost">实际结算 </span>{{ stat.subValue }}
            </p>
            <div
              v-if="stat.change || stat.extraBadge"
              class="mt-1.5 flex flex-wrap gap-1"
            >
              <Badge
                v-if="stat.change"
                variant="secondary"
                class="text-[10px] font-normal"
              >
                {{ stat.change }}
              </Badge>
              <Badge
                v-if="stat.extraBadge"
                variant="secondary"
                class="text-[10px] font-normal"
              >
                {{ stat.extraBadge }}
              </Badge>
            </div>
          </template>
        </Card>
      </div>
    </div>

    <Card
      data-dashboard-section="lifetime"
      class="dashboard-lifetime p-3 sm:p-4"
      aria-labelledby="lifetime-usage-title"
      :aria-busy="lifetimeLoading"
    >
      <dl class="dashboard-lifetime-grid grid min-w-0 grid-cols-2 gap-x-4 gap-y-3">
        <div class="min-w-0">
          <dt class="flex min-h-5 flex-wrap items-center gap-1.5">
            <Database
              class="h-3.5 w-3.5 shrink-0 text-primary"
              aria-hidden="true"
            />
            <h2
              id="lifetime-usage-title"
              class="text-xs font-medium text-muted-foreground"
            >
              累计 Token
            </h2>
            <Badge
              variant="secondary"
              class="text-[10px] font-normal"
            >
              全部历史
            </Badge>
          </dt>
          <dd class="mt-1 text-xl font-semibold leading-7 tabular-nums text-foreground [overflow-wrap:anywhere]">
            <Skeleton
              v-if="lifetimeLoading"
              class="h-7 w-32 max-w-full"
            />
            <template v-else>
              {{ lifetime ? lifetime.tokens.toLocaleString(locale) : '--' }}
            </template>
          </dd>
          <dd
            v-if="lifetime?.firstActiveDate"
            class="mt-1 text-xs leading-4 text-muted-foreground [overflow-wrap:anywhere]"
          >
            <span>统计起始</span> · {{ lifetime.firstActiveDate.replaceAll('-', '/') }}
          </dd>
          <dd
            v-else-if="lifetime && !lifetimeLoading"
            class="mt-1 text-xs leading-4 text-muted-foreground"
          >
            暂无使用数据
          </dd>
        </div>

        <div
          v-if="lifetimeError"
          role="status"
          class="col-span-2 flex min-w-0 flex-wrap items-center gap-2 text-sm text-muted-foreground"
        >
          <span>累计统计加载失败</span>
          <Button
            variant="outline"
            size="sm"
            @click="$emit('retryLifetime')"
          >
            重试累计统计
          </Button>
        </div>
        <template v-else>
          <div class="min-w-0">
            <dt class="min-h-5 text-xs font-medium leading-5 text-muted-foreground">
              累计请求
            </dt>
            <dd class="mt-1 text-xl font-semibold leading-7 tabular-nums text-foreground [overflow-wrap:anywhere]">
              <Skeleton
                v-if="lifetimeLoading"
                class="h-7 w-20 max-w-full"
              />
              <template v-else>
                {{ lifetime ? lifetime.requests.toLocaleString(locale) : '--' }}
              </template>
            </dd>
          </div>
          <div class="dashboard-lifetime-cost col-span-2 min-w-0">
            <dt class="min-h-5 text-xs font-medium leading-5 text-muted-foreground">
              累计费用
            </dt>
            <dd class="mt-1 text-xl font-semibold leading-7 tabular-nums text-primary [overflow-wrap:anywhere]">
              <Skeleton
                v-if="lifetimeLoading"
                class="h-7 w-24 max-w-full"
              />
              <template v-else>
                {{ lifetime ? formatCurrency(lifetime.cost) : '--' }}
              </template>
            </dd>
            <dd
              v-if="lifetime && !lifetimeLoading"
              class="mt-1 text-xs leading-4 tabular-nums text-muted-foreground [overflow-wrap:anywhere]"
            >
              <span>实际结算</span> {{ formatCurrency(lifetime.actualCost) }}
            </dd>
          </div>
        </template>
      </dl>
    </Card>
  </section>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { Component } from 'vue'
import { Activity, Clock, Database, DollarSign, Hash, Key, TrendingUp, Zap } from 'lucide-vue-next'
import type { DashboardLifetimeStats, DashboardStat, DashboardStatsResponse } from '@/api/dashboard'
import { Badge, Button, Card, Skeleton } from '@/components/ui'
import { useI18n } from '@/i18n'
import { formatCurrency } from '@/utils/format'

const props = defineProps<{
  today: DashboardStatsResponse | null
  todayLoading: boolean
  todayError: boolean
  lifetime: DashboardLifetimeStats | null
  lifetimeLoading: boolean
  lifetimeError: boolean
}>()
defineEmits<{ retryToday: []; retryLifetime: [] }>()
const { locale } = useI18n()
const todayLabel = computed(() => new Date().toLocaleDateString(locale.value, {
  year: 'numeric', month: 'long', day: 'numeric', weekday: 'short',
}))
type StatCard = Omit<DashboardStat, 'icon'> & { icon: Component; isCost?: boolean }
const iconMap: Record<string, Component> = { Activity, Clock, Database, DollarSign, Hash, Key, TrendingUp, Zap }
const placeholders = [
  { name: '今日请求', icon: Activity },
  { name: '今日 Token', icon: Zap },
  { name: '今日费用', icon: DollarSign },
  { name: '全站 RPM / TPM', icon: Activity },
  { name: '今日平均响应', icon: Clock },
]
const cards = computed<StatCard[]>(() => {
  if (!props.today?.stats.length) return placeholders.map(stat => ({ ...stat, value: '--' }))
  const result: StatCard[] = props.today.stats.map(stat => ({
    ...stat,
    icon: iconMap[stat.icon] ?? Activity,
    isCost: stat.name === '今日费用',
  }))
  const health = props.today.system_health
  const responseTime = health?.avg_response_time
  const hasResponseTime = (health?.total_requests ?? 0) > 0
    && typeof responseTime === 'number' && Number.isFinite(responseTime) && responseTime >= 0
  result.push({
    name: '今日平均响应', icon: Clock,
    value: hasResponseTime ? `${responseTime.toFixed(2)}s` : '--',
    subValue: health?.total_requests === 0
      ? '今日暂无请求'
      : hasResponseTime ? '今日请求的平均耗时' : '暂无响应时间数据',
  })
  return result
})
</script>

<style scoped>
.dashboard-overview {
  container-name: dashboard-overview;
  container-type: inline-size;
}

.dashboard-today-card:last-child,
.dashboard-lifetime-cost {
  grid-column: span 2 / span 2;
}

@container dashboard-overview (min-width: 36rem) {
  .dashboard-today-grid {
    grid-template-columns: repeat(6, minmax(0, 1fr));
  }

  .dashboard-today-card:nth-child(-n + 3) {
    grid-column: span 2 / span 2;
  }

  .dashboard-today-card:nth-child(n + 4) {
    grid-column: span 3 / span 3;
  }

  .dashboard-lifetime-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
    column-gap: 1.5rem;
  }

  .dashboard-lifetime-cost {
    grid-column: span 1 / span 1;
  }
}

@container dashboard-overview (min-width: 56rem) {
  .dashboard-today-grid {
    grid-template-columns: repeat(5, minmax(0, 1fr));
  }

  .dashboard-today-card:nth-child(n) {
    grid-column: span 1 / span 1;
  }
}

.dashboard-lifetime {
  border-color: color-mix(in srgb, var(--primary) 18%, var(--border));
  background: linear-gradient(120deg, color-mix(in srgb, var(--primary) 5%, var(--card)), var(--card) 65%);
}
</style>
