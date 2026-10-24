<template>
  <div class="minimal-request-timeline" :class="{ 'is-dark': isDark }">
    <!-- 执行瀑布：每行表达一次 Key 调用，横轴表示相对时间，详情只在所选行下方展开。 -->
    <div
      v-if="loading"
      class="execution-waterfall execution-waterfall--loading"
    >
      <Skeleton class="h-40 w-full" />
    </div>

    <div
      v-else-if="error"
      class="execution-waterfall execution-waterfall--error"
      role="alert"
    >
      {{ error }}
    </div>

    <section
      v-else-if="trace && trace.candidates.length > 0"
      class="execution-waterfall"
      data-request-trace-waterfall
    >
      <header class="waterfall-header">
        <div class="waterfall-heading">
          <h4 class="waterfall-title">
            执行路径
          </h4>
          <span class="waterfall-duration" data-trace-latency>{{ formatLatency(totalTraceLatency) }}</span>
        </div>
        <p
          class="waterfall-summary"
          data-route-summary
        >
          <span>{{ providerRounds.length }} 家供应商</span>
          <span>{{ attemptedCallCount }} 次上游请求</span>
          <span v-if="cooledDownKeyCount > 0">{{ cooledDownKeyCount }} 次冷却跳过</span>
        </p>
      </header>

      <div
        class="waterfall-axis"
        aria-hidden="true"
      >
        <span>Key</span>
        <span class="waterfall-axis-track">
          <span>0</span>
          <span>{{ formatLatency(totalTraceLatency / 4) }}</span>
          <span>{{ formatLatency(totalTraceLatency / 2) }}</span>
          <span>{{ formatLatency(totalTraceLatency * 3 / 4) }}</span>
          <span>{{ formatLatency(totalTraceLatency) }}</span>
        </span>
      </div>

      <div class="waterfall-groups">
        <section
          v-for="(round, roundIndex) in providerRounds"
          :key="round.providerId"
          class="waterfall-provider provider-record"
          :data-provider-id="round.providerId"
        >
          <header class="waterfall-provider-header">
            <span class="waterfall-provider-order">{{ String(roundIndex + 1).padStart(2, '0') }}</span>
            <span class="waterfall-provider-name provider-record-name">{{ round.providerName }}</span>
            <span class="waterfall-provider-meta provider-record-count">{{ formatWaterfallProviderMeta(round) }}</span>
          </header>

          <div class="waterfall-rows" role="list" :aria-label="`${round.providerName} 的 Key 调用`">
            <div
              v-for="(attempt, keyIndex) in round.attempts"
              :key="attempt.id"
              class="waterfall-item"
              :class="{ 'is-open': openedAttemptId === attempt.id }"
              role="listitem"
              :data-attempt-id="attempt.id"
              :data-status="getTraceLineStatus(attempt)"
            >
              <button
                type="button"
                class="waterfall-row attempt-stop key-record"
                :class="getStatusColorClass(getDisplayStatus(attempt))"
                :aria-expanded="openedAttemptId === attempt.id"
                :aria-label="`${getAttemptKeyDisplay(attempt)}，${formatAttemptOutcome(attempt)}，查看详情`"
                @click="selectAttempt(attempt)"
              >
                <span class="waterfall-label">
                  <span
                    class="waterfall-status-mark"
                    aria-hidden="true"
                  />
                  <span class="waterfall-key">{{ getAttemptKeyDisplay(attempt) }}</span>
                  <span
                    v-if="getAttemptTraceDescription(attempt)"
                    class="waterfall-row-note"
                  >{{ getAttemptTraceDescription(attempt) }}</span>
                </span>

                <span class="waterfall-track">
                  <span
                    v-if="getDisplayStatus(attempt) === 'skipped'"
                    class="waterfall-not-sent"
                    :style="{ left: getWaterfallPositionPercent(attempt, 0) }"
                  >{{ attempt.skip_reason === 'key_circuit_open' ? '冷却跳过' : '未发送' }}</span>
                  <span
                    v-else
                    class="waterfall-bar"
                    :class="getStatusColorClass(getDisplayStatus(attempt))"
                    :style="getWaterfallBarStyle(attempt)"
                  />
                      <span
                        v-if="getDisplayStatus(attempt) !== 'skipped'"
                        class="waterfall-row-duration"
                        :class="{ 'is-inside': Number.parseFloat(getWaterfallEndPercent(attempt)) >= 86 }"
                        :style="{ left: getWaterfallEndPercent(attempt) }"
                        :data-tone="getTraceLineStatus(attempt)"
                      >{{ formatLatency(attempt.latency_ms) }}</span>
                </span>
              </button>

              <section
                v-if="openedAttemptId === attempt.id"
                class="waterfall-detail"
                :data-execution-detail="attempt.id"
                :data-execution-inspector="attempt.id"
                :data-attempt-id="attempt.id"
                :data-status="getTraceLineStatus(attempt)"
                aria-live="polite"
              >
                <div class="waterfall-detail-heading">
                  <div class="waterfall-detail-identity">
                    <span class="waterfall-detail-provider">{{ getProviderDisplayName(attempt) }}</span>
                    <strong class="waterfall-detail-key">{{ getAttemptKeyDisplay(attempt) }}</strong>
                  </div>
                  <dl class="waterfall-detail-facts">
                    <div v-if="getDisplayStatus(attempt) !== 'skipped'">
                      <dt>HTTP</dt>
                      <dd class="mono" data-attempt-http-status>{{ attempt.status_code ?? (getDisplayStatus(attempt) === 'success' ? 200 : '-') }}</dd>
                    </div>
                    <div>
                      <dt>耗时</dt>
                      <dd class="mono">{{ getAttemptTimeRange(attempt)?.durationLabel || formatLatency(attempt.latency_ms) }}</dd>
                    </div>
                    <div v-if="getAttemptTimeRange(attempt)" class="waterfall-detail-time">
                      <dt>时间</dt>
                      <dd class="mono">
                        {{ formatTime(getAttemptTimeRange(attempt)!.startIso) }}
                        <span class="waterfall-time-arrow">→</span>
                        {{ getAttemptTimeRange(attempt)?.endIso ? formatTime(getAttemptTimeRange(attempt)!.endIso!) : '进行中' }}
                      </dd>
                    </div>
                    <div v-if="attempt.key_preview">
                      <dt>标识</dt>
                      <dd class="mono">{{ attempt.key_preview }}</dd>
                    </div>
                  </dl>
                </div>

                <div v-if="currentDetailImageProgress" class="image-progress-block">
                  <div class="image-progress-header">
                    <span class="image-progress-title">图片生成进度</span>
                    <span class="image-progress-phase" :class="imageProgressPhaseClass(currentDetailImageProgress.phase)">
                      {{ formatImageProgressPhase(currentDetailImageProgress.phase) }}
                    </span>
                  </div>
                  <div class="image-progress-grid">
                    <div class="image-progress-item">
                      <span class="image-progress-label">上游 TTFB</span>
                      <span class="image-progress-value mono">{{ formatLatency(currentDetailImageProgress.upstream_ttfb_ms) }}</span>
                    </div>
                    <div class="image-progress-item">
                      <span class="image-progress-label">SSE 帧数</span>
                      <span class="image-progress-value mono">{{ formatProgressCount(currentDetailImageProgress.upstream_sse_frame_count) }}</span>
                    </div>
                    <div class="image-progress-item">
                      <span class="image-progress-label">Partial 图片</span>
                      <span class="image-progress-value mono">{{ formatProgressCount(currentDetailImageProgress.partial_image_count) }}</span>
                    </div>
                    <div class="image-progress-item">
                      <span class="image-progress-label">最后帧</span>
                      <span class="image-progress-value mono">{{ formatProgressFrameTime(currentDetailImageProgress.last_upstream_frame_at_unix_ms) }}</span>
                    </div>
                    <template v-if="hasDetailDownstreamHeartbeatProgress">
                      <div class="image-progress-item">
                        <span class="image-progress-label">下游心跳</span>
                        <span class="image-progress-value mono">{{ formatProgressCount(currentDetailImageProgress.downstream_heartbeat_count) }}</span>
                      </div>
                      <div class="image-progress-item">
                        <span class="image-progress-label">心跳间隔</span>
                        <span class="image-progress-value mono">{{ formatLatency(currentDetailImageProgress.downstream_heartbeat_interval_ms) }}</span>
                      </div>
                      <div class="image-progress-item">
                        <span class="image-progress-label">最后心跳</span>
                        <span class="image-progress-value mono">{{ formatProgressFrameTime(currentDetailImageProgress.last_downstream_heartbeat_at_unix_ms) }}</span>
                      </div>
                    </template>
                    <div v-if="currentDetailImageProgress.last_upstream_event" class="image-progress-item full-width">
                      <span class="image-progress-label">上游事件</span>
                      <code class="image-progress-code">{{ currentDetailImageProgress.last_upstream_event }}</code>
                    </div>
                    <div v-if="currentDetailImageProgress.last_client_visible_event" class="image-progress-item full-width">
                      <span class="image-progress-label">客户端可见事件</span>
                      <code class="image-progress-code">{{ currentDetailImageProgress.last_client_visible_event }}</code>
                    </div>
                  </div>
                </div>

                <div v-if="attempt.status === 'success' && usageData" class="usage-section">
                  <span class="usage-label">输入 {{ formatNumber(usageData.tokens.input) }}</span>
                  <span class="usage-label">输出 {{ formatNumber(usageData.tokens.output) }}</span>
                  <span class="usage-label">缓存创建 {{ formatNumber(usageData.tokens.cache_creation || 0) }}</span>
                  <span class="usage-label">缓存读取 {{ formatNumber(usageData.tokens.cache_read || 0) }}</span>
                </div>

                <RequestAttemptErrorPanel
                  v-if="getAttemptError(attempt)"
                  :key="attempt.id"
                  :error="getAttemptError(attempt)!"
                  :is-dark="isDark"
                />
              </section>
            </div>
          </div>
        </section>
      </div>
    </section>

    <div
      v-else
      class="execution-waterfall execution-waterfall--empty"
    >
      暂无追踪数据
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, onBeforeUnmount } from 'vue'
import { isAxiosError } from 'axios'
import Skeleton from '@/components/ui/skeleton.vue'
import RequestAttemptErrorPanel from './RequestAttemptErrorPanel.vue'
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

