<template>
  <div class="space-y-6 px-4 sm:px-6 lg:px-0">
    <!-- 页面头部：统计卡片 + 公告 -->
    <div class="flex flex-col lg:flex-row gap-6 lg:items-start">
      <!-- 左侧统计区域 -->
      <div
        ref="statsPanelRef"
        class="flex-1 min-w-0 flex flex-col"
      >
        <Badge
          variant="default"
          class="mb-4 self-start uppercase tracking-[0.45em]"
        >
          {{ dashboardModeLabel }}
        </Badge>

        <!-- 主要统计卡片 -->
        <div class="grid grid-cols-2 gap-3 sm:gap-4 xl:grid-cols-4">
          <!-- 加载中骨架屏 -->
          <template v-if="loading">
            <Card
              v-for="i in statSkeletonCount"
              :key="'skeleton-' + i"
              class="p-5"
            >
              <Skeleton class="h-4 w-20 mb-4" />
              <Skeleton class="h-8 w-32 mb-2" />
              <Skeleton class="h-4 w-16" />
            </Card>
          </template>
          <!-- 有数据时显示统计卡片 -->
          <template v-else-if="stats.length > 0">
            <Card
              v-for="(stat, index) in stats"
              :key="stat.name"
              class="relative overflow-hidden p-3 sm:p-5"
              :class="statCardBorders[index % statCardBorders.length]"
            >
              <div
                class="pointer-events-none absolute -right-4 -top-6 h-28 w-28 rounded-full blur-3xl opacity-40"
                :class="statCardGlows[index % statCardGlows.length]"
              />
              <!-- 图标固定在右上角 -->
              <div
                class="absolute top-3 right-3 sm:top-5 sm:right-5 rounded-xl sm:rounded-2xl border border-border bg-card/50 p-2 sm:p-3 shadow-inner backdrop-blur-sm"
                :class="getStatIconColor(index)"
              >
                <component
                  :is="stat.icon"
                  class="h-4 w-4 sm:h-5 sm:w-5"
                />
              </div>
              <!-- 内容区域 -->
              <div>
                <p
                  class="text-[9px] sm:text-[11px] font-semibold uppercase tracking-[0.2em] sm:tracking-[0.4em] text-muted-foreground pr-10 sm:pr-14"
                >
                  {{ stat.name }}
                </p>
                <p
                  class="mt-2 sm:mt-4 text-xl sm:text-3xl font-semibold"
                  :class="stat.isCost ? 'text-primary' : 'text-foreground'"
                >
                  {{ stat.value }}
                </p>
                <p
                  v-if="stat.subValue"
                  class="mt-0.5 sm:mt-1 text-[10px] sm:text-sm text-muted-foreground"
                >
                  {{ stat.subValue }}
                </p>
                <div
                  v-if="stat.change || stat.extraBadge"
                  class="mt-1.5 sm:mt-2 flex items-center gap-1 sm:gap-1.5 flex-wrap"
                >
                  <Badge
                    v-if="stat.change"
                    variant="secondary"
                    class="text-[9px] sm:text-xs"
                  >
                    {{ stat.change }}
                  </Badge>
                  <Badge
                    v-if="stat.extraBadge"
                    variant="secondary"
                    class="text-[9px] sm:text-xs"
                  >
                    {{ stat.extraBadge }}
                  </Badge>
                </div>
              </div>
            </Card>
          </template>
          <!-- 无数据时显示占位卡片 -->
          <template v-else>
            <Card
              v-for="(placeholder, index) in emptyStatPlaceholders"
              :key="'empty-' + index"
              class="relative overflow-hidden p-3 sm:p-5"
              :class="statCardBorders[index % statCardBorders.length]"
            >
              <div
                class="pointer-events-none absolute -right-4 -top-6 h-28 w-28 rounded-full blur-3xl opacity-20"
                :class="statCardGlows[index % statCardGlows.length]"
              />
              <div
                class="absolute top-3 right-3 sm:top-5 sm:right-5 rounded-xl sm:rounded-2xl border border-border bg-card/50 p-2 sm:p-3 shadow-inner backdrop-blur-sm"
                :class="getStatIconColor(index)"
              >
                <component
                  :is="placeholder.icon"
                  class="h-4 w-4 sm:h-5 sm:w-5"
                />
              </div>
              <div>
                <p
                  class="text-[9px] sm:text-[11px] font-semibold uppercase tracking-[0.2em] sm:tracking-[0.4em] text-muted-foreground pr-10 sm:pr-14"
                >
                  {{ placeholder.name }}
                </p>
                <p
                  class="mt-2 sm:mt-4 text-xl sm:text-3xl font-semibold text-muted-foreground/50"
                >
                  --
                </p>
                <p
                  class="mt-0.5 sm:mt-1 text-[10px] sm:text-sm text-muted-foreground/50"
                >
                  暂无数据
                </p>
              </div>
            </Card>
          </template>
        </div>

        <!-- 系统健康摘要 -->
        <div
          v-if="systemHealth"
          class="mt-6"
        >
          <div class="mb-3 flex items-center justify-between">
            <h3 class="text-sm font-medium text-foreground">
              本月系统健康
            </h3>
            <Badge
              variant="outline"
              class="uppercase tracking-[0.3em] text-[10px]"
            >
              Monthly
            </Badge>
          </div>
          <div class="grid grid-cols-2 gap-2 sm:gap-3 xl:grid-cols-4">
            <Card class="relative p-3 sm:p-4 border-book-cloth/30">
              <Clock
                class="absolute top-3 right-3 h-3.5 w-3.5 sm:h-4 sm:w-4 text-muted-foreground"
              />
              <div class="pr-6">
                <p
                  class="text-[9px] sm:text-[10px] font-semibold uppercase tracking-[0.2em] sm:tracking-[0.3em] text-muted-foreground"
                >
                  平均响应
                </p>
                <p
                  class="mt-1.5 sm:mt-2 text-lg sm:text-xl font-semibold text-foreground"
                >
                  {{ systemHealth.avg_response_time }}s
                </p>
              </div>
            </Card>
            <Card class="relative p-3 sm:p-4 border-kraft/30">
              <AlertTriangle
                class="absolute top-3 right-3 h-3.5 w-3.5 sm:h-4 sm:w-4 text-muted-foreground"
              />
              <div class="pr-6">
                <p
                  class="text-[9px] sm:text-[10px] font-semibold uppercase tracking-[0.2em] sm:tracking-[0.3em] text-muted-foreground"
                >
                  错误率
                </p>
                <p
                  class="mt-1.5 sm:mt-2 text-lg sm:text-xl font-semibold"
                  :class="
                    systemHealth.error_rate > 5
                      ? 'text-destructive'
                      : 'text-foreground'
                  "
                >
                  {{ systemHealth.error_rate }}%
                </p>
              </div>
            </Card>
            <Card class="relative p-3 sm:p-4 border-book-cloth/25">
              <Shuffle
                class="absolute top-3 right-3 h-3.5 w-3.5 sm:h-4 sm:w-4 text-muted-foreground"
              />
              <div class="pr-6">
                <p
                  class="text-[9px] sm:text-[10px] font-semibold uppercase tracking-[0.2em] sm:tracking-[0.3em] text-muted-foreground"
                >
                  转移次数
                </p>
                <p
                  class="mt-1.5 sm:mt-2 text-lg sm:text-xl font-semibold text-foreground"
                >
                  {{ systemHealth.fallback_count }}
                </p>
              </div>
            </Card>
            <Card
              v-if="costStats"
              class="relative p-3 sm:p-4 border-manilla/40"
            >
              <DollarSign
                class="absolute top-3 right-3 h-3.5 w-3.5 sm:h-4 sm:w-4 text-muted-foreground"
              />
              <div class="pr-6">
                <p
                  class="text-[9px] sm:text-[10px] font-semibold uppercase tracking-[0.2em] sm:tracking-[0.3em] text-muted-foreground"
                >
                  本月费用
                </p>
                <p
                  class="mt-1.5 sm:mt-2 text-lg sm:text-xl font-semibold text-primary"
                >
                  {{ formatCurrency(costStats.total_cost) }}
                </p>
                <p
                  class="mt-0.5 text-[10px] sm:text-xs text-muted-foreground tabular-nums"
                >
                  {{ formatCurrency(costStats.total_actual_cost) }}
                </p>
                <Badge
                  v-if="costStats.cost_savings > 0"
                  variant="success"
                  class="mt-1 text-[9px] sm:text-[10px]"
                >
                  节省 {{ formatCurrency(costStats.cost_savings) }}
                </Badge>
              </div>
            </Card>
          </div>
        </div>
      </div>
    </div>

    <!-- 趋势图表筛选 -->
    <div class="flex flex-wrap items-center justify-between gap-3">
      <h3
        class="text-xs font-semibold uppercase tracking-wider text-muted-foreground"
      >
        统计周期
      </h3>
      <TimeRangePicker
        v-model="dailyTimeRange"
        :allow-hourly="true"
      />
    </div>

    <!-- 趋势图表区域 -->
    <div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
      <!-- 每日模型成本（堆叠柱状图） -->
      <Card class="p-5">
        <h4
          class="mb-3 text-xs font-semibold text-foreground uppercase tracking-wider"
        >
          每日模型成本
        </h4>
        <div
          v-if="loadingDaily"
          class="flex items-center justify-center h-[280px]"
        >
          <Skeleton class="h-full w-full" />
        </div>
        <div
          v-else
          style="height: 280px"
        >
          <BarChart
            v-if="
              dailyModelCostChartData.labels &&
                dailyModelCostChartData.labels.length > 0
            "
            :data="dailyModelCostChartData"
            :options="dailyModelCostChartOptions"
          />
          <div
            v-else
            class="flex h-full items-center justify-center text-xs text-muted-foreground"
          >
            暂无数据
          </div>
        </div>
      </Card>

      <!-- 提供商成本分布（环形图） -->
      <Card class="p-5">
        <h4
          class="mb-3 text-xs font-semibold text-foreground uppercase tracking-wider"
        >
          提供商成本分布
        </h4>
        <div
          v-if="loadingDaily"
          class="flex items-center justify-center h-[280px]"
        >
          <Skeleton class="h-full w-full" />
        </div>
        <div
          v-else
          style="height: 280px"
        >
          <DoughnutChart
            v-if="
              providerCostChartData.labels &&
                providerCostChartData.labels.length > 0
            "
            :data="providerCostChartData"
            :options="providerCostChartOptions"
          />
          <div
            v-else
            class="flex h-full items-center justify-center text-xs text-muted-foreground"
          >
            暂无数据
          </div>
        </div>
      </Card>
    </div>

    <!-- 每日统计 -->
    <Card class="overflow-hidden mt-6">
      <!-- 移动端：卡片列表 -->
      <div class="sm:hidden">
        <div class="px-4 py-3 border-b border-border/60">
          <h3 class="text-sm font-semibold">
            每日统计
          </h3>
        </div>
        <div
          v-if="loadingDaily"
          class="flex items-center justify-center py-8"
        >
          <Skeleton class="h-5 w-5 rounded-full" />
          <span class="ml-2 text-muted-foreground text-xs">加载中...</span>
        </div>
        <div
          v-else-if="dailyStats.length === 0"
          class="py-8 text-center text-muted-foreground text-xs"
        >
          暂无数据
        </div>
        <div
          v-else
          class="divide-y divide-border/60"
        >
          <div
            v-for="stat in dailyStats.slice().reverse()"
            :key="stat.date"
            class="p-4 space-y-2"
          >
            <div class="flex items-center justify-between">
              <span class="font-medium text-sm">{{
                formatDate(stat.date)
              }}</span>
              <div
                class="flex flex-col items-end gap-0.5 text-[10px] leading-tight tabular-nums"
              >
                <span class="font-semibold text-primary">
                  {{ formatDashboardCost(stat.cost) }}
                </span>
                <span class="text-muted-foreground">
                  {{ formatDashboardCost(stat.actual_cost) }}
                </span>
              </div>
            </div>
            <div class="grid grid-cols-2 gap-2 text-xs">
              <div class="flex justify-between">
                <span class="text-muted-foreground">请求</span>
                <span>{{ stat.requests.toLocaleString() }}</span>
              </div>
              <div class="flex justify-between">
                <span class="text-muted-foreground">Tokens</span>
                <span>{{ formatTokens(stat.tokens) }}</span>
              </div>
              <div class="flex justify-between">
                <span class="text-muted-foreground">响应</span>
                <span>{{ formatResponseTime(stat.avg_response_time) }}</span>
              </div>
              <div class="flex justify-between">
                <span class="text-muted-foreground">模型</span>
                <span>{{ stat.unique_models }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 桌面端：表格 -->
      <Table class="hidden sm:table">
        <TableHeader>
          <TableRow>
            <TableHead class="text-left">
              日期
            </TableHead>
            <TableHead class="text-center">
              请求次数
            </TableHead>
            <TableHead class="text-center">
              Tokens
            </TableHead>
            <TableHead class="text-center">
              费用
            </TableHead>
            <TableHead class="text-center">
              平均响应
            </TableHead>
            <TableHead class="text-center">
              使用模型
            </TableHead>
            <TableHead class="text-center">
              使用提供商
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-if="loadingDaily">
            <TableCell
              colspan="7"
              class="text-center py-8"
            >
              <div class="flex items-center justify-center gap-2">
                <Skeleton class="h-5 w-5 rounded-full" />
                <span class="text-muted-foreground text-xs">加载中...</span>
              </div>
            </TableCell>
          </TableRow>
          <TableRow v-else-if="dailyStats.length === 0">
            <TableCell
              colspan="7"
              class="text-center py-8 text-muted-foreground text-xs"
            >
              暂无数据
            </TableCell>
          </TableRow>
          <template v-else>
            <TableRow
              v-for="stat in dailyStats.slice().reverse()"
              :key="stat.date"
            >
              <TableCell class="font-medium text-xs">
                {{ formatDate(stat.date) }}
              </TableCell>
              <TableCell class="text-center text-xs">
                {{ stat.requests.toLocaleString() }}
              </TableCell>
              <TableCell class="text-center">
                <Badge
                  variant="secondary"
                  class="text-[10px]"
                >
                  {{ formatTokens(stat.tokens) }}
                </Badge>
              </TableCell>
              <TableCell class="text-center">
                <div
                  class="flex flex-col items-center gap-0.5 text-[10px] leading-tight tabular-nums"
                >
                  <span class="font-semibold text-primary">
                    {{ formatDashboardCost(stat.cost) }}
                  </span>
                  <span class="text-muted-foreground">
                    {{ formatDashboardCost(stat.actual_cost) }}
                  </span>
                </div>
              </TableCell>
              <TableCell class="text-center">
                <Badge
                  variant="outline"
                  class="text-[10px]"
                >
                  {{ formatResponseTime(stat.avg_response_time) }}
                </Badge>
              </TableCell>
              <TableCell class="text-center text-xs">
                {{ stat.unique_models }}
              </TableCell>
              <TableCell class="text-center text-xs">
                {{ stat.unique_providers }}
              </TableCell>
            </TableRow>
          </template>
        </TableBody>
      </Table>

      <!-- 汇总信息 -->
      <div
        v-if="dailyStats.length > 0"
        class="border-t border-border bg-muted/30 backdrop-blur-sm px-4 py-3 text-xs"
      >
        <div class="grid grid-cols-2 gap-4 sm:grid-cols-4">
          <div class="text-center">
            <div class="text-muted-foreground text-[10px]">
              总请求
            </div>
            <div class="font-semibold text-foreground">
              {{ totalStats.requests.toLocaleString() }}
            </div>
          </div>
          <div class="text-center">
            <div class="text-muted-foreground text-[10px]">
              总Tokens
            </div>
            <div class="font-semibold text-book-cloth dark:text-kraft">
              {{ formatTokens(totalStats.tokens) }}
            </div>
          </div>
          <div class="text-center">
            <div class="text-muted-foreground text-[10px]">
              总费用
            </div>
            <div
              class="flex flex-col items-center gap-0.5 leading-tight tabular-nums"
            >
              <span class="font-semibold text-primary">
                {{ formatDashboardCost(totalStats.cost) }}
              </span>
              <span class="text-[10px] text-muted-foreground">
                {{ formatDashboardCost(totalStats.actualCost) }}
              </span>
            </div>
          </div>
          <div class="text-center">
            <div class="text-muted-foreground text-[10px]">
              平均响应
            </div>
            <div class="font-semibold text-book-cloth dark:text-kraft">
              {{ formatResponseTime(totalStats.avgResponseTime) }}
            </div>
          </div>
        </div>
      </div>
    </Card>
  </div>
</template>

<script setup lang="ts">
import {
  ref,
  onMounted,
  computed,
  onBeforeUnmount,
  nextTick,
  watch,
  markRaw,
} from "vue";
import type { Component } from "vue";
import {
  dashboardApi,
  type DashboardStat,
  type DailyStat,
  type ProviderSummary,
} from "@/api/dashboard";
import { getDateRangeFromPeriod } from "@/features/usage/composables";
import type { DateRangeParams } from "@/features/usage/types";
import {
  Card,
  Badge,
  Button,
  Skeleton,
  Dialog,
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from "@/components/ui";
import { TimeRangePicker } from "@/components/common";
import BarChart from "@/components/charts/BarChart.vue";
import DoughnutChart from "@/components/charts/DoughnutChart.vue";
import {
  Activity,
  TrendingUp,
  DollarSign,
  Key,
  Hash,
  Zap,
  Bell,
  AlertCircle,
  AlertTriangle,
  Info,
  Wrench,
  Loader2,
  Clock,
  Database,
  Shuffle,
} from "lucide-vue-next";
import { formatTokens, formatCurrency } from "@/utils/format";
import { parseDateLike } from "@/utils/date";
import { marked } from "marked";
import { sanitizeMarkdown } from "@/utils/sanitize";
import type {
  ChartData,
  ChartOptions,
  ChartDataset,
  TooltipItem,
} from "chart.js";

type DashboardStatCard = Omit<DashboardStat, "icon"> & {
  icon: Component;
  isCost: boolean;
};

const statsPanelRef = ref<HTMLElement | null>(null);
const isLargeScreen = ref(false);

function checkScreenSize() {
  if (typeof window !== "undefined") {
    isLargeScreen.value = window.innerWidth >= 1024; // lg breakpoint
  }
}

let statsPanelObserver: ResizeObserver | null = null;

function handleWindowResize() {
  checkScreenSize();
}

function setupResizeObserver() {
  if (typeof window === "undefined") return;
  const panel = statsPanelRef.value;
  if (!panel || !("ResizeObserver" in window)) return;
  statsPanelObserver = new ResizeObserver(() => {});
  statsPanelObserver.observe(panel);
}

const dashboardModeLabel = computed(() => "ADMIN MODE");

const statCardBorders = [
  "border-book-cloth/30 dark:border-book-cloth/25",
  "border-kraft/30 dark:border-kraft/25",
  "border-manilla/40 dark:border-manilla/30",
  "border-book-cloth/25 dark:border-kraft/25",
];

const statCardGlows = [
  "bg-book-cloth/30",
  "bg-kraft/30",
  "bg-manilla/35",
  "bg-kraft/30",
];

const getStatIconColor = (_index: number): string => {
  return "text-muted-foreground";
};

// 统计数据
const stats = ref<DashboardStatCard[]>([]);
const todayStats = ref<{
  requests: number;
  tokens: number;
  cost: number;
  actual_cost?: number;
  cache_creation_tokens?: number;
  cache_read_tokens?: number;
}>({ requests: 0, tokens: 0, cost: 0 });

const systemHealth = ref<{
  avg_response_time: number;
  error_rate: number;
  error_requests: number;
  fallback_count: number;
  total_requests: number;
} | null>(null);

const costStats = ref<{
  total_cost: number;
  total_actual_cost: number;
  cost_savings: number;
} | null>(null);

const dailyStats = ref<DailyStat[]>([]);
const providerSummary = ref<ProviderSummary[]>([]);
const dailyTimeRange = ref<DateRangeParams>(
  getDateRangeFromPeriod("last7days"),
);
// 统计周期
const loadingDaily = ref(false);
const loading = ref(false);
let dailyStatsRequestId = 0;
let dailyStatsLoadPromise: Promise<void> | null = null;
let hasPendingDailyStatsLoad = false;
let dailyStatsDebounceTimer: ReturnType<typeof setTimeout> | null = null;

// 公告
const detailDialogOpen = ref(false);

const iconMap: Record<string, Component> = {
  Activity,
  TrendingUp,
  DollarSign,
  Key,
  Hash,
  Zap,
  Database,
};

// 空状态占位卡片
const emptyStatPlaceholders = computed(() => {
  return [
    { name: "今日请求", icon: Activity },
    { name: "今日 Tokens", icon: Hash },
    { name: "今日费用", icon: DollarSign },
    { name: "全站 RPM / 全站 TPM", icon: Activity },
  ];
});

const statSkeletonCount = computed(() => emptyStatPlaceholders.value.length);

const totalStats = computed(() => {
  if (dailyStats.value.length === 0) {
    return {
      requests: 0,
      tokens: 0,
      cost: 0,
      actualCost: 0,
      avgResponseTime: 0,
    };
  }
  const totals = dailyStats.value.reduce(
    (acc, stat) => {
      acc.requests += stat.requests;
      acc.tokens += stat.tokens;
      acc.cost += stat.cost;
      acc.actualCost += stat.actual_cost;
      acc.totalResponseTime += stat.avg_response_time * stat.requests;
      return acc;
    },
    {
      requests: 0,
      tokens: 0,
      cost: 0,
      actualCost: 0,
      totalResponseTime: 0,
    },
  );
  return {
    requests: totals.requests,
    tokens: totals.tokens,
    cost: totals.cost,
    actualCost: totals.actualCost,
    avgResponseTime:
      totals.requests > 0 ? totals.totalResponseTime / totals.requests : 0,
  };
});

// 每日模型成本（堆叠柱状图）
const MODEL_COLORS = [
  "rgba(59, 130, 246, 0.8)", // blue
  "rgba(239, 68, 68, 0.8)", // red
  "rgba(16, 185, 129, 0.8)", // green
  "rgba(245, 158, 11, 0.8)", // amber
  "rgba(139, 92, 246, 0.8)", // purple
  "rgba(6, 182, 212, 0.8)", // cyan
  "rgba(132, 204, 22, 0.8)", // lime
  "rgba(249, 115, 22, 0.8)", // orange
];

const dailyModelCostChartData = computed<ChartData<"bar">>(() => {
  if (dailyStats.value.length === 0) {
    return { labels: [], datasets: [] };
  }

  // 收集所有出现过的模型
  const allModels = new Set<string>();
  dailyStats.value.forEach((day) => {
    day.model_breakdown?.forEach((mb) => allModels.add(mb.model));
  });
  const modelList = Array.from(allModels);

  // 按总费用降序排列模型
  const modelTotalCost = new Map<string, number>();
  dailyStats.value.forEach((day) => {
    day.model_breakdown?.forEach((mb) => {
      modelTotalCost.set(
        mb.model,
        (modelTotalCost.get(mb.model) || 0) + mb.cost,
      );
    });
  });
  modelList.sort(
    (a, b) => (modelTotalCost.get(b) || 0) - (modelTotalCost.get(a) || 0),
  );

  // 为每个模型创建一个 dataset
  const datasets: ChartDataset<"bar", number[]>[] = modelList.map(
    (model, index) => ({
      label: model.replace("claude-", "").replace("gpt-", ""),
      data: dailyStats.value.map((day) => {
        const found = day.model_breakdown?.find((mb) => mb.model === model);
        return found ? found.cost : 0;
      }),
      backgroundColor: MODEL_COLORS[index % MODEL_COLORS.length],
      borderRadius: 2,
      stack: "stack0",
      barPercentage: 0.6,
      categoryPercentage: 0.7,
    }),
  );

  return {
    labels: dailyStats.value.map((stat) => formatDateForChart(stat.date)),
    datasets,
  };
});

const dailyModelCostChartOptions = computed<ChartOptions<"bar">>(() => ({
  responsive: true,
  maintainAspectRatio: false,
  interaction: {
    mode: "index",
    intersect: false,
  },
  scales: {
    x: {
      stacked: true,
      ticks: { font: { size: 10 } },
    },
    y: {
      stacked: true,
      title: {
        display: true,
        text: "费用 ($)",
        color: "rgb(107, 114, 128)",
        font: { size: 10 },
      },
      ticks: { font: { size: 10 } },
    },
  },
  plugins: {
    legend: {
      display: true,
      position: "bottom",
      labels: { font: { size: 10 }, boxWidth: 12, padding: 8 },
    },
    tooltip: {
      callbacks: {
        label: (context: TooltipItem<"bar">) => {
          const value = typeof context.raw === "number" ? context.raw : 0;
          if (value === 0) return "";
          return `${context.dataset.label}: $${value.toFixed(4)}`;
        },
        footer: (items: TooltipItem<"bar">[]) => {
          const total = items.reduce((sum, item) => {
            const val = typeof item.raw === "number" ? item.raw : 0;
            return sum + val;
          }, 0);
          return `Total: $${total.toFixed(4)}`;
        },
      },
    },
  },
}));

// 提供商成本分布（环形图）
const PROVIDER_COLORS = [
  "rgba(59, 130, 246, 0.8)", // blue
  "rgba(239, 68, 68, 0.8)", // red
  "rgba(16, 185, 129, 0.8)", // green
  "rgba(245, 158, 11, 0.8)", // amber
  "rgba(139, 92, 246, 0.8)", // purple
  "rgba(6, 182, 212, 0.8)", // cyan
  "rgba(132, 204, 22, 0.8)", // lime
  "rgba(249, 115, 22, 0.8)", // orange
];

const providerCostChartData = computed<ChartData<"doughnut">>(() => {
  if (providerSummary.value.length === 0) {
    return { labels: [], datasets: [] };
  }

  return {
    labels: providerSummary.value.map((p) => p.provider),
    datasets: [
      {
        data: providerSummary.value.map((p) => p.cost),
        backgroundColor: providerSummary.value.map(
          (_, i) => PROVIDER_COLORS[i % PROVIDER_COLORS.length],
        ),
        borderWidth: 2,
        borderColor: "rgba(255, 255, 255, 0.1)",
      },
    ],
  };
});

const providerCostChartOptions = computed<ChartOptions<"doughnut">>(() => ({
  responsive: true,
  maintainAspectRatio: false,
  cutout: "60%",
  plugins: {
    legend: {
      position: "right",
      labels: {
        font: { size: 10 },
        boxWidth: 12,
        padding: 8,
      },
    },
    tooltip: {
      callbacks: {
        label: (context) => {
          const value = context.raw as number;
          const total = (context.dataset.data as number[]).reduce(
            (a, b) => a + b,
            0,
          );
          const percentage =
            total > 0 ? ((value / total) * 100).toFixed(1) : "0";
          return `${context.label}: $${value.toFixed(4)} (${percentage}%)`;
        },
      },
    },
  },
}));

onMounted(async () => {
  checkScreenSize();
  setupResizeObserver();
  if (typeof window !== "undefined") {
    window.addEventListener("resize", handleWindowResize);
  }
  await Promise.all([
    loadDashboardData(),
    loadDailyStats(),
  ]);
  await nextTick();
});

onBeforeUnmount(() => {
  if (typeof window !== "undefined") {
    window.removeEventListener("resize", handleWindowResize);
  }
  if (statsPanelObserver && statsPanelRef.value) {
    statsPanelObserver.unobserve(statsPanelRef.value);
  }
  statsPanelObserver?.disconnect();
  statsPanelObserver = null;
  if (dailyStatsDebounceTimer) {
    clearTimeout(dailyStatsDebounceTimer);
    dailyStatsDebounceTimer = null;
  }
  hasPendingDailyStatsLoad = false;
  dailyStatsLoadPromise = null;
  dailyStatsRequestId += 1;
});

async function loadDashboardData() {
  loading.value = true;
  try {
    const statsData = await dashboardApi.getStats({
      timezone: dailyTimeRange.value.timezone,
      tz_offset_minutes: dailyTimeRange.value.tz_offset_minutes,
    });
    stats.value = statsData.stats.map((stat) => ({
      ...stat,
      icon: markRaw(iconMap[stat.icon] || Activity),
      isCost: stat.name === "今日费用",
    }));
    if (statsData.today) todayStats.value = statsData.today;
    if (statsData.system_health) systemHealth.value = statsData.system_health;
    if (statsData.cost_stats) costStats.value = statsData.cost_stats;
  } finally {
    loading.value = false;
  }
}

function formatDashboardCost(value: number): string {
  return Number.isFinite(value) ? `$${value.toFixed(4)}` : "$0.0000";
}

async function loadDailyStats() {
  if (dailyStatsLoadPromise) {
    hasPendingDailyStatsLoad = true;
    return dailyStatsLoadPromise;
  }
  const requestId = ++dailyStatsRequestId;
  loadingDaily.value = true;
  dailyStatsLoadPromise = (async () => {
    try {
      const response = await dashboardApi.getDailyStats(dailyTimeRange.value);
      if (requestId !== dailyStatsRequestId) return;
      dailyStats.value = response.daily_stats;
      providerSummary.value = response.provider_summary || [];
    } catch {
      if (requestId !== dailyStatsRequestId) return;
      dailyStats.value = [];
      providerSummary.value = [];
    } finally {
      if (requestId === dailyStatsRequestId) {
        loadingDaily.value = false;
      }
    }
  })().finally(() => {
    dailyStatsLoadPromise = null;
    if (hasPendingDailyStatsLoad) {
      hasPendingDailyStatsLoad = false;
      void loadDailyStats();
    }
  });
  return dailyStatsLoadPromise;
}

function scheduleDailyStatsLoad() {
  if (dailyStatsDebounceTimer) {
    clearTimeout(dailyStatsDebounceTimer);
  }
  dailyStatsDebounceTimer = setTimeout(() => {
    dailyStatsDebounceTimer = null;
    void loadDailyStats();
  }, 120);
}

watch(dailyTimeRange, scheduleDailyStatsLoad, { deep: true });

function formatDate(dateString: string): string {
  const date = parseDateLike(dateString);
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);
  if (date.toDateString() === today.toDateString()) return "今天";
  if (date.toDateString() === yesterday.toDateString()) return "昨天";
  return date.toLocaleDateString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    weekday: "short",
  });
}

