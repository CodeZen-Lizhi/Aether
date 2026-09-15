<template>
  <div class="minimal-request-timeline">
    <!-- Loading State -->
    <div
      v-if="loading"
      class="py-4"
    >
      <Skeleton class="h-32 w-full" />
    </div>

    <!-- Error State -->
    <Card
      v-else-if="error"
      class="border-red-200 dark:border-red-800"
    >
      <div class="p-4">
        <p class="text-sm text-red-600 dark:text-red-400">
          {{ error }}
        </p>
      </div>
    </Card>

    <!-- Timeline Content -->
    <div
      v-else-if="trace && trace.candidates.length > 0"
      class="space-y-0"
    >
      <Card>
        <div
          class="trace-card"
          @keydown="handlePagerKeydown"
        >
          <!-- 概览信息 -->
          <div class="flex flex-wrap items-center justify-between gap-2 mb-4">
            <div class="flex flex-wrap items-center gap-x-3 gap-y-2">
              <h4 class="text-sm font-semibold whitespace-nowrap">
                请求链路追踪
              </h4>
              <span
                class="final-status"
                :class="getStatusColorClass(computedFinalStatus)"
              >
                <span
                  class="title-dot"
                  :class="getStatusColorClass(computedFinalStatus)"
                  aria-hidden="true"
                />
                {{ getFinalStatusLabel(computedFinalStatus) }}
              </span>
            </div>
            <div class="text-sm text-muted-foreground">
              {{ formatLatency(totalTraceLatency) }}
            </div>
          </div>

          <!-- 选中详情面板 -->
          <Transition name="slide-up">
            <div
              v-if="currentAttempt"
              class="detail-panel"
              :data-attempt-id="currentAttempt.id"
            >
              <div class="panel-header">
                <div class="panel-title">
                  <span
                    class="title-dot"
                    :class="getStatusColorClass(currentAttemptDisplayStatus)"
                  />
                  <span class="title-text">{{ getProviderDisplayName(currentAttempt) }}</span>
                  <a
                    v-if="currentAttempt.provider_website"
                    :href="currentAttempt.provider_website"
                    target="_blank"
                    rel="noopener noreferrer"
                    class="provider-link"
                    @click.stop
                  >
                    <ExternalLink class="w-3 h-3" />
                  </a>
                  <span
                    class="status-tag"
                    :class="getStatusColorClass(currentAttemptDisplayStatus)"
                  >
                    {{ getStatusLabel(currentAttemptDisplayStatus) }}
                    <span
                      v-if="currentAttempt.status_code != null && currentAttemptDisplayStatus !== 'skipped'"
                      class="attempt-http"
                    >HTTP {{ currentAttempt.status_code }}</span>
                  </span>
                </div>
                <span class="attempt-latency">{{ formatLatency(currentAttempt.latency_ms) }}</span>
              </div>

              <div class="panel-body">
                <!-- 核心信息网格 -->
                <div class="info-grid">
                  <div
                    v-if="currentAttemptTimeRange"
                    class="info-item"
                  >
                    <span class="info-label">时间范围</span>
                    <span
                      class="info-value mono time-range-value"
                      :title="currentAttemptTimeRange.durationLabel"
                    >
                      {{ formatTime(currentAttemptTimeRange.startIso) }}
                      <span class="time-arrow">→</span>
                      {{ currentAttemptTimeRange.endIso ? formatTime(currentAttemptTimeRange.endIso) : '进行中' }}
                    </span>
                  </div>
                  <div
                    v-if="currentAttempt.extra_data?.first_byte_time_ms != null"
                    class="info-item"
                  >
                    <span class="info-label">首字 (TTFB)</span>
                    <span class="info-value mono">{{ formatLatency(currentAttempt.extra_data.first_byte_time_ms) }}</span>
                  </div>
                  <div
                    v-if="currentAttemptFormatDisplay"
                    class="info-item"
                  >
                    <span class="info-label">格式</span>
                    <span class="info-value">
                      <code class="format-code">{{ currentAttemptFormatDisplay }}</code>
                    </span>
                  </div>
                  <div
                    v-if="currentAttemptRequestPathDisplay"
                    class="info-item"
                  >
                    <span class="info-label">请求路径</span>
                    <span class="info-value">
                      <code class="format-code request-path-code">{{ currentAttemptRequestPathDisplay }}</code>
                    </span>
                  </div>
                  <div
                    v-if="currentAttemptKeyDisplay"
                    class="info-item"
                  >
                    <span class="info-label">密钥</span>
                    <span class="info-value info-value-stacked">
                      <span class="key-name">
                        {{ currentAttemptKeyDisplay }}
                        <span
                          v-if="currentAttempt.key_auth_type && currentAttempt.key_auth_type !== 'api_key'"
                          class="auth-type-tag"
                        >{{ formatAuthTypeWithPlan(currentAttempt.key_auth_type) }}</span>
                      </span>
                      <code
                        v-if="currentAttempt.key_preview"
                        class="key-preview"
                      >{{ currentAttempt.key_preview }}</code>
                    </span>
                  </div>
                  <div
                    v-if="currentAttemptKeyFormatsDisplay"
                    class="info-item"
                  >
                    <span class="info-label">支持端点</span>
                    <span class="info-value info-value-stacked">
                      <code class="format-code">{{ currentAttemptKeyFormatsDisplay }}</code>
                      <span class="text-xs text-muted-foreground">
                        Key 声明的可用 endpoint 格式
                      </span>
                    </span>
                  </div>
                  <div
                    v-if="currentAttempt.extra_data?.proxy"
                    class="info-item"
                  >
                    <span class="info-label">代理</span>
                    <span class="info-value info-value-stacked">
                      <span class="proxy-name">
                        {{ currentAttempt.extra_data.proxy.node_name || currentAttempt.extra_data.proxy.url || '未知' }}
                        <span
                          v-if="currentAttempt.extra_data.proxy.source === 'system'"
                          class="text-xs text-muted-foreground ml-1"
                        >(系统)</span>
                      </span>
                      <span class="proxy-detail">
                        <span
                          v-if="currentAttempt.extra_data.proxy.ttfb_ms != null"
                          class="text-xs text-muted-foreground"
                        >{{ formatLatency(currentAttempt.extra_data.proxy.ttfb_ms) }}</span>
                        <span
                          v-if="currentAttempt.extra_data.proxy.timing"
                          class="text-xs text-muted-foreground"
                        >(<!--
                          -->{{ proxyTimingBreakdown(currentAttempt.extra_data.proxy) }}<!--
                        -->)</span>
                      </span>
                      <code
                        v-if="typeof currentAttempt.extra_data.proxy.node_id === 'string' && currentAttempt.extra_data.proxy.node_id"
                        class="text-xs font-mono text-muted-foreground"
                      >节点 Key {{ currentAttempt.extra_data.proxy.node_id }}</code>
                    </span>
                  </div>
                </div>

                <div
                  v-if="currentImageProgress"
                  class="image-progress-block"
                >
                  <div class="image-progress-header">
                    <span class="image-progress-title">图片生成进度</span>
                    <span
                      class="image-progress-phase"
                      :class="imageProgressPhaseClass(currentImageProgress.phase)"
                    >
                      {{ formatImageProgressPhase(currentImageProgress.phase) }}
                    </span>
                  </div>
                  <div class="image-progress-grid">
                    <div class="image-progress-item">
                      <span class="image-progress-label">上游 TTFB</span>
                      <span class="image-progress-value mono">{{ formatLatency(currentImageProgress.upstream_ttfb_ms) }}</span>
                    </div>
                    <div class="image-progress-item">
                      <span class="image-progress-label">SSE 帧数</span>
                      <span class="image-progress-value mono">{{ formatProgressCount(currentImageProgress.upstream_sse_frame_count) }}</span>
                    </div>
                    <div class="image-progress-item">
                      <span class="image-progress-label">Partial 图片</span>
                      <span class="image-progress-value mono">{{ formatProgressCount(currentImageProgress.partial_image_count) }}</span>
                    </div>
                    <div class="image-progress-item">
                      <span class="image-progress-label">最后帧</span>
                      <span class="image-progress-value mono">{{ formatProgressFrameTime(currentImageProgress.last_upstream_frame_at_unix_ms) }}</span>
                    </div>
                    <template v-if="hasDownstreamHeartbeatProgress">
                      <div class="image-progress-item">
                        <span class="image-progress-label">下游心跳</span>
                        <span class="image-progress-value mono">{{ formatProgressCount(currentImageProgress.downstream_heartbeat_count) }}</span>
                      </div>
                      <div class="image-progress-item">
                        <span class="image-progress-label">心跳间隔</span>
                        <span class="image-progress-value mono">{{ formatLatency(currentImageProgress.downstream_heartbeat_interval_ms) }}</span>
                      </div>
                      <div class="image-progress-item">
                        <span class="image-progress-label">最后心跳</span>
                        <span class="image-progress-value mono">{{ formatProgressFrameTime(currentImageProgress.last_downstream_heartbeat_at_unix_ms) }}</span>
                      </div>
                    </template>
                    <div
                      v-if="currentImageProgress.last_upstream_event"
                      class="image-progress-item full-width"
                    >
                      <span class="image-progress-label">上游事件</span>
                      <code class="image-progress-code">{{ currentImageProgress.last_upstream_event }}</code>
                    </div>
                    <div
                      v-if="currentImageProgress.last_client_visible_event"
                      class="image-progress-item full-width"
                    >
                      <span class="image-progress-label">客户端可见事件</span>
                      <code class="image-progress-code">{{ currentImageProgress.last_client_visible_event }}</code>
                    </div>
                  </div>
                </div>

                <!-- 用量与费用（仅成功节点显示） -->
                <div
                  v-if="currentAttempt.status === 'success' && usageData"
                  class="usage-section"
                >
                  <div class="usage-grid">
                    <!-- 输入 输出 -->
                    <div class="usage-row">
                      <div class="usage-item">
                        <span class="usage-label">输入</span>
                        <span class="usage-tokens">{{ formatNumber(usageData.tokens.input) }}</span>
                        <span class="usage-cost">${{ usageData.cost.input.toFixed(6) }}</span>
                      </div>
                      <div class="usage-divider" />
                      <div class="usage-item">
                        <span class="usage-label">输出</span>
                        <span class="usage-tokens">{{ formatNumber(usageData.tokens.output) }}</span>
                        <span class="usage-cost">${{ usageData.cost.output.toFixed(6) }}</span>
                      </div>
                    </div>
                    <!-- 缓存创建 缓存读取（仅在有缓存数据时显示） -->
                    <div
                      v-if="usageData.tokens.cache_creation || usageData.tokens.cache_read"
                      class="usage-row"
                    >
                      <div class="usage-item">
                        <span class="usage-label">缓存创建</span>
                        <span class="usage-tokens">{{ formatNumber(usageData.tokens.cache_creation || 0) }}</span>
                        <span class="usage-cost">${{ (usageData.cost.cache_creation || 0).toFixed(6) }}</span>
                      </div>
                      <div class="usage-divider" />
                      <div class="usage-item">
                        <span class="usage-label">缓存读取</span>
                        <span class="usage-tokens">{{ formatNumber(usageData.tokens.cache_read || 0) }}</span>
                        <span class="usage-cost">${{ (usageData.cost.cache_read || 0).toFixed(6) }}</span>
                      </div>
                    </div>
                  </div>
                </div>

                <!-- 跳过原因 -->
                <div
                  v-if="currentAttemptSkipReasonDisplay && !currentAttemptRequestError"
                  class="skip-reason"
                >
                  <span class="reason-label">跳过原因</span>
                  <span class="reason-content">
                    <span class="reason-value">{{ currentAttemptSkipReasonDisplay }}</span>
                    <span
                      v-if="currentAttemptFailureDiagnostic"
                      class="reason-detail"
                    >
                      <code>{{ currentAttemptFailureDiagnostic.path }}</code>
                      {{ currentAttemptFailureDiagnostic.message }}
                    </span>
                  </span>
                </div>

                <RequestAttemptErrorPanel
                  v-if="currentAttemptRequestError"
                  :key="currentAttempt.id"
                  :error="currentAttemptRequestError"
                  :is-dark="isDark"
                />

                <!-- 额外数据 -->
                <details
                  v-if="currentAttemptExtraDataDisplay"
                  class="extra-block"
                >
                  <summary class="extra-toggle">
                    额外信息
                  </summary>
                  <JsonContentPanel
                    class="extra-json-panel"
                    :data="currentAttemptExtraDataDisplay"
                    :is-dark="isDark"
                    empty-message="无额外信息"
                  />
                </details>
              </div>
            </div>
          </Transition>
          <template v-if="timeline.length > 1">
            <button
              type="button"
              class="trace-edge trace-edge-prev"
              aria-label="上一条尝试"
              @click="navigateAttempt(-1)"
            >
              <span><ChevronLeft class="h-6 w-6" /></span>
            </button>
            <button
              type="button"
              class="trace-edge trace-edge-next"
              aria-label="下一条尝试"
              @click="navigateAttempt(1)"
            >
              <span><ChevronRight class="h-6 w-6" /></span>
            </button>
          </template>
          <div
            v-if="timeline.length"
            class="trace-pagination"
            aria-live="polite"
            aria-atomic="true"
          >
            {{ selectedAttemptIndex + 1 }} / {{ timeline.length }}
          </div>
        </div>
      </Card>
    </div>

    <!-- Empty State -->
    <Card
      v-else
      class="border-dashed"
    >
      <div class="p-8 text-center">
        <p class="text-sm text-muted-foreground">
          暂无追踪数据
        </p>
      </div>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, onBeforeUnmount } from 'vue'