/** 同一供应商的 Key 调用分组及其在整体调度顺序中的位置。 */
interface ProviderRound {
  providerId: string
  providerName: string
  attempts: CandidateRecord[]
  attemptIndexes: number[]
  firstAttemptIndex: number
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
  /** 用量和费用数据 */
  usageData?: UsageData | null
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

const loading = ref(false)
const error = ref<string | null>(null)
const internalTrace = ref<RequestTrace | null>(null)
const { isDark } = useDarkMode()
const trace = computed(() => props.traceData ?? internalTrace.value)
const selectedAttemptIndex = ref(0)
/** 当前展开详情的记录 ID；刷新或重排时跟随记录身份，默认保持收起。 */
const openedAttemptId = ref<string | null>(null)
const selectionPinnedByUser = ref(false)
const selectedAttemptId = ref<string | null>(null)
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

// 保留 trace 中的实际调度顺序，并将同一供应商的所有 Key 调用聚合为一行。
const providerRounds = computed<ProviderRound[]>(() => {
  const rounds: ProviderRound[] = []
  const indexesByProvider = new Map<string, number>()

  timeline.value.forEach((attempt, attemptIndex) => {
    const providerId = String(attempt.provider_id || attempt.provider_name || '__unknown__')
    const existingIndex = indexesByProvider.get(providerId)
    if (existingIndex != null) {
      const round = rounds[existingIndex]
      round.attempts.push(attempt)
      round.attemptIndexes.push(attemptIndex)
      return
    }

    indexesByProvider.set(providerId, rounds.length)
    rounds.push({
      providerId,
      providerName: getProviderDisplayName(attempt),
      attempts: [attempt],
      attemptIndexes: [attemptIndex],
      firstAttemptIndex: attemptIndex,
    })
  })

  return rounds
})

/** 实际发出的上游调用数；本地冷却跳过项没有调用上游，不计入。 */
const attemptedCallCount = computed(() => (
  timeline.value.filter(attempt => attempt.status !== 'skipped').length
))

/** 本地冷却跳过的 Key 数，与实际调用数分开以免把未发送节点算成请求。 */
const cooledDownKeyCount = computed(() => (
  timeline.value.filter(attempt => attempt.skip_reason === 'key_circuit_open').length
))

/** Key 节点展示统一结果措辞；冷却中的未发送 Key 不再使用“尝试”描述。 */
function formatAttemptOutcome(attempt: CandidateRecord): string {
  if (attempt.skip_reason === 'key_circuit_open') return '冷却跳过，未发送'
  return getStatusLabel(getDisplayStatus(attempt))
}

/** 将现有候选状态映射为 shadcn TraceLine 的状态，不把业务状态文本塞进行内。 */
function getTraceLineStatus(attempt: CandidateRecord): 'failed' | 'success' | 'skipped' | 'pending' | undefined {
  const status = getDisplayStatus(attempt)
  if (status === 'failed' || status === 'cancelled' || status === 'stream_interrupted') return 'failed'
  if (status === 'success') return 'success'
  if (status === 'skipped') return 'skipped'
  if (isLiveCandidate(attempt)) return 'pending'
  return undefined
}

/** 只把冷却截止时间作为 TraceLine 的次要说明，不重复调用结果。 */
function getAttemptTraceDescription(attempt: CandidateRecord): string | undefined {
  const deadline = getAttemptCooldownDeadline(attempt)
  return deadline ? `冷却至 ${deadline}` : undefined
}

// 供应商行与 Key 的位置来自同一实际调度索引，避免聚合后详情错位。
const getProviderDisplayName = (attempt: CandidateRecord | null | undefined): string => {
  if (!attempt) return '未知'
  const providerName = String(attempt.provider_name || '').trim()
  if (providerName) return providerName
  return '未知'
}

/** 返回当前 Key 的名称；账号标签优先于内部名称，缺失时回退到 Key ID。 */
const getAttemptKeyDisplay = (attempt: CandidateRecord): string => {
  return attempt.key_account_label || attempt.key_name || attempt.key_id || '未知 Key'
}

// 总耗时优先取 trace 聚合值，包含本次请求的所有失败切换；单条候选耗时仍只描述该 Key。
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

// 当前选择始终指向一条实际候选记录，供应商行与 Key 节点共享这个调度索引。
const currentAttempt = computed(() => timeline.value[selectedAttemptIndex.value] ?? null)

/** 瀑布横轴统一使用 trace 聚合耗时；每条记录的相对起点和长度再由时间戳计算。 */
function getWaterfallPositionPercent(attempt: CandidateRecord, offsetMs = 0): string {
  const wallClockStart = rawTimeline.value
    .map(candidate => parseTimestampMs(candidate.started_at))
    .filter((value): value is number => value != null)
  const originMs = wallClockStart.length > 0 ? Math.min(...wallClockStart) : null
  const startMs = parseTimestampMs(attempt.started_at)
  const duration = totalTraceLatency.value
  if (originMs == null || startMs == null || duration <= 0) return '0%'
  return `${Math.max(0, Math.min(100, ((startMs - originMs + offsetMs) / duration) * 100))}%`
}

/** 将一次真实调用映射为瀑布条；缺少时间戳的回退到调度序号，不伪造精确时序。 */
function getWaterfallBarStyle(attempt: CandidateRecord): Record<string, string> {
  const duration = Math.max(0, normalizeLatencyMs(attempt.latency_ms) ?? 0)
  const total = totalTraceLatency.value
  const left = getWaterfallPositionPercent(attempt)
  const width = total > 0 ? Math.max(0.7, Math.min(100, (duration / total) * 100)) : 0.7
  return { left, width: `${width}%` }
}

function getWaterfallEndPercent(attempt: CandidateRecord): string {
  const total = totalTraceLatency.value
  const duration = Math.max(0, normalizeLatencyMs(attempt.latency_ms) ?? 0)
  if (total <= 0) return '0%'
  const start = Number.parseFloat(getWaterfallPositionPercent(attempt))
  return `${Math.min(97, start + (duration / total) * 100)}%`
}

/** 供应商分组摘要只陈述 Key 数和实际发送数。 */
function formatWaterfallProviderMeta(round: ProviderRound): string {
  const attempted = round.attempts.filter(attempt => getDisplayStatus(attempt) !== 'skipped').length
  return `${round.attempts.length} 个 Key · ${attempted} 次发送`
}

/** 展开行读取自身记录的时间范围，不受导航选择下标变化影响。 */
function getAttemptTimeRange(attempt: CandidateRecord): AttemptTimeRange | null {
  return resolveAttemptTimeRange(attempt)
}

watch(currentAttempt, (attempt) => {
  if (!selectionPinnedByUser.value || !selectedAttemptId.value) {
    selectedAttemptId.value = attempt?.id ?? null
  }
  emit('selectAttempt', attempt ?? null)
}, { immediate: true })

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

/** 展开行使用各自的图片进度，不读取另一个 Key 的活动状态。 */
function getAttemptImageProgress(attempt: CandidateRecord): ImageProgress | null {
  return normalizeImageProgress(attempt.image_progress)
    ?? normalizeImageProgress(extractObject(attempt.extra_data)?.image_progress)
}

const currentDetailImageProgress = computed<ImageProgress | null>(() => (
  currentAttempt.value ? getAttemptImageProgress(currentAttempt.value) : null
))

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

const hasDetailDownstreamHeartbeatProgress = computed(() => {
  const progress = currentDetailImageProgress.value
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

/** 从常见上游错误响应中提取实际消息，不把执行包装状态当作失败原因。 */
const extractUpstreamErrorDetail = (
  upstreamResponse: Record<string, unknown> | null,
): { message: string; type: string } => {
  const body = extractObject(upstreamResponse?.body)
  const error = extractObject(body?.error)
  const detail = extractObject(body?.detail)
  const firstDetail = Array.isArray(body?.detail) ? extractObject(body.detail[0]) : null
  const message = readStringField(error ?? {}, 'message')
    || readStringField(detail ?? {}, 'message')
    || readStringField(firstDetail ?? {}, 'msg')
    || readStringField(body ?? {}, 'message')
    || (typeof body?.detail === 'string' ? body.detail.trim() : '')
  const type = readStringField(error ?? {}, 'type')
    || readStringField(detail ?? {}, 'type')
    || readStringField(body ?? {}, 'type')
    || ''
  return { message, type }
}

/** 用上游 HTTP 语义和原文组成首层原因，避免同一状态显示固定套话。 */
const formatUpstreamReason = (
  statusCode: number | undefined,
  detail: { message: string; type: string },
): string => {
  if (!detail.message) return statusCode != null ? `上游返回 HTTP ${statusCode}` : '上游未返回可用的错误说明'
  const prefix = statusCode === 429
    ? '上游限流'
    : statusCode === 401 || statusCode === 403
      ? '上游鉴权失败'
      : statusCode != null
        ? `上游返回 HTTP ${statusCode}`
        : '上游执行失败'
  return `${prefix}：${detail.message}`
}

/** 提取常见的上游错误体消息；只有读取支持的上游返回原文，才用它替换包装错误。 */
const resolveTransportDiagnostics = (attempt: CandidateRecord): Record<string, unknown> | null => {
  const extra = extractObject(attempt.extra_data)
  return extractObject(extra?.transport_diagnostics)
}

const currentAttemptKeyDisplay = computed(() => {
  const attempt = currentAttempt.value
  if (!attempt) return ''
  return attempt.key_account_label || attempt.key_name || attempt.key_id || ''
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

  if (attempt.skip_reason === 'key_circuit_open') {
    return getAttemptCooldownDeadline(attempt)
      ? `本地冷却至 ${getAttemptCooldownDeadline(attempt)}`
      : '本地冷却中'
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

/**
 * 从 trace 已带回的诊断字段读取 Key 冷却截止时间。
 * 时间戳统一按 Unix 秒转换为本地时刻；字段缺失或越界时返回空字符串，避免猜造恢复时间。
 */
function getAttemptCooldownDeadline(attempt: CandidateRecord): string {
  const extra = extractObject(attempt.extra_data)
  const candidates = [
    extra?.rate_limit_cooldown_until_unix_secs,
    extra?.cooldown_until_unix_secs,
    extra?.retry_after_until_unix_secs,
  ]
  const raw = candidates.find(value => typeof value === 'number' && Number.isFinite(value) && value > 0)
  if (typeof raw !== 'number') return ''
  return new Date(raw * 1000).toLocaleTimeString('zh-CN', {
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

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

interface AttemptRequestErrorDisplay {
  message: string
  technicalMessage: string
  presentationSource: string
  statusCode?: number
  upstreamResponse: Record<string, unknown> | null
  skipReason?: string
  skipReasonLabel?: string
  skipped?: boolean
}

/** 解析指定 Key 的真实结果；每个瀑布行只使用自己的记录，不借用当前选中行。 */
function resolveAttemptRequestError(attempt: CandidateRecord): AttemptRequestErrorDisplay | null {
  const displayStatus = getDisplayStatus(attempt)
  if (!['failed', 'skipped', 'stream_interrupted'].includes(displayStatus)) return null

  const extra = extractObject(attempt.extra_data)
  const upstreamResponse = extractObject(extra?.upstream_response)
  const errorFlow = extractObject(extra?.error_flow)
  const statusCode = readNumberField(upstreamResponse ?? {}, 'status_code')
    ?? readNumberField(upstreamResponse ?? {}, 'statusCode')
    ?? readNumberField(errorFlow ?? {}, 'status_code')
    ?? readNumberField(errorFlow ?? {}, 'statusCode')
    ?? attempt.status_code
  const flowMessage = errorFlow ? readStringField(errorFlow, 'message') : ''
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
  const upstreamDetail = extractUpstreamErrorDetail(upstreamResponseDisplay)
  const upstreamResponseData: Record<string, unknown> = {}
  if (hasRenderableValue(upstreamResponseDisplay?.headers)) upstreamResponseData.header = upstreamResponseDisplay?.headers
  if (hasRenderableValue(upstreamResponseDisplay?.body)) upstreamResponseData.body = upstreamResponseDisplay?.body
  const response = Object.keys(upstreamResponseData).length > 0 ? upstreamResponseData : null
  if (!attempt.skip_reason && !message && statusCode == null && !response) return null

  const skipReasonLabel = attempt.skip_reason === 'key_circuit_open'
    ? getAttemptCooldownDeadline(attempt) ? `本地冷却至 ${getAttemptCooldownDeadline(attempt)}` : '本地冷却中'
    : SKIP_REASON_LABELS[attempt.skip_reason ?? ''] ?? attempt.skip_reason ?? ''
  const showMessage = shouldShowAttemptMessageWithUpstreamResponse(rawMessage || fallbackType, upstreamResponseDisplay)

  return {
    message: upstreamDetail.message
      ? formatUpstreamReason(statusCode, upstreamDetail)
      : showMessage || upstreamDetail.message
        ? (message || (attempt.status === 'skipped' ? skipReasonLabel : '未知错误'))
        : '',
    technicalMessage: upstreamDetail.message
      ? upstreamDetail.message
      : showMessage ? (rawMessage || fallbackType || attempt.skip_reason || '') : '',
    skipReason: attempt.skip_reason,
    skipReasonLabel,
    skipped: attempt.status === 'skipped',
    presentationSource: upstreamDetail.message || rawMessage || fallbackType || message,
    statusCode,
    upstreamResponse: response,
  }
}

/** 返回瀑布中指定 Key 的错误展示数据；没有真实错误或跳过原因时返回 null。 */
function getAttemptError(attempt: CandidateRecord): AttemptRequestErrorDisplay | null {
  return resolveAttemptRequestError(attempt)
}

const currentAttemptRequestError = computed<{
  message: string
  technicalMessage: string
  presentationSource: string
  statusCode?: number
  upstreamResponse: Record<string, unknown> | null
  skipReason?: string
  skipReasonLabel?: string
  skipped?: boolean
} | null>(() => currentAttempt.value ? resolveAttemptRequestError(currentAttempt.value) : null)

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

/** 点击 Key 时切换选中记录；再次点击同一 Key 收起其真实详情。 */
function selectAttempt(attempt: CandidateRecord): void {
  const index = timeline.value.findIndex(candidate => candidate.id === attempt.id)
  if (index < 0) return
  if (openedAttemptId.value === attempt.id) {
    openedAttemptId.value = null
    return
  }
  selectedAttemptIndex.value = index
  selectedAttemptId.value = attempt.id
  selectionPinnedByUser.value = true
  openedAttemptId.value = attempt.id
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
  if (selectionPinnedByUser.value && sameRequest && (selectedAttemptId.value || previousAttempt)) {
    const selectedId = selectedAttemptId.value ?? previousAttempt?.id
    const index = attempts.findIndex(attempt => attempt.id === selectedId)
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
        selectedAttemptId.value = null
        openedAttemptId.value = null
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
  min-width: 0;
  container-type: inline-size;
  font-family: var(--sans-serif);
}

.execution-waterfall {
  min-width: 0;
  overflow: hidden;
  border: 1px solid hsl(var(--border) / 0.6);
  border-radius: 0.75rem;
  background: hsl(var(--card) / 0.6);
}
.execution-waterfall--loading { padding: 0.7rem; }
.execution-waterfall--error { padding: 0.8rem 1rem; color: var(--destructive); font-size: 0.75rem; }
.execution-waterfall--empty { padding: 1rem; color: var(--muted-foreground); font-size: 0.75rem; text-align: center; }

.waterfall-header {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 0.55rem 1rem;
  padding: 0.78rem 0.86rem 0.68rem;
  border-bottom: 1px solid hsl(var(--border) / 0.52);
}
.waterfall-heading { display: flex; min-width: 0; align-items: baseline; gap: 0.55rem; }
.waterfall-title { margin: 0; font-size: 0.84rem; font-weight: 600; letter-spacing: -0.01em; }
.waterfall-duration { color: var(--muted-foreground); font-family: var(--monospace); font-size: 0.68rem; }
.waterfall-summary { display: flex; min-width: 0; flex-wrap: wrap; gap: 0.15rem 0.85rem; margin: 0; color: var(--muted-foreground); font-size: 0.66rem; font-variant-numeric: tabular-nums; }
.waterfall-summary span + span::before { margin-right: 0.85rem; color: hsl(var(--border) / 0.8); content: '·'; }

.waterfall-axis,
.waterfall-row {
  display: grid;
  grid-template-columns: minmax(7.5rem, 9.1rem) minmax(0, 1fr);
  column-gap: 0.72rem;
}
.waterfall-axis { padding: 0.56rem 0.86rem 0.38rem; color: hsl(var(--muted-foreground) / 0.82); font-family: var(--monospace); font-size: 0.6rem; }
.waterfall-axis-track { display: flex; justify-content: space-between; padding: 0 0.25rem; }
.waterfall-groups { padding: 0 0.86rem 0.58rem; }
.waterfall-provider { min-width: 0; padding-top: 0.3rem; }
.waterfall-provider + .waterfall-provider { margin-top: 0.3rem; border-top: 1px solid hsl(var(--border) / 0.48); }
.waterfall-provider-header { display: flex; min-width: 0; align-items: center; gap: 0.42rem; min-height: 1.74rem; padding: 0 0.2rem; }
.waterfall-provider-order { color: var(--muted-foreground); font-family: var(--monospace); font-size: 0.61rem; }
.waterfall-provider-name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 0.74rem; font-weight: 600; }
.waterfall-provider-meta { margin-left: auto; color: var(--muted-foreground); font-size: 0.64rem; white-space: nowrap; }

.waterfall-item { min-width: 0; }
.waterfall-row {
  width: 100%;
  align-items: center;
  min-height: 2.05rem;
  padding: 0.32rem 0.24rem;
  border: 0;
  border-radius: 0;
  background: transparent;
  color: var(--foreground);
  text-align: left;
  transition: color 150ms ease;
}
.waterfall-row:hover .waterfall-key { color: hsl(var(--foreground) / 0.86); }
.waterfall-row:focus-visible { outline: 1px solid hsl(var(--ring) / 0.44); outline-offset: 2px; }
.waterfall-item.is-open > .waterfall-row { background: transparent; }
.waterfall-label { display: grid; min-width: 0; grid-template-columns: 0.58rem minmax(0, 1fr); align-items: center; gap: 0.38rem; }
.waterfall-status-mark { width: 0.46rem; height: 0.46rem; border: 1.3px solid var(--muted-foreground); border-radius: 50%; }
.waterfall-row.status-failed .waterfall-status-mark { border-color: #bf565b; }
.waterfall-row.status-success .waterfall-status-mark { border-color: #71838d; }
.waterfall-row.status-pending .waterfall-status-mark { border-color: #4d80b8; }
.waterfall-row.status-cancelled .waterfall-status-mark { border-color: #a97832; }
.waterfall-row.status-skipped .waterfall-status-mark { border-color: #93a1b3; background: transparent; }
.waterfall-key { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 0.72rem; font-weight: 500; }
.waterfall-row-note { grid-column: 2; color: var(--muted-foreground); font-size: 0.6rem; line-height: 1.25; }
.waterfall-track { position: relative; display: block; min-width: 0; height: 1.2rem; border-radius: 0.25rem; background-image: repeating-linear-gradient(to right, transparent 0, transparent calc(25% - 1px), hsl(var(--border) / 0.65) 25%); }
.waterfall-bar { position: absolute; top: 0.31rem; min-width: 2px; height: 0.58rem; border-radius: 99px; }
.waterfall-bar.status-failed { background: #efdede; box-shadow: inset 0 0 0 1px #e4c3c5; }
.waterfall-bar.status-success { background: #dfe5e8; box-shadow: inset 0 0 0 1px #ccd6db; }
.waterfall-bar.status-pending { background: #dde8f3; box-shadow: inset 0 0 0 1px #bfd4e8; }
.waterfall-bar.status-cancelled { background: #efe5d5; box-shadow: inset 0 0 0 1px #e1d1b8; }
/* 耗时标签默认收在条内末端；只有条未接近右边界时才显示在条外，避免遮挡或溢出。 */
.waterfall-track { overflow: hidden; }
.waterfall-row-duration { position: absolute; top: 0.29rem; right: 0.18rem; color: hsl(var(--muted-foreground) / 0.95); font-family: var(--monospace); font-size: 0.58rem; white-space: nowrap; }
.waterfall-row-duration.is-inside { color: hsl(var(--foreground) / 0.72); }
.waterfall-not-sent { position: absolute; top: 0.3rem; color: var(--muted-foreground); font-size: 0.6rem; white-space: nowrap; }
.waterfall-detail { min-width: 0; margin: 0 0 0.5rem; padding: 0.68rem 0.24rem 0.68rem 1rem; border-top: 1px solid hsl(var(--border) / 0.48); background: transparent; }
.waterfall-detail-heading { display: grid; gap: 0.65rem; }
.waterfall-detail-identity { display: flex; min-width: 0; align-items: baseline; gap: 0.42rem; }
.waterfall-detail-provider { color: var(--muted-foreground); font-size: 0.66rem; }
.waterfall-detail-key { min-width: 0; overflow-wrap: anywhere; font-family: var(--monospace); font-size: 0.78rem; font-weight: 600; }
.waterfall-detail-facts { display: grid; min-width: 0; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 0.55rem 0.8rem; margin: 0; }
.waterfall-detail-facts > div { min-width: 0; display: grid; gap: 0.16rem; }
.waterfall-detail-facts dt { color: var(--muted-foreground); font-size: 0.62rem; }
.waterfall-detail-facts dd { margin: 0; overflow-wrap: anywhere; font-size: 0.72rem; font-weight: 500; }
.waterfall-detail-time { grid-column: span 2; }
.waterfall-time-arrow { margin: 0 0.2rem; color: var(--muted-foreground); }

.image-progress-block { margin-top: 0.7rem; padding-top: 0.65rem; border-top: 1px solid hsl(var(--border) / 0.7); }
.image-progress-header { display: flex; align-items: center; justify-content: space-between; gap: 0.7rem; margin-bottom: 0.55rem; }
.image-progress-title { font-size: 0.65rem; font-weight: 600; }
.image-progress-phase { padding: 0.1rem 0.35rem; border: 1px solid hsl(var(--border)); border-radius: 99px; font-size: 0.58rem; }
.image-progress-phase.phase-connecting, .image-progress-phase.phase-streaming { color: #3978b8; }
.image-progress-phase.phase-completed { color: #278461; }
.image-progress-phase.phase-failed { color: #c14b4b; }
.image-progress-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 0.48rem 0.7rem; }
.image-progress-item { display: grid; min-width: 0; gap: 0.13rem; }
.image-progress-item.full-width { grid-column: 1 / -1; }
.image-progress-label { color: var(--muted-foreground); font-size: 0.56rem; }
.image-progress-value { font-size: 0.65rem; font-weight: 500; }
.image-progress-code { min-width: 0; max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 0.08rem 0.25rem; border-radius: 0.2rem; background: hsl(var(--muted) / 0.65); color: var(--muted-foreground); font-family: var(--monospace); font-size: 0.65rem; }

.usage-section { display: flex; flex-wrap: wrap; gap: 0.4rem 0.85rem; margin-top: 0.7rem; padding-top: 0.65rem; border-top: 1px solid hsl(var(--border) / 0.7); }
.usage-label { color: var(--muted-foreground); font-size: 0.6rem; }
.mono { font-family: var(--monospace); font-variant-numeric: tabular-nums; }

/* 深色主题通过组件主题类应用同一语义状态色，避免 scoped :global 裁掉后代选择器。 */
.is-dark .execution-waterfall { background: hsl(var(--card) / 0.7); }
.is-dark .waterfall-track { background-image: repeating-linear-gradient(to right, transparent 0, transparent calc(25% - 1px), hsl(var(--border) / 0.82) 25%); }
.is-dark .waterfall-bar.status-failed { background: linear-gradient(90deg, #502b2f, #67373b); box-shadow: inset 0 0 0 1px #82494e; }
.is-dark .waterfall-bar.status-success { background: linear-gradient(90deg, #2b363c, #3a484f); box-shadow: inset 0 0 0 1px #52636c; }
.is-dark .waterfall-bar.status-pending { background: linear-gradient(90deg, #1c3448, #294a64); box-shadow: inset 0 0 0 1px #3d6484; }
.is-dark .waterfall-bar.status-cancelled { background: linear-gradient(90deg, #423422, #5b472c); box-shadow: inset 0 0 0 1px #76603e; }

@container (max-width: 420px) {
  .waterfall-axis, .waterfall-row { grid-template-columns: minmax(6.2rem, 7.9rem) minmax(0, 1fr); column-gap: 0.42rem; }
  .waterfall-header, .waterfall-axis, .waterfall-groups { padding-inline: 0.56rem; }
  .waterfall-provider-meta { display: none; }
  .waterfall-detail { padding-right: 0.18rem; padding-left: 0.88rem; margin-inline: 0; }
  .waterfall-detail-facts { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .waterfall-detail-time { grid-column: span 2; }
}
@media (prefers-reduced-motion: reduce) {
  .waterfall-row { transition: none; }
}
</style>
