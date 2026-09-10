<template>
  <section
    class="min-w-0 space-y-3"
    aria-labelledby="usage-breakdown-title"
  >
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <h2
          id="usage-breakdown-title"
          class="text-sm font-semibold"
        >
          用量分布
        </h2>
        <p class="mt-1 text-xs text-muted-foreground">
          按所选统计周期汇总
        </p>
      </div>
      <div
        role="group"
        aria-label="排行排序"
        class="inline-flex flex-wrap gap-1 rounded-lg border border-border bg-muted p-1"
      >
        <button
          v-for="option in metricOptions"
          :key="option.key"
          type="button"
          :aria-pressed="metric === option.key"
          class="min-h-8 rounded-md px-3 py-1 text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          :class="metric === option.key ? 'bg-card font-medium text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground'"
          @click="metric = option.key"
        >
          {{ option.label }}
        </button>
      </div>
    </div>

    <div
      v-if="error"
      role="status"
      class="flex flex-wrap items-center justify-center gap-3 rounded-2xl border border-border p-6 text-sm text-muted-foreground"
    >
      <span>用量分布加载失败</span>
      <Button
        variant="outline"
        size="sm"
        @click="$emit('retry')"
      >
        重试
      </Button>
    </div>
    <div
      v-else
      class="grid min-w-0 grid-cols-1 gap-4 xl:grid-cols-2"
    >
      <Card
        v-for="group in groups"
        :key="group.key"
        class="min-w-0 p-4 sm:p-5"
        :aria-label="group.title"
        :aria-busy="loading"
      >
        <div class="mb-4 flex items-center justify-between gap-3">
          <h3 class="text-sm font-medium">
            {{ group.title }}
          </h3>
          <span
            v-if="!loading"
            class="text-xs tabular-nums text-muted-foreground"
          >{{ group.entries.length }}</span>
        </div>
        <div
          v-if="loading"
          class="space-y-4"
        >
          <div
            v-for="index in 3"
            :key="index"
            class="space-y-2"
          >
            <Skeleton class="h-4 w-32" />
            <Skeleton class="h-1.5 w-full" />
            <Skeleton class="h-3 w-48 max-w-full" />
          </div>
        </div>
        <template v-else>
          <p
            v-if="!group.entries.length"
            class="py-7 text-center text-sm text-muted-foreground"
          >
            {{ group.missingRequests > 0 ? '该周期未保留分组明细' : '所选时间内暂无使用数据' }}
          </p>
          <ol
            v-else
            class="space-y-4"
          >
            <li
              v-for="row in visibleEntries(group)"
              :key="row.name"
              class="min-w-0"
            >
              <div class="flex items-start justify-between gap-3 text-sm">
                <!-- Business identifiers must bypass legacy i18n. -->
                <samp class="min-w-0 font-sans font-medium leading-relaxed [overflow-wrap:anywhere]">{{ row.name }}</samp>
                <span class="shrink-0 text-right font-semibold tabular-nums text-foreground">
                  {{ formatMetric(row[metric], metric) }}<span
                    v-if="metric === 'tokens'"
                    class="ml-1 text-[10px] font-normal text-muted-foreground"
                  >Token</span>
                </span>
              </div>
              <div
                class="mt-2 h-1.5 overflow-hidden rounded-full bg-muted"
                aria-hidden="true"
              >
                <div
                  class="h-full rounded-full bg-primary opacity-70"
                  :style="{ width: `${share(row, group)}%` }"
                />
              </div>
              <dl class="mt-1.5 flex flex-wrap items-center justify-between gap-x-3 gap-y-1 text-xs text-muted-foreground">
                <div
                  v-for="field in secondaryMetrics"
                  :key="field.key"
                  class="flex items-center gap-1.5"
                >
                  <dt>{{ field.label }}</dt>
                  <dd class="tabular-nums">
                    {{ formatMetric(row[field.key], field.key) }}
                  </dd>
                </div>
              </dl>
            </li>
          </ol>
          <div
            v-if="group.missingRequests > 0 || group.missingCost > 0"
            role="status"
            class="mt-4 rounded-lg border border-dashed border-border bg-muted px-3 py-2 text-xs text-muted-foreground"
          >
            <p>{{ group.missingLabel }}</p>
            <div class="mt-1 flex flex-wrap justify-between gap-2 tabular-nums">
              <span><span>请求</span> {{ group.missingRequests.toLocaleString(locale) }}</span>
              <span><span>费用</span> {{ formatCurrency(group.missingCost) }}</span>
            </div>
          </div>
          <Button
            v-if="group.entries.length > 5"
            variant="ghost"
            size="sm"
            class="mt-3 w-full text-xs"
            :aria-expanded="expanded.has(group.key)"
            @click="toggleGroup(group.key)"
          >
            {{ expanded.has(group.key) ? '收起列表' : '展开全部' }}
          </Button>
        </template>
      </Card>
    </div>
    <p
      v-if="!loading && !error && hasMissingDetails"
      class="text-xs leading-relaxed text-muted-foreground"
    >
      部分历史记录未保留模型或提供商明细，总请求和总费用已保留，缺失部分单独标注。
    </p>
  </section>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { DailyStat, ModelSummary, ProviderSummary } from '@/api/dashboard'