import { isAxiosError } from 'axios'
import Card from '@/components/ui/card.vue'
import Skeleton from '@/components/ui/skeleton.vue'
import JsonContentPanel from './JsonContentPanel.vue'
import RequestAttemptErrorPanel from './RequestAttemptErrorPanel.vue'
import { ChevronLeft, ChevronRight, ExternalLink } from 'lucide-vue-next'
import { requestTraceApi, type RequestTrace, type CandidateRecord, type ImageProgress } from '@/api/requestTrace'
import { log } from '@/utils/logger'
import { parseApiError } from '@/utils/errorParser'
import { formatTokens } from '@/utils/format'
import { formatApiFormat } from '@/api/endpoints/types/api-format'
import { useDarkMode } from '@/composables/useDarkMode'
import { resolveTimelineFinalStatus } from '../utils/status'
import { TIMELINE_STATUS } from '../utils/timelineCandidates'

interface AttemptTimeRange {
  startIso: string
  endIso?: string
  durationLabel?: string
}

// 用量数据类型
interface UsageData {
  status?: string | null
  tokens: {
    input: number
    output: number
    cache_creation: number
    cache_read: number
  }
  cost: {
    input: number
    output: number
    cache_creation: number
    cache_read: number
    per_request: number
    total: number
  }
  pricing: {
    input?: number
    output?: number
    cache_creation?: number
    cache_read?: number
    per_request?: number
  }
}

const props = defineProps<{
  requestId?: string | null
  /** 外部传入的状态码，用于覆盖 trace.final_status 的判断 */
  overrideStatusCode?: number
  /** 外部传入的请求状态，用于识别已失败/取消的终态请求 */
  requestStatus?: string | null
  /** 请求侧 API 格式（客户端入口格式） */
  requestApiFormat?: string | null
  /** 用量和费用数据 */
  usageData?: UsageData | null
  /** 请求元数据（用于请求路径解析） */
  requestMetadata?: Record<string, unknown> | null
  /** 已获取的追踪数据；传入时不再内部拉取 */
  traceData?: RequestTrace | null
}>()

const emit = defineEmits<{
  selectAttempt: [attempt: CandidateRecord | null]
  traceState: [state: {
    loaded: boolean
    hasTrace: boolean
    finalStatus?: RequestTrace['final_status'] | null
    statusCode?: number | null
    latencyMs?: number | null
    imageProgress?: ImageProgress | null
    errorMessage?: string | null
  }]
}>()

// 用量数据（从 props 获取）
const usageData = computed(() => props.usageData)

// 格式化数字
const formatNumber = (num: number): string => {
  return formatTokens(num)
}

// 获取最终状态标签
const getFinalStatusLabel = (status: string) => {
  const labels: Record<string, string> = {
    success: '请求成功',
    failed: '请求失败',
    cancelled: '已取消',
    streaming: '流式传输中',
    pending: '进行中'
  }
  return labels[status] || status
}

const loading = ref(false)
const error = ref<string | null>(null)
const internalTrace = ref<RequestTrace | null>(null)
const { isDark } = useDarkMode()
const trace = computed(() => props.traceData ?? internalTrace.value)
const selectedAttemptIndex = ref(0)
const selectionPinnedByUser = ref(false)
const traceLoadStarted = ref(false)
let tracePollTimer: ReturnType<typeof setTimeout> | null = null
let traceLoadInFlight: Promise<void> | null = null
let traceLoadVersion = 0
const TRACE_POLL_INTERVAL_MS = 1000

// 格式化延迟（自动调整单位）
const formatLatency = (ms: number | undefined | null): string => {
  if (ms === undefined || ms === null) return '-'
  if (ms >= 1000) {
    return `${(ms / 1000).toFixed(2)}s`
  }
  return `${ms}ms`
}

// 格式化字节大小
const formatSize = (bytes: number): string => {
  if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(1)}MB`
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)}KB`
  return `${bytes}B`
}

// 代理 timing 分阶段展示
const proxyTimingBreakdown = (proxy: Record<string, unknown>): string => {
  const t = proxy.timing as Record<string, number | null | undefined> | undefined
  if (!t) return ''

  const parts: string[] = []

  // 兼容旧版 timing（含 body_read_ms/decompress_ms）
  const readDecompress = ((t.body_read_ms as number) || 0) + ((t.decompress_ms as number) || 0)
  if (readDecompress > 0) {
    let label = `读取 ${formatLatency(readDecompress)}`
    if (t.decompress_ms != null && t.decompress_ms > 0 && t.wire_size != null && t.body_size != null && (t.body_size as number) > 0) {
      const ratio = Math.round((1 - (t.wire_size as number) / (t.body_size as number)) * 100)
      label += ` ${formatSize(t.wire_size as number)}→${formatSize(t.body_size as number)}`
      if (ratio > 0) label += ` -${ratio}%`
    }
    parts.push(label)
  }

  const ttfbMs = t.ttfb_ms ?? t.upstream_ms
  const responseWaitMs = t.response_wait_ms ?? (
    t.connection_acquire_ms != null && ttfbMs != null
      ? Math.max(0, (ttfbMs as number) - (t.connection_acquire_ms as number))
      : null
  )
  const legacyWaitMs = t.upstream_processing_ms ?? (
    ttfbMs != null && t.connect_ms != null && t.tls_ms != null
      ? Math.max(0, (ttfbMs as number) - (t.connect_ms as number) - (t.tls_ms as number))
      : null
  )

  if (t.dns_ms != null && (t.dns_ms as number) > 0) {
    parts.push(`DNS ${formatLatency(t.dns_ms as number)}`)
  }
  if ((t.connection_reused as unknown) === true) {
    parts.push('复用连接')
  }
  if (t.connect_ms != null && (t.connect_ms as number) > 0) {
    parts.push(`连接 ${formatLatency(t.connect_ms as number)}`)
  }
  if (t.tls_ms != null && (t.tls_ms as number) > 0) {
    parts.push(`TLS ${formatLatency(t.tls_ms as number)}`)
  }
  if (ttfbMs != null && (ttfbMs as number) > 0) {
    parts.push(`TTFB ${formatLatency(ttfbMs as number)}`)
  }
  if (responseWaitMs != null && (responseWaitMs as number) > 0) {
    parts.push(`等待响应头 ${formatLatency(Math.round(responseWaitMs as number))}`)
  } else if (legacyWaitMs != null && (legacyWaitMs as number) > 0) {
    parts.push(`等待响应头(旧版估算) ${formatLatency(Math.round(legacyWaitMs as number))}`)
  }

  // 计算 Aether→代理 之间无法解释的耗时差
  if (proxy.ttfb_ms != null && t.total_ms != null) {
    const gap = (proxy.ttfb_ms as number) - (t.total_ms as number)
    if (gap > 500) {
      parts.push(`传输 ${formatLatency(Math.round(gap))}`)
    }
  }

  return parts.join(' / ')
}