function formatDateForChart(dateString: string): string {
  const date = parseDateLike(dateString);
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);
  if (date.toDateString() === today.toDateString()) return "今天";
  if (date.toDateString() === yesterday.toDateString()) return "昨天";
  return date.toLocaleDateString("zh-CN", { month: "numeric", day: "numeric" });
}

function formatResponseTime(seconds: number): string {
  if (seconds === 0) return "-";
  if (seconds < 1) return `${(seconds * 1000).toFixed(0)}ms`;
  return `${seconds.toFixed(2)}s`;
}

function formatFullDate(dateString: string): string {
  const date = new Date(dateString);
  return date.toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function renderMarkdown(content: string): string {
  const rawHtml = marked(content) as string;
  return sanitizeMarkdown(rawHtml);
}
</script>

<style scoped>
.line-clamp-1,
.line-clamp-2 {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.line-clamp-1 {
  -webkit-line-clamp: 1;
}
.line-clamp-2 {
  -webkit-line-clamp: 2;
}

.scrollbar-thin::-webkit-scrollbar {
  width: 5px;
}
.scrollbar-thin::-webkit-scrollbar-track {
  background: transparent;
}
.scrollbar-thin::-webkit-scrollbar-thumb {
  background: rgb(203 213 225);
  border-radius: 2px;
}
.dark .scrollbar-thin::-webkit-scrollbar-thumb {
  background: rgb(71 85 105);
}
.scrollbar-thin::-webkit-scrollbar-thumb:hover {
  background: rgb(148 163 184);
}
.dark .scrollbar-thin::-webkit-scrollbar-thumb:hover {
  background: rgb(100 116 139);
}

:deep(.prose) {
  color: var(--color-text);
}
:deep(.prose p) {
  margin-top: 0.75em;
  margin-bottom: 0.75em;
  line-height: 1.65;
}
:deep(.prose ul),
:deep(.prose ol) {
  margin-top: 0.75em;
  margin-bottom: 0.75em;
  padding-left: 1.5em;
}
:deep(.prose li) {
  margin-top: 0.25em;
  margin-bottom: 0.25em;
}
:deep(.prose h1),
:deep(.prose h2),
:deep(.prose h3),
:deep(.prose h4) {
  margin-top: 1.5em;
  margin-bottom: 0.75em;
  font-weight: 600;
  color: var(--color-text);
}
:deep(.prose code) {
  background: var(--color-code-background);
  color: var(--color-code-text);
  padding: 0.2em 0.4em;
  border-radius: 4px;
  font-size: 0.9em;
  font-weight: 500;
}
:deep(.prose pre) {
  background: var(--color-code-background);
  padding: 1em;
  border-radius: 8px;
  overflow-x: auto;
}
:deep(.prose a) {
  color: var(--book-cloth);
  text-decoration: underline;
}
:deep(.prose blockquote) {
  border-left: 3px solid var(--book-cloth);
  padding-left: 1em;
  margin-left: 0;
  font-style: italic;
  color: var(--cloud-dark);
}
:deep(.prose strong) {
  font-weight: 600;
}
</style>
