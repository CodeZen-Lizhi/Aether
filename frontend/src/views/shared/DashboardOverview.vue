<template>
  <section class="min-w-0 space-y-4">
    <Card
      class="dashboard-lifetime p-4 sm:p-5"
      aria-labelledby="lifetime-usage-title"
      :aria-busy="lifetimeLoading"
    >
      <div class="flex flex-col gap-5 lg:flex-row lg:items-center lg:justify-between">
        <div class="min-w-0">
          <div class="mb-2 flex flex-wrap items-center gap-2">
            <Database
              class="h-4 w-4 text-primary"
              aria-hidden="true"
            />
            <h2
              id="lifetime-usage-title"
              class="text-sm font-medium text-muted-foreground"
            >
              累计 Token
            </h2>
            <Badge
              variant="secondary"
              class="text-[10px] font-normal"
            >
              全部历史
            </Badge>
          </div>
          <Skeleton
            v-if="lifetimeLoading"
            class="h-10 w-56 max-w-full"
          />
          <p
            v-else
            class="text-3xl font-semibold tracking-tight tabular-nums text-foreground [overflow-wrap:anywhere] sm:text-4xl"
          >
            {{ lifetime ? lifetime.tokens.toLocaleString(locale) : '--' }}
          </p>
          <p
            v-if="lifetime?.firstActiveDate"
            class="mt-1.5 text-xs text-muted-foreground"
          >
            <span>统计起始</span> · {{ lifetime.firstActiveDate.replaceAll('-', '/') }}
          </p>
          <p
            v-else-if="lifetime && !lifetimeLoading"
            class="mt-1.5 text-xs text-muted-foreground"
          >
            暂无使用数据
          </p>
        </div>

        <div
          v-if="lifetimeError"
          role="status"
          class="flex flex-wrap items-center gap-3 text-sm text-muted-foreground"
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
        <dl
          v-else
          class="grid min-w-0 grid-cols-2 gap-x-6 gap-y-3 border-t border-border pt-4 lg:min-w-80 lg:border-l lg:border-t-0 lg:pl-6 lg:pt-0"
        >
          <div class="min-w-0">
            <dt class="text-xs text-muted-foreground">
              累计请求
            </dt>
            <dd class="mt-1.5 text-xl font-semibold tabular-nums [overflow-wrap:anywhere]">
              <Skeleton
                v-if="lifetimeLoading"
                class="h-7 w-20"
              />
              <template v-else>
                {{ lifetime ? lifetime.requests.toLocaleString(locale) : '--' }}
              </template>
            </dd>
          </div>
          <div class="min-w-0">
            <dt class="text-xs text-muted-foreground">
              累计费用
            </dt>
            <dd class="mt-1.5 text-xl font-semibold tabular-nums text-primary [overflow-wrap:anywhere]">
              <Skeleton
                v-if="lifetimeLoading"
                class="h-7 w-24"
              />
              <template v-else>
                {{ lifetime ? formatCurrency(lifetime.cost) : '--' }}
              </template>
            </dd>
            <dd
              v-if="lifetime && !lifetimeLoading"
              class="mt-1 text-xs tabular-nums text-muted-foreground [overflow-wrap:anywhere]"
            >
              <span>实际结算</span> {{ formatCurrency(lifetime.actualCost) }}
            </dd>
          </div>
        </dl>
      </div>
    </Card>

    <div class="flex flex-wrap items-center justify-between gap-2">
      <h2 class="text-sm font-semibold text-foreground">
        今日概览
      </h2>
      <div
        v-if="todayError"
        role="status"
        class="flex items-center gap-2 text-xs text-muted-foreground"
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

    <div class="grid min-w-0 grid-cols-2 gap-3 md:grid-cols-6 xl:grid-cols-5">
      <Card
        v-for="(stat, index) in cards"
        :key="stat.name"
        role="group"
        :aria-label="stat.name"
        :aria-busy="todayLoading"
        class="min-w-0 p-3.5 sm:p-4 xl:col-span-1"
        :class="index >= 3 ? 'md:col-span-3' : 'md:col-span-2'"
      >
        <div class="flex items-center justify-between gap-2">
          <h3 class="min-w-0 text-xs font-medium text-muted-foreground [overflow-wrap:anywhere]">
            {{ stat.name }}
          </h3>
          <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-border bg-muted text-muted-foreground">
            <component
              :is="stat.icon"
              class="h-4 w-4"
              aria-hidden="true"
            />
          </div>
        </div>
        <template v-if="todayLoading">
          <Skeleton class="mt-3 h-8 w-28 max-w-full" />
          <Skeleton class="mt-2 h-3 w-36 max-w-full" />
        </template>
        <template v-else>
          <p
            class="mt-2 text-2xl font-semibold tracking-tight tabular-nums [overflow-wrap:anywhere]"
            :class="stat.isCost ? 'text-primary' : 'text-foreground'"
          >
            {{ stat.value }}
          </p>
          <p
            v-if="stat.subValue"
            class="mt-2 text-xs leading-relaxed text-muted-foreground [overflow-wrap:anywhere]"
          >
            <span v-if="stat.isCost">实际结算 </span>{{ stat.subValue }}
          </p>
          <div
            v-if="stat.change || stat.extraBadge"
            class="mt-2 flex flex-wrap gap-1.5"
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
.dashboard-lifetime {
  border-color: color-mix(in srgb, var(--primary) 18%, var(--border));
  background: linear-gradient(120deg, color-mix(in srgb, var(--primary) 5%, var(--card)), var(--card) 65%);
}
</style>