const isLiveCandidate = (candidate: CandidateRecord): boolean => {
  if (candidate.status === 'streaming') return true
  return candidate.status === 'pending' && Boolean(candidate.started_at)
}

// 计算最终状态：优先检查真正已启动的进行中状态，再使用外部状态码
const computedFinalStatus = computed(() => {
  const hasPending = trace.value?.candidates?.some(isLiveCandidate)
  return resolveTimelineFinalStatus({
    hasPendingCandidates: hasPending,
    statusCode: props.overrideStatusCode,
    requestStatus: props.requestStatus ?? usageData.value?.status,
    traceFinalStatus: trace.value?.final_status,
  })
})

const compareBySchedulingOrder = (a: CandidateRecord, b: CandidateRecord): number => {
  if (a.candidate_index !== b.candidate_index) {
    return a.candidate_index - b.candidate_index
  }
  if (a.retry_index !== b.retry_index) {
    return a.retry_index - b.retry_index
  }
  return new Date(a.created_at).getTime() - new Date(b.created_at).getTime()
}

// 候选时间线（按调度顺序排序；lazy 加载的跳过候选通常没有 started_at）
const rawTimeline = computed<CandidateRecord[]>(() => {
  if (!trace.value) return []
  return [...trace.value.candidates]
    .filter(c => TIMELINE_STATUS.includes(c.status))
    .sort(compareBySchedulingOrder)
})


const timeline = computed<CandidateRecord[]>(() => rawTimeline.value)

const getProviderDisplayName = (attempt: CandidateRecord | null | undefined): string => {
  if (!attempt) return '未知'
  const providerName = String(attempt.provider_name || '').trim()
  if (providerName) return providerName
  return '未知'
}

// The trace aggregate includes every attempted candidate, including failed
// failover attempts. Per-candidate latency remains provider-scoped.
const totalTraceLatency = computed(() => {
  if (!rawTimeline.value || rawTimeline.value.length === 0) return 0

  const aggregateLatency = trace.value?.total_latency_ms
  if (typeof aggregateLatency === 'number' && Number.isFinite(aggregateLatency) && aggregateLatency > 0) {
    return aggregateLatency
  }

  const attemptedLatency = rawTimeline.value.reduce((sum, candidate) => {
    const latency = normalizeLatencyMs(candidate.latency_ms)
    return sum + (latency ?? 0)
  }, 0)
  if (attemptedLatency > 0) {
    return attemptedLatency
  }

  // Historical transport failures may not have latency_ms. Recover the wall
  // clock span from candidate timestamps for those records.
  let earliestStart: number | null = null
  let latestEnd: number | null = null

  for (const candidate of rawTimeline.value) {
    if (candidate.started_at) {
      const startTime = new Date(candidate.started_at).getTime()
      if (earliestStart === null || startTime < earliestStart) {
        earliestStart = startTime
      }
    }
    if (candidate.finished_at) {
      const endTime = new Date(candidate.finished_at).getTime()
      if (latestEnd === null || endTime > latestEnd) {
        latestEnd = endTime
      }
    }
  }

  if (earliestStart !== null && latestEnd !== null) {
    return latestEnd - earliestStart
  }
  return 0
})

// One page per recorded candidate / Key attempt, in scheduling order.
const currentAttempt = computed(() => timeline.value[selectedAttemptIndex.value] ?? null)

const currentAttemptTimeRange = computed<AttemptTimeRange | null>(() => {
  return resolveAttemptTimeRange(currentAttempt.value)
})

const currentAttemptDisplayStatus = computed(() => getDisplayStatus(currentAttempt.value))

watch(currentAttempt, (attempt) => {
  emit('selectAttempt', attempt ?? null)
}, { immediate: true })

const normalizeFormatSignature = (value: string): string => {
  return value.trim().toLowerCase()
}

const extractObject = (value: unknown): Record<string, unknown> | null => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return null
  }
  return value as Record<string, unknown>
}

const readStringField = (obj: Record<string, unknown>, key: string): string | undefined => {
  const value = obj[key]
  return typeof value === 'string' && value.trim() ? value.trim() : undefined
}

const readNumberField = (obj: Record<string, unknown>, key: string): number | undefined => {
  const value = obj[key]
  if (typeof value === 'number' && Number.isFinite(value)) return value
  if (typeof value === 'string' && value.trim()) {
    const parsed = Number(value)
    if (Number.isFinite(parsed)) return parsed
  }
  return undefined
}

const hasRenderableValue = (value: unknown): boolean => {
  if (value == null) return false
  if (typeof value === 'string') return value.trim().length > 0
  if (typeof value === 'object') return Object.keys(value as Record<string, unknown>).length > 0
  return true
}

const normalizeImageProgress = (value: unknown): ImageProgress | null => {
  const raw = extractObject(value)
  if (!raw) return null

  const progress: ImageProgress = {
    phase: readStringField(raw, 'phase'),
    upstream_ttfb_ms: readNumberField(raw, 'upstream_ttfb_ms') ?? null,
    upstream_sse_frame_count: readNumberField(raw, 'upstream_sse_frame_count') ?? null,
    last_upstream_event: readStringField(raw, 'last_upstream_event') ?? null,
    last_upstream_frame_at_unix_ms: readNumberField(raw, 'last_upstream_frame_at_unix_ms') ?? null,
    partial_image_count: readNumberField(raw, 'partial_image_count') ?? null,
    last_client_visible_event: readStringField(raw, 'last_client_visible_event') ?? null,
    downstream_heartbeat_count: readNumberField(raw, 'downstream_heartbeat_count') ?? null,
    last_downstream_heartbeat_at_unix_ms: readNumberField(raw, 'last_downstream_heartbeat_at_unix_ms') ?? null,
    downstream_heartbeat_interval_ms: readNumberField(raw, 'downstream_heartbeat_interval_ms') ?? null,
  }

  return Object.values(progress).some(value => value !== undefined && value !== null && value !== '') ? progress : null
}

const currentImageProgress = computed<ImageProgress | null>(() => {
  const attempt = currentAttempt.value
  if (!attempt) return null
  return normalizeImageProgress(attempt.image_progress)
    ?? normalizeImageProgress(extractObject(attempt.extra_data)?.image_progress)
})

const formatImageProgressPhase = (phase?: string | null): string => {
  const labels: Record<string, string> = {
    upstream_connecting: '连接上游',
    upstream_streaming: '上游生成中',
    upstream_completed: '上游已完成',
    failed: '失败',
  }
  if (!phase) return '未知'
  return labels[phase] || phase
}

const imageProgressPhaseClass = (phase?: string | null): string => {
  if (phase === 'upstream_completed') return 'phase-completed'
  if (phase === 'failed') return 'phase-failed'
  if (phase === 'upstream_streaming') return 'phase-streaming'
  return 'phase-connecting'
}

const formatProgressCount = (value?: number | null): string => {
  return typeof value === 'number' && Number.isFinite(value) ? String(value) : '-'
}

const formatProgressFrameTime = (value?: number | null): string => {
  if (typeof value !== 'number' || !Number.isFinite(value) || value <= 0) return '-'
  const date = new Date(value)
  const time = formatTime(date.toISOString())
  const ageMs = Date.now() - value
  if (ageMs >= 0 && ageMs < 60_000) {
    return `${Math.max(0, Math.round(ageMs / 1000))}s 前 (${time})`
  }
  return time
}

const hasDownstreamHeartbeatProgress = computed(() => {
  const progress = currentImageProgress.value
  return typeof progress?.downstream_heartbeat_count === 'number' ||
    typeof progress?.last_downstream_heartbeat_at_unix_ms === 'number' ||
    typeof progress?.downstream_heartbeat_interval_ms === 'number'
})

const latestTraceAttemptForState = computed<CandidateRecord | null>(() => {
  const candidates = rawTimeline.value
  for (let index = candidates.length - 1; index >= 0; index -= 1) {
    const candidate = candidates[index]
    if (candidate.status !== 'available' && candidate.status !== 'unused') {
      return candidate
    }
  }
  return null
})

const latestTraceImageProgress = computed<ImageProgress | null>(() => {
  const candidates = rawTimeline.value
  for (let index = candidates.length - 1; index >= 0; index -= 1) {
    const candidate = candidates[index]
    const progress = normalizeImageProgress(candidate.image_progress)
      ?? normalizeImageProgress(extractObject(candidate.extra_data)?.image_progress)
    if (progress) return progress
  }
  return null
})