import { Button, Card, Skeleton } from '@/components/ui'
import { useI18n } from '@/i18n'
import { formatCurrency, formatTokens } from '@/utils/format'

const props = defineProps<{
  dailyStats: DailyStat[]
  models: ModelSummary[]
  providers: ProviderSummary[]
  loading: boolean
  error: boolean
}>()
defineEmits<{ retry: [] }>()
type Metric = 'tokens' | 'cost' | 'requests'
interface RankingEntry { name: string; tokens: number; cost: number; requests: number }
interface RankingGroup {
  key: string; title: string; entries: RankingEntry[]
  missingRequests: number; missingCost: number; missingLabel: string; totalCost: number
}
const { locale } = useI18n()
const metric = ref<Metric>('tokens')
const expanded = ref(new Set<string>())
const metricOptions: Array<{ key: Metric; label: string }> = [
  { key: 'tokens', label: 'Token' }, { key: 'cost', label: '费用' }, { key: 'requests', label: '请求' },
]
const secondaryMetrics = computed(() => metricOptions.filter(option => option.key !== metric.value))
const totals = computed(() => props.dailyStats.reduce((sum, day) => ({
  tokens: sum.tokens + day.tokens, cost: sum.cost + day.cost, requests: sum.requests + day.requests,
}), { tokens: 0, cost: 0, requests: 0 }))
const groups = computed<RankingGroup[]>(() => {
  const retainedMissing = props.dailyStats.every(day => day.unattributed_requests !== undefined && day.unattributed_cost !== undefined)
    ? props.dailyStats.reduce((sum, day) => ({
      requests: sum.requests + (day.unattributed_requests ?? 0),
      cost: sum.cost + (day.unattributed_cost ?? 0),
    }), { requests: 0, cost: 0 })
    : null
  return [
    { key: 'models', title: '模型用量', entries: props.models.map(row => ({ ...row, name: row.model })), missingLabel: '未保留模型明细' },
    { key: 'providers', title: '提供商用量', entries: props.providers.map(row => ({ ...row, name: row.provider })), missingLabel: '未保留提供商明细' },
  ].map(group => {
    const missingRequests = Math.max(0, totals.value.requests - group.entries.reduce((sum, row) => sum + row.requests, 0))
    const attributedCost = group.entries.reduce((sum, row) => sum + row.cost, 0)
    // Daily totals and group costs are rounded separately; their difference is not a history gap.
    const retainedCost = retainedMissing && missingRequests === retainedMissing.requests
      ? retainedMissing.cost
      : totals.value.cost - attributedCost
    const missingCost = Math.max(0, Number(retainedCost.toFixed(4)))
    return {
      ...group,
      entries: group.entries.sort((left, right) => right[metric.value] - left[metric.value] || left.name.localeCompare(right.name)),
      missingRequests,
      missingCost,
      totalCost: attributedCost + missingCost,
    }
  })
})
const hasMissingDetails = computed(() => groups.value.some(group => group.missingRequests > 0 || group.missingCost > 0.000001))
function visibleEntries(group: RankingGroup): RankingEntry[] {
  return expanded.value.has(group.key) ? group.entries : group.entries.slice(0, 5)
}
function toggleGroup(key: string) {
  const next = new Set(expanded.value)
  if (next.has(key)) next.delete(key)
  else next.add(key)
  expanded.value = next
}
function share(row: RankingEntry, group: RankingGroup): number {
  const total = metric.value === 'cost' ? group.totalCost : totals.value[metric.value]
  return total > 0 ? Math.max(0, Math.min(100, row[metric.value] / total * 100)) : 0
}
function formatMetric(value: number, field: Metric): string {
  if (field === 'cost') return formatCurrency(value)
  if (field === 'tokens') return formatTokens(value)
  return value.toLocaleString(locale.value)
}
</script>