watch(
  [trace, loading, latestTraceImageProgress, latestTraceAttemptForState, computedFinalStatus],
  ([value, isLoading, imageProgress, attempt, finalStatus]) => {
    const waitingForInternalTrace = Boolean(props.requestId && !props.traceData && !traceLoadStarted.value && !value)
    emit('traceState', {
      loaded: !isLoading && !waitingForInternalTrace,
      hasTrace: Boolean(value?.candidates?.length),
      finalStatus: finalStatus ?? value?.final_status ?? null,
      statusCode: attempt?.status_code ?? null,
      latencyMs: value?.total_latency_ms ?? attempt?.latency_ms ?? null,
      imageProgress,
      errorMessage: attempt?.error_message ?? null,
    })
  },
  { immediate: true },
)

const normalizeUpstreamResponseDisplay = (value: unknown): Record<string, unknown> | null => {
  const raw = extractObject(value)
  if (!raw) return null
  const statusCode = readNumberField(raw, 'status_code') ?? readNumberField(raw, 'statusCode')
  const headers = raw.headers ?? raw.header
  const body = raw.body
  const bodyRef = readStringField(raw, 'body_ref') ?? readStringField(raw, 'bodyRef')
  const bodyState = readStringField(raw, 'body_state') ?? readStringField(raw, 'bodyState')
  const meaningfulBodyState = bodyState && bodyState.toLowerCase() !== 'none'
    ? bodyState
    : ''

  if (
    statusCode == null &&
    !hasRenderableValue(headers) &&
    !hasRenderableValue(body) &&
    !bodyRef &&
    !meaningfulBodyState
  ) {
    return null
  }

  const data: Record<string, unknown> = {}
  if (statusCode != null) data.status_code = statusCode
  if (hasRenderableValue(headers)) data.headers = headers
  if (hasRenderableValue(body)) data.body = body
  if (bodyRef) data.body_ref = bodyRef
  if (meaningfulBodyState) data.body_state = meaningfulBodyState

  return data
}

const extractStringList = (value: unknown): string[] => {
  if (Array.isArray(value)) {
    return value
      .map(item => typeof item === 'string' ? item.trim() : '')
      .filter(Boolean)
  }
  if (typeof value === 'string') {
    const raw = value.trim()
    if (!raw) return []
    try {
      return extractStringList(JSON.parse(raw))
    } catch {
      return [raw]
    }
  }
  return []
}

const resolveTransportDiagnostics = (attempt: CandidateRecord): Record<string, unknown> | null => {
  const extra = extractObject(attempt.extra_data)
  return extractObject(extra?.transport_diagnostics)
}

const currentAttemptFormatDisplay = computed(() => {
  const attempt = currentAttempt.value
  if (!attempt) return ''
  const extra = extractObject(attempt.extra_data) ?? {}

  const providerRaw = typeof extra.provider_api_format === 'string' ? extra.provider_api_format : ''
  const clientRawFromExtra = typeof extra.client_api_format === 'string' ? extra.client_api_format : ''
  const requestRaw = clientRawFromExtra || (typeof props.requestApiFormat === 'string' ? props.requestApiFormat : '')

  if (!providerRaw && !requestRaw) return ''

  const providerText = providerRaw ? formatApiFormat(providerRaw) : ''
  const requestText = requestRaw ? formatApiFormat(requestRaw) : ''
  const convertedByFlag = extra.needs_conversion === true
  const convertedByDiff = Boolean(
    providerRaw &&
      requestRaw &&
      normalizeFormatSignature(providerRaw) !== normalizeFormatSignature(requestRaw),
  )

  if ((convertedByFlag || convertedByDiff) && requestText && providerText) {
    return `${requestText} -> ${providerText}`
  }

  return providerText || requestText
})

const normalizeQueryString = (value: string): string => {
  const trimmed = value.trim()
  if (!trimmed) return ''
  return trimmed.startsWith('?') ? trimmed.slice(1) : trimmed
}

const resolveRequestPathFromObject = (value: unknown): string => {
  const object = extractObject(value)
  if (!object) return ''

  const pathWithQuery = (
    readStringField(object, 'request_path_and_query')
    || readStringField(object, 'public_path_and_query')
    || readStringField(object, 'path_and_query')
    || readStringField(object, 'request_uri')
    || readStringField(object, 'public_uri')
  )
  if (pathWithQuery) return pathWithQuery

  const path = (
    readStringField(object, 'request_path')
    || readStringField(object, 'public_path')
    || readStringField(object, 'path')
  )
  if (!path) return ''

  const query = normalizeQueryString(
    readStringField(object, 'request_query_string')
    || readStringField(object, 'public_query_string')
    || readStringField(object, 'query_string')
    || readStringField(object, 'query')
    || '',
  )
  if (!query || path.includes('?')) return path
  return `${path}?${query}`
}

const currentAttemptRequestPathDisplay = computed(() => {
  const attempt = currentAttempt.value
  const fromAttempt = resolveRequestPathFromObject(attempt?.extra_data)
  if (fromAttempt) return fromAttempt

  const fromTrace = resolveRequestPathFromObject(trace.value)
  if (fromTrace) return fromTrace

  const fromRequestMetadata = resolveRequestPathFromObject(props.requestMetadata)
  if (fromRequestMetadata) return fromRequestMetadata

  return ''
})

const currentAttemptKeyDisplay = computed(() => {
  const attempt = currentAttempt.value
  if (!attempt) return ''
  return attempt.key_account_label || attempt.key_name || attempt.key_id || ''
})

const currentAttemptKeyFormatsDisplay = computed(() => {
  const attempt = currentAttempt.value
  if (!attempt) return ''

  const formats = extractStringList(attempt.key_api_formats)
  if (!formats.length) return ''

  return formats
    .map(format => formatApiFormat(format))
    .join(' / ')
})
const SKIP_REASON_LABELS: Record<string, string> = {
  auth_api_key_concurrency_limit_reached: '调用方 API Key 并发已达上限',
  api_key_concurrency_limit_reached: '调用方 API Key 并发已达上限',
  provider_concurrency_limit_reached: '上游提供商并发已达上限',
  provider_key_concurrency_limit_reached: '上游账号并发已达上限',
  provider_request_body_build_failed: '上游请求体转换失败',
  provider_request_body_missing: '无法构建上游请求体',
}
const currentAttemptSkipReasonDisplay = computed(() => {
  const attempt = currentAttempt.value
  if (!attempt?.skip_reason) return ''

  const skipReasonLabel = SKIP_REASON_LABELS[attempt.skip_reason]
  if (skipReasonLabel) {
    return skipReasonLabel
  }

  if (attempt.skip_reason !== 'transport_unsupported') {
    return attempt.skip_reason
  }

  const transportDiagnostics = resolveTransportDiagnostics(attempt)
  const requestPair = extractObject(transportDiagnostics?.request_pair)
  const detailedReason = typeof requestPair?.transport_unsupported_reason === 'string'
    ? requestPair.transport_unsupported_reason.trim()
    : ''

  return detailedReason || attempt.skip_reason
})

const currentAttemptFailureDiagnostic = computed<{
  path: string
  message: string
} | null>(() => {
  const attempt = currentAttempt.value
  if (!attempt) return null
  const extra = extractObject(attempt.extra_data)
  const failureDiagnostic = extractVisibleFailureDiagnostic(extra)
  const error = failureDiagnostic
    ? failureDiagnostic
    : extractObject(extra?.request_body_build_error)
  const path = typeof error?.path === 'string' && error.path.trim()
    ? error.path.trim()
    : ''
  const message = typeof error?.message === 'string' && error.message.trim()
    ? error.message.trim()
    : ''
  if (!path && !message) return null
  return {
    path: path || '$',
    message: formatAttemptErrorMessage(message) || message || '请求体转换失败',
  }
})

const isGenericExecutionRuntimeStatusMessage = (message: string): boolean =>
  /execution runtime (stream )?returned non-success status \d+/i.test(message)

const isLocalSyncFinalizeDiagnostic = (message: string): boolean =>
  /local sync attempt failed before terminal finalization/i.test(message)
  || /unsupported provider stream (event|finish reason)/i.test(message)

const isActionableDiagnosticMessage = (message: string): boolean =>
  isLocalSyncFinalizeDiagnostic(message) || isConversionDiagnosticMessage(message)

const extractVisibleFailureDiagnostic = (
  extra: Record<string, unknown> | null | undefined,
): Record<string, unknown> | null => {
  const failureDiagnostic = extractObject(extra?.failure_diagnostic)
  if (!failureDiagnostic || failureDiagnostic.safe_to_show === false) return null
  return failureDiagnostic
}

const extractVisibleDiagnosticObjects = (
  extra: Record<string, unknown> | null | undefined,
): Array<Record<string, unknown>> => [
  extractVisibleFailureDiagnostic(extra),
  extractObject(extra?.request_conversion_error),
  extractObject(extra?.request_body_build_error),
].filter((value): value is Record<string, unknown> => Boolean(value))

const extractVisibleDiagnosticMessage = (
  extra: Record<string, unknown> | null | undefined,
): string => {
  for (const diagnostic of extractVisibleDiagnosticObjects(extra)) {
    const message = readStringField(diagnostic, 'message')
    if (message) return message
  }
  return ''
}

const chooseAttemptRawErrorMessage = (
  flowMessage: string,
  fallbackMessage: string,
  diagnosticMessage: string,
): string => {
  const flow = flowMessage.trim()
  const fallback = fallbackMessage.trim()
  const diagnostic = diagnosticMessage.trim()

  if (flow && fallback && !isActionableDiagnosticMessage(flow) && isActionableDiagnosticMessage(fallback)) {
    return fallback
  }
  if (flow && diagnostic && isGenericExecutionRuntimeStatusMessage(flow) && isActionableDiagnosticMessage(diagnostic)) {
    return diagnostic
  }
  if (fallback && diagnostic && isGenericExecutionRuntimeStatusMessage(fallback) && isActionableDiagnosticMessage(diagnostic)) {
    return diagnostic
  }

  return flow || fallback || diagnostic
}

const decodeRustDebugString = (value: string): string => {
  try {
    return JSON.parse(`"${value}"`)
  } catch {
    return value.replace(/\\"/g, '"')
  }
}

const normalizeDiagnosticFieldPath = (field: string): string => {
  const trimmed = field.trim()
  if (!trimmed || trimmed === '$') return '$'
  if (trimmed.startsWith('$')) return trimmed
  if (trimmed.startsWith('[')) return `$${trimmed}`
  return `$.${trimmed}`
}

const formatConversionPair = (source: string, target: string): string =>
  `${formatApiFormat(source.trim())} → ${formatApiFormat(target.trim())}`

const extractFieldDetail = (message: string): string => {
  const fieldMatch = message.match(/field\s+([^;=]+?)\s*=\s*("(?:\\.|[^"\\])*"|[^;]+)/i)
  const unsupportedFieldMatch = message.match(/field\s+([^;]+?)\s+is unsupported/i)
  if (fieldMatch?.[1]) {
    return `字段 ${normalizeDiagnosticFieldPath(fieldMatch[1])} = ${fieldMatch[2].trim()}`
  }
  if (unsupportedFieldMatch?.[1]) {
    return `字段 ${normalizeDiagnosticFieldPath(unsupportedFieldMatch[1])} 不支持`
  }
  return ''
}

const formatUnsupportedStreamEventMessage = (message: string): string => {
  const detail = extractFieldDetail(message)
  const fieldDetail = detail ? `（${detail}）` : ''
  return `流式格式转换失败：上游返回了当前不支持的 stream event${fieldDetail}，无法无损转换到客户端请求格式`
}

const formatUnsupportedFinishReasonMessage = (message: string): string => {
  const fieldDetail = extractFieldDetail(message)
  const legacyMatch = message.match(/unsupported provider stream finish reason\s+(.+?)\s+cannot be converted losslessly/i)
  const detail = fieldDetail || (legacyMatch?.[1]
    ? `字段 $.finish_reason = ${legacyMatch[1].trim()}`
    : '')
  return `流式格式转换失败：上游返回了当前不支持的 finish reason${detail ? `（${detail}）` : ''}，无法无损转换到客户端请求格式`
}

const formatKnownConversionErrorMessage = (message: string): string => {
  const lossy = message.match(/^lossy conversion blocked from\s+(\S+)\s+to\s+(\S+)\s+at\s+([^:]+):\s*(.+)$/i)
  if (lossy) {
    return `格式转换失败：${formatConversionPair(lossy[1], lossy[2])} 在字段 ${normalizeDiagnosticFieldPath(lossy[3])} 会丢失信息：${lossy[4].trim()}`
  }

  const unaudited = message.match(/^unaudited field\s+(.+?)\s+in\s+(.+?)\s+cannot be converted to\s+([^:]+):\s*(.+)$/i)
  if (unaudited) {
    return `格式转换失败：${formatConversionPair(unaudited[2], unaudited[3])} 的字段 ${normalizeDiagnosticFieldPath(unaudited[1])} 尚未审计，不能安全转换：${unaudited[4].trim()}`
  }

  const unsupportedField = message.match(/^unsupported field\s+(.+?)\s+in\s+([^:]+(?::[^:]+)?):\s*(.+)$/i)
  if (unsupportedField) {
    return `格式转换失败：${formatApiFormat(unsupportedField[2])} 不支持字段 ${normalizeDiagnosticFieldPath(unsupportedField[1])}：${unsupportedField[3].trim()}`
  }

  const invalidEnum = message.match(/^invalid enum value\s+(.+?)\s+for\s+(.+)\.([^.\s]+)$/i)
  if (invalidEnum) {
    return `格式转换失败：${formatApiFormat(invalidEnum[2])} 字段 ${normalizeDiagnosticFieldPath(invalidEnum[3])} 的枚举值 ${invalidEnum[1].trim()} 无效`
  }

  const invalidTarget = message.match(/^invalid target field\s+(.+?)\s+for\s+(.+?):\s*(.+)$/i)
  if (invalidTarget) {
    return `格式转换失败：目标格式 ${formatApiFormat(invalidTarget[2])} 字段 ${normalizeDiagnosticFieldPath(invalidTarget[1])} 无效：${invalidTarget[3].trim()}`
  }

  const unsupportedFormat = message.match(/^unsupported AI format:\s*(.+)$/i)
  if (unsupportedFormat) {
    return `格式转换失败：不支持的 API 格式 ${unsupportedFormat[1].trim()}`
  }

  const parseEmit = message.match(/^failed to\s+(parse|emit)\s+(.+?)\s+(request|response)$/i)
  if (parseEmit) {
    const action = parseEmit[1].toLowerCase() === 'parse' ? '解析' : '生成'
    const subject = parseEmit[3].toLowerCase() === 'request' ? '请求体' : '响应体'
    return `格式转换失败：无法${action} ${formatApiFormat(parseEmit[2])} ${subject}`
  }

  return ''
}

const isConversionDiagnosticMessage = (message: string): boolean => {
  const normalized = message.trim()
  if (!normalized) return false
  return /conversion|converted|convertible|cannot be converted|lossy conversion|unsupported field|unaudited field|invalid enum value|invalid target field|unsupported ai format|failed to (parse|emit) .+ (request|response)|unsupported provider stream (event|finish reason)|转换|无损|字段 .*不支持/i
    .test(normalized)
}

const formatAttemptErrorMessage = (message: string, statusCode?: number): string => {
  const normalized = message.trim()
  if (!normalized) return ''
  const directInternal = normalized.match(/^Internal\("((?:\\.|[^"\\])*)"\)$/i)
  if (directInternal?.[1]) {
    return formatAttemptErrorMessage(decodeRustDebugString(directInternal[1]), statusCode)
  }
  const localSyncInternal = normalized.match(/local sync attempt failed before terminal finalization:\s*Internal\("((?:\\.|[^"\\])*)"\)/i)
  if (localSyncInternal?.[1]) {
    return formatAttemptErrorMessage(decodeRustDebugString(localSyncInternal[1]), statusCode)
  }
  if (/unsupported provider stream event cannot be converted losslessly/i.test(normalized)) {
    return formatUnsupportedStreamEventMessage(normalized)
  }
  if (/unsupported provider stream finish reason/i.test(normalized)) {
    return formatUnsupportedFinishReasonMessage(normalized)
  }
  const conversionMessage = formatKnownConversionErrorMessage(normalized)
  if (conversionMessage) {
    return conversionMessage
  }
  if (isGenericExecutionRuntimeStatusMessage(normalized)) {
    return statusCode != null ? `上游返回非成功状态 ${statusCode}` : '上游返回非成功状态'
  }
  return normalized
}

const shouldShowAttemptMessageWithUpstreamResponse = (
  rawMessage: string,
  upstreamResponse: Record<string, unknown> | null,
): boolean => {
  if (!upstreamResponse) return true
  const normalized = rawMessage.trim()
  if (!normalized) return false
  if (isLocalSyncFinalizeDiagnostic(normalized)) return true
  if (isConversionDiagnosticMessage(normalized)) return true
  if (isGenericExecutionRuntimeStatusMessage(normalized)) return false

  const hasBody = hasRenderableValue(upstreamResponse.body)
  const bodyState = (readStringField(upstreamResponse, 'body_state') ?? '').toLowerCase()
  return !hasBody && bodyState === 'disabled'
}

const currentAttemptRequestError = computed<{
  message: string
  technicalMessage: string
  presentationSource: string
  statusCode?: number
  upstreamResponse: Record<string, unknown> | null
  diagnostic: Record<string, unknown> | null
  skipReason?: string
  skipReasonLabel?: string
  skipped?: boolean
} | null>(() => {
  const attempt = currentAttempt.value
  if (!attempt || !['failed', 'skipped', 'stream_interrupted'].includes(getDisplayStatus(attempt))) return null

  const extra = extractObject(attempt.extra_data)
  const upstreamResponse = extractObject(extra?.upstream_response)
  const errorFlow = extractObject(extra?.error_flow)
  const statusCode = readNumberField(upstreamResponse ?? {}, 'status_code')
    ?? readNumberField(upstreamResponse ?? {}, 'statusCode')
    ?? readNumberField(errorFlow ?? {}, 'status_code')
    ?? readNumberField(errorFlow ?? {}, 'statusCode')
    ?? attempt.status_code
  const flowMessage = errorFlow
    ? readStringField(errorFlow, 'message')
    : ''
  const fallbackMessage = typeof attempt.error_message === 'string' && attempt.error_message.trim()
    ? attempt.error_message.trim()
    : ''
  const fallbackType = typeof attempt.error_type === 'string' && attempt.error_type.trim()
    ? attempt.error_type.trim()
    : ''
  const diagnosticMessage = extractVisibleDiagnosticMessage(extra)
  const rawMessage = chooseAttemptRawErrorMessage(flowMessage || '', fallbackMessage, diagnosticMessage)
  const message = formatAttemptErrorMessage(rawMessage, statusCode) || fallbackType
  const upstreamResponseDisplay = normalizeUpstreamResponseDisplay(extra?.upstream_response)
  const visibleDiagnosticObjects = extractVisibleDiagnosticObjects(extra)
  const shouldAttachDiagnostic = Boolean(
    visibleDiagnosticObjects.length
      || isLocalSyncFinalizeDiagnostic(rawMessage)
      || isConversionDiagnosticMessage(rawMessage),
  )
  const diagnostic = shouldAttachDiagnostic
    ? buildAttemptDiagnosticPayload(
        attempt,
        message || fallbackType || rawMessage || '未知失败',
        statusCode,
        upstreamResponseDisplay,
        rawMessage,
      )
    : null
  const upstreamResponseData: Record<string, unknown> = {}
  const responseHeader = upstreamResponseDisplay?.headers
  const responseBody = upstreamResponseDisplay?.body
  if (hasRenderableValue(responseHeader)) upstreamResponseData.header = responseHeader
  if (hasRenderableValue(responseBody)) upstreamResponseData.body = responseBody
  const response = Object.keys(upstreamResponseData).length > 0
    ? upstreamResponseData
    : null
  if (
    !attempt.skip_reason
    && !message
    && statusCode == null
    && !response
    && !diagnostic
  ) return null
  const showMessage = shouldShowAttemptMessageWithUpstreamResponse(
    rawMessage || fallbackType,
    upstreamResponseDisplay,
  )

  return {
    message: showMessage ? (message || (attempt.status === 'skipped' ? currentAttemptSkipReasonDisplay.value : '未知错误')) : '',
    technicalMessage: showMessage ? (rawMessage || fallbackType || attempt.skip_reason || '') : '',
    skipReason: attempt.skip_reason,
    skipReasonLabel: currentAttemptSkipReasonDisplay.value,
    skipped: attempt.status === 'skipped',
    presentationSource: rawMessage || fallbackType || message,
    statusCode,
    upstreamResponse: response,
    diagnostic,
  }
})

const diagnosticPathFromObject = (value: unknown): string => {
  const object = extractObject(value)
  if (!object) return ''
  return readStringField(object, 'path')
    || readStringField(object, 'field_path')
    || readStringField(object, 'fieldPath')
    || readStringField(object, 'field')
    || ''
}

const diagnosticFieldPathFromMessage = (message: string): string => {
  const normalized = message.trim()
  if (!normalized) return ''
  const fieldMatch = normalized.match(/field\s+([^;=]+?)\s*(?:=|is unsupported|不支持)/i)
  if (fieldMatch?.[1]) return normalizeDiagnosticFieldPath(fieldMatch[1])
  const lossyMatch = normalized.match(/lossy conversion blocked from\s+\S+\s+to\s+\S+\s+at\s+([^:]+):/i)
  if (lossyMatch?.[1]) return normalizeDiagnosticFieldPath(lossyMatch[1])
  const invalidTargetMatch = normalized.match(/invalid target field\s+(.+?)\s+for\s+/i)
  if (invalidTargetMatch?.[1]) return normalizeDiagnosticFieldPath(invalidTargetMatch[1])
  const unsupportedFieldMatch = normalized.match(/unsupported field\s+(.+?)\s+in\s+/i)
  if (unsupportedFieldMatch?.[1]) return normalizeDiagnosticFieldPath(unsupportedFieldMatch[1])
  const invalidEnumMatch = normalized.match(/invalid enum value\s+.+?\s+for\s+.+\.([^.\s]+)$/i)
  if (invalidEnumMatch?.[1]) return normalizeDiagnosticFieldPath(invalidEnumMatch[1])
  return ''
}

function resolveAttemptDiagnosticBreakpoint(attempt: CandidateRecord, rawMessageOverride = ''): string {
  const extra = extractObject(attempt.extra_data)
  const failureDiagnostic = extractVisibleFailureDiagnostic(extra)
  const requestConversionError = extractObject(extra?.request_conversion_error)
  const requestBodyBuildError = extractObject(extra?.request_body_build_error)
  const errorFlow = extractObject(extra?.error_flow)
  const rawMessage = [
    rawMessageOverride,
    readStringField(errorFlow ?? {}, 'message') ?? '',
    readStringField(failureDiagnostic ?? {}, 'message') ?? '',
    readStringField(requestConversionError ?? {}, 'message') ?? '',
    readStringField(requestBodyBuildError ?? {}, 'message') ?? '',
    typeof attempt.error_message === 'string' ? attempt.error_message : '',
  ].find(item => item.trim()) ?? ''

  return diagnosticPathFromObject(failureDiagnostic)
    || diagnosticPathFromObject(requestConversionError)
    || diagnosticPathFromObject(requestBodyBuildError)
    || diagnosticFieldPathFromMessage(rawMessage)
    || '$'
}

function buildAttemptDiagnosticPayload(
  attempt: CandidateRecord,
  summaryInput: string,
  statusCode: number | undefined,
  upstreamResponseDisplay: Record<string, unknown> | null,
  rawMessageForBreakpoint = '',
): Record<string, unknown> | null {
  const extra = extractObject(attempt.extra_data)
  const rawFailureDiagnostic = extractVisibleFailureDiagnostic(extra)
  const rawRequestConversionError = extractObject(extra?.request_conversion_error)
  const rawRequestBodyBuildError = extractObject(extra?.request_body_build_error)
  const hasDiagnostic = Boolean(
    summaryInput
      || rawFailureDiagnostic
      || rawRequestConversionError
      || rawRequestBodyBuildError
      || attempt.error_message
      || attempt.error_type
      || attempt.skip_reason
      || upstreamResponseDisplay,
  )
  if (!hasDiagnostic) return null

  const summary = summaryInput
    || readStringField(rawFailureDiagnostic ?? {}, 'message')
    || readStringField(rawRequestConversionError ?? {}, 'message')
    || readStringField(rawRequestBodyBuildError ?? {}, 'message')
    || (typeof attempt.error_message === 'string' ? formatAttemptErrorMessage(attempt.error_message, attempt.status_code) : '')
    || attempt.error_type
    || currentAttemptSkipReasonDisplay.value
    || '未知失败'
  const breakpoint = resolveAttemptDiagnosticBreakpoint(attempt, rawMessageForBreakpoint)
  const providerFormat = readStringField(extra ?? {}, 'provider_api_format')
  const clientFormat = readStringField(extra ?? {}, 'client_api_format')
    || (typeof props.requestApiFormat === 'string' ? props.requestApiFormat : '')
  const conversionDisplay = clientFormat || providerFormat
    ? `${clientFormat ? formatApiFormat(clientFormat) : '未知请求格式'} → ${providerFormat ? formatApiFormat(providerFormat) : '未知上游格式'}`
    : currentAttemptFormatDisplay.value
  const analysisHint = (() => {
    const raw = `${summary}\n${rawMessageForBreakpoint}\n${attempt.error_message ?? ''}`.toLowerCase()
    if (raw.includes('unsupported provider stream event')) {
      return '断点在上游流式事件解析/转换矩阵：先按 breakpoint 对应字段确认 event type，再决定是补 canonical mapping 还是加入 known noop。'
    }
    if (raw.includes('finish reason')) {
      return '断点在 finish_reason 映射：确认该结束原因是否可等价映射；不能无损映射时保持失败闭合。'
    }
    if (raw.includes('lossy conversion') || raw.includes('无损') || raw.includes('丢失信息') || raw.includes('request_conversion')) {
      return '断点在请求/响应格式转换器：检查 breakpoint 字段是否能被目标格式表达，不能表达就需要拒绝、降级或新增显式映射策略。'
    }
    return '先从 breakpoint 字段开始回放；若 breakpoint 为 $，优先查看 raw.failure_diagnostic / raw.error_message 和 upstream_response。'
  })()

  const payload = {
    summary,
    breakpoint,
    analysis_hint: analysisHint,
    request: {
      request_id: trace.value?.request_id ?? attempt.request_id,
      path: currentAttemptRequestPathDisplay.value || null,
      format_conversion: conversionDisplay || null,
      client_api_format: clientFormat || null,
      provider_api_format: providerFormat || null,
      needs_conversion: extra?.needs_conversion ?? null,
      conversion_mode: readStringField(extra ?? {}, 'conversion_mode') ?? null,
    },
    node: {
      candidate_id: attempt.id,
      candidate_index: attempt.candidate_index,
      retry_index: attempt.retry_index,
      provider: attempt.provider_name || attempt.provider_id || null,
      key: currentAttemptKeyDisplay.value || null,
      status: attempt.status,
      status_code: statusCode ?? attempt.status_code ?? null,
      skip_reason: attempt.skip_reason ?? null,
      error_type: attempt.error_type ?? null,
      error_message: attempt.error_message ?? null,
    },
    raw: {
      failure_diagnostic: rawFailureDiagnostic,
      request_conversion_error: rawRequestConversionError,
      request_body_build_error: rawRequestBodyBuildError,
      error_flow: extractObject(extra?.error_flow),
      upstream_response: upstreamResponseDisplay ?? normalizeUpstreamResponseDisplay(extra?.upstream_response),
    },
  }
  return payload
}

const currentAttemptExtraDataDisplay = computed<Record<string, unknown> | null>(() => {
  const extra = extractObject(currentAttempt.value?.extra_data)
  if (!extra) return null

  const display = { ...extra }
  delete display.upstream_response
  delete display.error_flow
  delete display.client_response
  delete display.provider_response

  return Object.keys(display).length > 0 ? display : null
})

const hasActiveImageProgress = computed(() => {
  return rawTimeline.value.some((candidate) => {
    const progress = normalizeImageProgress(candidate.image_progress)
      ?? normalizeImageProgress(extractObject(candidate.extra_data)?.image_progress)
    if (!progress?.phase) return false
    return progress.phase !== 'upstream_completed' && progress.phase !== 'failed'
  })
})

// 格式化认证类型
const formatAuthTypeWithPlan = (authType: string): string => {
  const labels: Record<string, string> = {
    'bearer': 'Bearer Token',
  }
  return labels[authType] || authType
}

const TERMINAL_ATTEMPT_STATUSES = ['failed', 'cancelled', 'stream_interrupted', 'skipped']

const selectMostRelevantAttempt = (attempts: CandidateRecord[]) => {
  const success = attempts.findIndex(attempt => getDisplayStatus(attempt) === 'success')
  const active = attempts.findIndex(isLiveCandidate)
  const terminal = attempts.map(attempt => TERMINAL_ATTEMPT_STATUSES.includes(getDisplayStatus(attempt))).lastIndexOf(true)
  selectedAttemptIndex.value = success >= 0 ? success : active >= 0 ? active : Math.max(terminal, 0)
}

const navigateAttempt = (direction: number) => {
  const count = timeline.value.length
  if (count < 2) return
  selectionPinnedByUser.value = true
  selectedAttemptIndex.value = (selectedAttemptIndex.value + direction + count) % count
}

const handlePagerKeydown = (event: KeyboardEvent) => {
  if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
  const target = event.target as HTMLElement | null
  if (target?.closest('input, textarea, select, [contenteditable="true"], pre, code')) return
  event.preventDefault()
  navigateAttempt(event.key === 'ArrowLeft' ? -1 : 1)
}

// 加载请求追踪数据
const loadTrace = async (silent = false) => {
  if (!props.requestId || props.traceData) return
  const requestId = props.requestId
  const version = traceLoadVersion
  if (traceLoadInFlight) return traceLoadInFlight

  traceLoadInFlight = (async () => {
    traceLoadStarted.value = true

    if (!silent) {
      loading.value = true
    }
    error.value = null

    try {
      const result = await requestTraceApi.getRequestTrace(requestId, { attemptedOnly: true })
      if (version !== traceLoadVersion) return
      internalTrace.value = result
    } catch (err: unknown) {
      if (version !== traceLoadVersion) return
      if (isAxiosError(err) && err.response?.status === 404) {
        internalTrace.value = null
        error.value = null
        return
      }
      if (!silent) {
        error.value = parseApiError(err, '加载失败')
      }
      log.error('加载请求追踪失败:', err)
    } finally {
      if (version === traceLoadVersion) {
        if (!silent) loading.value = false
        traceLoadInFlight = null
      }
    }
  })()

  return traceLoadInFlight
}

const propsRequestIsActive = computed(() => {
  const status = props.requestStatus ?? usageData.value?.status
  return status === 'pending' || status === 'streaming'
})

const traceHasActiveCandidate = computed(() => {
  return rawTimeline.value.some(isLiveCandidate)
})

const traceFinalIsTerminal = computed(() => {
  const status = trace.value?.final_status
  return status === 'success' || status === 'failed' || status === 'cancelled'
})

const shouldPollTrace = computed(() => {
  if (!props.requestId || props.traceData) return false
  if (traceHasActiveCandidate.value || hasActiveImageProgress.value) return true
  return propsRequestIsActive.value && !traceFinalIsTerminal.value
})

const stopTracePolling = () => {
  if (tracePollTimer) {
    clearTimeout(tracePollTimer)
    tracePollTimer = null
  }
}

const scheduleTracePolling = () => {
  stopTracePolling()
  if (!shouldPollTrace.value) return

  tracePollTimer = setTimeout(async () => {
    await loadTrace(true)
    scheduleTracePolling()
  }, TRACE_POLL_INTERVAL_MS)
}

// Preserve manually selected identity across refresh/reordering, while automatic
// selection continues to follow a live or successful attempt.
watch(timeline, (attempts, previous) => {
  const previousAttempt = previous?.[selectedAttemptIndex.value]
  const sameRequest = previousAttempt?.request_id === attempts[0]?.request_id
  if (selectionPinnedByUser.value && sameRequest && previousAttempt) {
    const index = attempts.findIndex(attempt => attempt.id === previousAttempt.id)
    if (index >= 0) {
      selectedAttemptIndex.value = index
      return
    }
  }
  if (!sameRequest) selectionPinnedByUser.value = false
  selectMostRelevantAttempt(attempts)
}, { immediate: true })

watch(
  [() => props.requestId, () => props.traceData],
  ([requestId], previous) => {
    if (requestId !== previous?.[0] || props.traceData) {
      traceLoadVersion += 1
      traceLoadInFlight = null
      if (requestId !== previous?.[0]) {
        selectionPinnedByUser.value = false
        internalTrace.value = null
      }
    }
    traceLoadStarted.value = false
    if (props.traceData || !requestId) {
      internalTrace.value = null
      loading.value = false
      error.value = null
      return
    }
    void loadTrace()
  },
  { immediate: true },
)

watch(shouldPollTrace, () => {
  scheduleTracePolling()
}, { immediate: true })

onBeforeUnmount(() => {
  traceLoadVersion += 1
  stopTracePolling()
})

defineExpose({ refresh: () => loadTrace(true) })

const TERMINAL_TIME_RANGE_STATUSES = new Set([
  'success',
  'failed',
  'cancelled',
  'stream_interrupted',
])

const parseTimestampMs = (value?: string | null): number | null => {
  if (!value) return null
  const timestamp = new Date(value).getTime()
  return Number.isFinite(timestamp) ? timestamp : null
}

const normalizeLatencyMs = (value?: number | null): number | null => {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0) return null
  return Math.round(value)
}

const formatDurationMs = (durationMs: number): string => {
  const safeDurationMs = Math.max(0, Math.round(durationMs))
  if (safeDurationMs >= 1000) {
    return `${(safeDurationMs / 1000).toFixed(2)}s`
  }
  return `${safeDurationMs}ms`
}

const resolveAttemptTimeRange = (attempt: CandidateRecord | null | undefined): AttemptTimeRange | null => {
  if (!attempt?.started_at) return null

  const startMs = parseTimestampMs(attempt.started_at)
  if (startMs == null) {
    return {
      startIso: attempt.started_at,
      endIso: attempt.finished_at,
      durationLabel: attempt.finished_at ? formatDuration(attempt.started_at, attempt.finished_at) : undefined,
    }
  }

  const rawEndMs = parseTimestampMs(attempt.finished_at)
  const latencyMs = normalizeLatencyMs(attempt.latency_ms)
  let endMs = rawEndMs

  if (
    latencyMs != null &&
    latencyMs > 0 &&
    (endMs == null || endMs <= startMs) &&
    (attempt.finished_at || TERMINAL_TIME_RANGE_STATUSES.has(attempt.status))
  ) {
    endMs = startMs + latencyMs
  }

  if (endMs == null) {
    return { startIso: attempt.started_at }
  }

  return {
    startIso: attempt.started_at,
    endIso: new Date(endMs).toISOString(),
    durationLabel: formatDurationMs(endMs - startMs),
  }
}

// 格式化时间（详细）
const formatTime = (dateStr: string) => {
  const date = new Date(dateStr)
  const timeStr = date.toLocaleTimeString('zh-CN', {
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  })
  const ms = date.getMilliseconds().toString().padStart(3, '0')
  return `${timeStr}.${ms}`
}

// 格式化持续时间（开始到结束）
const formatDuration = (startStr: string, endStr: string): string => {
  const start = new Date(startStr).getTime()
  const end = new Date(endStr).getTime()
  return formatDurationMs(end - start)
}

// 获取状态标签
const getStatusLabel = (status: string) => {
  const labels: Record<string, string> = {
    available: '可用未尝试',
    unused: '未使用',
    pending: '进行中',
    streaming: '传输中',
    stream_interrupted: '流中断',
    success: '成功',
    failed: '失败',
    cancelled: '已取消',
    skipped: '未发送'
  }
  return labels[status] || status
}

// 获取状态颜色类
const getStatusColorClass = (status: string) => {
  const classes: Record<string, string> = {
    available: 'status-available',
    unused: 'status-available',
    pending: 'status-pending',
    streaming: 'status-pending',
    stream_interrupted: 'status-failed',
    success: 'status-success',
    failed: 'status-failed',
    cancelled: 'status-cancelled',
    skipped: 'status-skipped'
  }
  return classes[status] || 'status-available'
}

// 展示状态：进行中态优先（包括 started 但未 finished 的中间态），再按 HTTP 状态码兜底
function getDisplayStatus(attempt: CandidateRecord | null | undefined): string {
  if (!attempt) return 'available'
  const code = attempt.status_code
  const isTerminalSuccessCode = typeof code === 'number' && code >= 200 && code < 300

  if (attempt.status === 'success') {
    if (typeof code === 'number' && !isTerminalSuccessCode) {
      return 'failed'
    }
    return 'success'
  }
  if (
    attempt.status === 'failed' ||
    attempt.status === 'cancelled' ||
    attempt.status === 'skipped' ||
    attempt.status === 'stream_interrupted'
  ) {
    return attempt.status
  }
  const hasFinished = Boolean(attempt.finished_at)
  const isExplicitPending = (attempt.status === 'pending' || attempt.status === 'streaming') && !hasFinished
  const isImplicitPending = Boolean(
    attempt.started_at &&
      !hasFinished &&
      !['failed', 'cancelled', 'skipped', 'stream_interrupted'].includes(attempt.status),
  )

  if (isExplicitPending || isImplicitPending) {
    return 'pending'
  }
  if (typeof code === 'number') {
    if (isTerminalSuccessCode) return 'success'
    if (code >= 300) return 'failed'
  }
  return attempt.status
}
</script>

<style scoped>
.minimal-request-timeline {
  width: 100%;
  container-type: inline-size;
}
/* Nodes and retries reflow together; sequence numbers preserve order across rows. */
/* 详情面板 */
.detail-panel {
  margin-top: 0.5rem;
  min-width: 0;
}

.panel-header {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  gap: 0.5rem;
  align-items: center;
  justify-content: space-between;
  padding: 0.5rem 0rem;

}

.panel-title {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.625rem;
}

.title-dot {
  flex: none;
  width: 7px;
  height: 7px;
  border-radius: 50%;
}

.title-dot.status-success { background: #22c55e; }
.title-dot.status-failed { background: #ef4444; }
.title-dot.status-cancelled { background: #f59e0b; }
.title-dot.status-pending { background: #3b82f6; }
.title-dot.status-skipped { background: var(--muted-foreground); }
.title-dot.status-available { background: #d1d5db; }

.title-text {
  font-weight: 600;
  font-size: 0.95rem;
  overflow-wrap: anywhere;
}

.panel-body {
  padding: 0.75rem 0rem;
}

.status-tag { display: inline-flex; align-items: center; flex-wrap: wrap; gap: 0.5rem; font-size: 0.75rem; color: var(--muted-foreground); }
.attempt-http { font-size: 0.7rem; font-variant-numeric: tabular-nums; }
.final-status { white-space: nowrap; display: inline-flex; align-items: center; gap: 0.4rem; font-size: 0.75rem; color: var(--muted-foreground); }
.final-status.status-failed { color: #dc414c; }
.attempt-latency { margin-left: auto; color: var(--muted-foreground); font-size: 0.75rem; }
.trace-card { position: relative; padding: 1.125rem 2.75rem 0.75rem; }
.trace-pagination { text-align: center; color: var(--muted-foreground); font-size: 0.75rem; font-variant-numeric: tabular-nums; padding-top: 0.75rem; }
.trace-edge { position: absolute; top: 3.25rem; bottom: 2.5rem; width: 44px; display: flex; align-items: center; justify-content: center; background: transparent; border: 0; cursor: pointer; }
.trace-edge-prev { left: 0; }
.trace-edge-next { right: 0; }
.trace-edge span { display: grid; place-items: center; width: 32px; height: 62px; border: 1px solid var(--border); border-radius: 12px; background: var(--card); color: var(--muted-foreground); box-shadow: 0 4px 14px #26395410; opacity: 0; transition: opacity 150ms, color 150ms; }
.trace-edge:hover span, .trace-edge:focus-visible span { opacity: 1; color: var(--primary); }
.trace-edge:focus-visible { outline: none; }
.trace-edge:focus-visible span { outline: 2px solid var(--primary); outline-offset: 2px; }
@media (hover: none), (pointer: coarse) { .trace-edge span { opacity: 1; } }
@media (prefers-reduced-motion: reduce) { .trace-edge span { transition: none; } }
@media (max-width: 640px) { .trace-card { padding-inline: 2.75rem; } }

@container (max-width: 560px) {
  .info-grid { grid-template-columns: minmax(0, 1fr) !important; }
}

.info-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.625rem 1.25rem;
}

.info-item {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.info-label {
  font-size: 0.7rem;
  color: var(--muted-foreground);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  font-weight: 500;
}

.info-value {
  overflow-wrap: anywhere;
  flex-wrap: wrap;
  font-size: 0.9rem;
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.info-value.highlight {
  font-size: 1.1rem;
  font-weight: 600;
  font-family: ui-monospace, monospace;
  color: hsl(var(--primary));
}

.info-value code {
  font-size: 0.7rem;
  padding: 0.15rem 0.375rem;
  background: hsl(var(--muted));
  border-radius: 4px;
  color: var(--muted-foreground);
  font-family: ui-monospace, monospace;
}

/* 两行堆叠布局 */
.info-value-stacked {
  flex-direction: column;
  align-items: flex-start;
  gap: 0.2rem;
}

/* 格式代码 */
.format-code {
  font-size: 0.75rem;
  padding: 0.1rem 0.3rem;
  background: hsl(var(--muted));
  border-radius: 3px;
  color: var(--muted-foreground);
  font-family: ui-monospace, monospace;
}

/* Key 信息 */
.key-name {
  font-weight: 500;
}

.key-preview {
  font-size: 0.75rem;
  padding: 0.1rem 0.3rem;
  background: hsl(var(--muted));
  border-radius: 3px;
  color: var(--muted-foreground);
  font-family: ui-monospace, monospace;
}

/* 认证类型标签 */
.auth-type-tag {
  display: inline-flex;
  align-items: center;
  padding: 0.1rem 0.35rem;
  margin-left: 0.375rem;
  font-size: 0.65rem;
  font-weight: 500;
  color: hsl(var(--primary) / 0.8);
  background: hsl(var(--primary) / 0.08);
  border: 1px solid hsl(var(--primary) / 0.2);
  border-radius: 3px;
}

/* 代理信息 */
.proxy-name {
  font-weight: 500;
}

.proxy-detail {
  display: flex;
  align-items: center;
  gap: 0.375rem;
}

.image-progress-block {
  margin-top: 0.875rem;
  padding: 0.75rem;
  border: 1px solid hsl(var(--border) / 0.7);
  border-radius: 8px;
  background: hsl(var(--background) / 0.72);
}

.image-progress-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  margin-bottom: 0.625rem;
}

.image-progress-title {
  font-size: 0.82rem;
  font-weight: 600;
}

.image-progress-phase {
  display: inline-flex;
  align-items: center;
  padding: 0.15rem 0.5rem;
  border-radius: 999px;
  font-size: 0.7rem;
  font-weight: 600;
  white-space: nowrap;
  border: 1px solid hsl(var(--border));
}

.image-progress-phase.phase-connecting,
.image-progress-phase.phase-streaming {
  color: #2563eb;
  background: #3b82f614;
  border-color: #3b82f633;
}

.image-progress-phase.phase-completed {
  color: #16a34a;
  background: #22c55e14;
  border-color: #22c55e33;
}

.image-progress-phase.phase-failed {
  color: #dc2626;
  background: #ef444414;
  border-color: #ef444433;
}

.image-progress-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 0.625rem 0.875rem;
}

.image-progress-item {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.image-progress-item.full-width {
  grid-column: span 2;
}

.image-progress-label {
  font-size: 0.68rem;
  color: var(--muted-foreground);
  white-space: nowrap;
}

.image-progress-value {
  min-width: 0;
  font-size: 0.82rem;
  font-weight: 600;
  color: hsl(var(--foreground));
}

.image-progress-code {
  min-width: 0;
  width: fit-content;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding: 0.12rem 0.35rem;
  border-radius: 4px;
  background: hsl(var(--muted));
  color: var(--muted-foreground);
  font-size: 0.72rem;
  font-family: ui-monospace, monospace;
}

@media (max-width: 768px) {
  .image-progress-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .image-progress-item.full-width {
    grid-column: 1 / -1;
  }
}

/* Provider 官网链接 */
.provider-link {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0.25rem;
  margin-left: 0.25rem;
  color: var(--muted-foreground);
  border-radius: 4px;
  transition: all 0.15s ease;
}

.provider-link:hover {
  color: hsl(var(--primary));
  background: hsl(var(--primary) / 0.1);
}

/* 时间范围 */
.time-range {
  margin-top: 1.25rem;
  padding-top: 1rem;
  border-top: 1px dashed hsl(var(--border));
  display: flex;
  flex-direction: column;
  gap: 0.375rem;
}

.time-label {
  font-size: 0.7rem;
  color: var(--muted-foreground);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  font-weight: 500;
}

.time-value {
  font-size: 0.85rem;
  font-family: ui-monospace, monospace;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.time-arrow {
  color: var(--muted-foreground);
}

/* 时间范围值 - 紧凑布局 */
.time-range-value {
  gap: 0.25rem !important;
}

/* 用量区域 */
.usage-section {
  margin-top: 0.75rem;
  padding-top: 0.75rem;
  border-top: 1px dashed hsl(var(--border));
}

.usage-grid {
  display: flex;
  flex-direction: column;
  gap: 0.375rem;
  padding: 0.5rem 0.75rem;
  background: hsl(var(--muted) / 0.2);
  border: 1px solid hsl(var(--border) / 0.5);
  border-radius: 8px;
}

.usage-row {
  display: flex;
  align-items: center;
}

.usage-item {
  display: flex;
  align-items: center;
  flex: 1;
}

.usage-label {
  font-size: 0.75rem;
  color: var(--muted-foreground);
  width: 56px;
  flex-shrink: 0;
}

.usage-tokens {
  font-size: 0.875rem;
  font-weight: 600;
  font-family: ui-monospace, monospace;
  width: 60px;
  flex-shrink: 0;
}

.usage-cost {
  font-size: 0.75rem;
  color: #16a34a;
  font-family: ui-monospace, monospace;
}

.dark .usage-cost {
  color: #4ade80;
}

.usage-divider {
  width: 1px;
  height: 16px;
  background: hsl(var(--border));
  margin: 0 1rem;
}

/* 跳过原因 */
.skip-reason {
  margin-top: 1rem;
  padding: 0.75rem;
  background: hsl(var(--muted) / 0.5);
  border-radius: 8px;
  display: flex;
  gap: 0.75rem;
  font-size: 0.85rem;
}

.reason-label {
  color: var(--muted-foreground);
  flex-shrink: 0;
}

.reason-value {
  color: hsl(var(--foreground));
}

.reason-content {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.reason-detail {
  color: var(--muted-foreground);
  line-height: 1.45;
  word-break: break-word;
}

.reason-detail code {
  margin-right: 0.4rem;
  padding: 0.1rem 0.35rem;
  border-radius: 4px;
  background: hsl(var(--background) / 0.8);
  color: hsl(var(--foreground));
  font-size: 0.8rem;
}

/* 额外信息 */
.extra-block {
  margin-top: 0.25rem;
}

.extra-toggle {
  font-size: 0.8rem;
  color: var(--muted-foreground);
  cursor: pointer;
  padding: 0.5rem 0;
  user-select: none;
}

.extra-toggle:hover {
  color: hsl(var(--foreground));
}

.extra-json-panel {
  margin-top: 0.5rem;
}

/* 动画 */
.slide-up-enter-active,
.slide-up-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}

.slide-up-enter-from,
.slide-up-leave-to {
  opacity: 0;
  transform: translateY(10px);
}
@media (prefers-reduced-motion: reduce) {
  .slide-up-enter-active, .slide-up-leave-active { transition: none; }
}
</style>
