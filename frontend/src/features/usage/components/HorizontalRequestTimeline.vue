<template>
  <div class="minimal-request-timeline" :class="{ 'is-dark': isDark }">
    <!-- 执行路径：按真实调度顺序横向展开请求入口、供应商 Key 和请求结果。 -->
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
      v-else-if="trace && providerRounds.length > 0"
      class="execution-waterfall"
      data-request-trace-waterfall
    >
      <div class="path-scroll" data-request-path-scroll>
        <div
          class="path-stage"
          :class="`path-stage--${responseState}`"
          :style="{ gridTemplateColumns: pathGridTemplate }"
        >
          <section
            class="path-node path-entry"
            data-request-path-entry
            aria-label="请求入口"
          >
            <span class="path-node-kicker">请求入口</span>
            <span class="path-entry-line">
              <span class="path-method">POST</span>
              <span class="path-endpoint" :title="requestPath">{{ requestPath }}</span>
            </span>
          </section>

          <span class="path-link path-link--in" aria-hidden="true" />

          <template v-for="(round, roundIndex) in providerRounds" :key="round.nodeKey">
            <span
              v-if="roundIndex > 0"
              class="path-link path-link--provider"
              aria-hidden="true"
            />

            <section
              class="path-provider provider-record"
              :class="`path-provider--tone-${round.colorTone}`"
              :data-provider-id="round.providerId"
              :data-provider-tone="round.colorTone"
              :aria-label="`供应商 ${round.order} ${round.providerName}`"
            >
              <header class="path-provider-label">
                <span class="waterfall-provider-order">{{ round.order }}</span>
                <span class="waterfall-provider-name provider-record-name">{{ round.providerName }}</span>
              </header>

              <div
                class="path-key-stack"
                role="list"
                :aria-label="`${round.providerName} 的 Key 调用`"
              >
                <template v-for="(attempt, keyIndex) in round.attempts" :key="attempt.id">
                  <span
                    v-if="keyIndex > 0"
                    class="path-key-hop"
                    aria-hidden="true"
                  />

                  <button
                    type="button"
                    class="waterfall-item attempt-stop key-record path-key"
                    :class="[
                      `path-key--${getTraceLineStatus(attempt) ?? 'available'}`,
                      { 'is-open': openedAttemptId === attempt.id },
                    ]"
                    :data-attempt-id="attempt.id"
                    :data-status="getTraceLineStatus(attempt)"
                    :aria-expanded="openedAttemptId === attempt.id"
                    :aria-label="getAttemptAriaLabel(attempt)"
                    @click="selectAttempt(attempt)"
                  >
                    <span class="path-key-head">
                      <span class="path-key-http">
                        {{ getAttemptStateSymbol(attempt) }} {{ getAttemptHttpLabel(attempt) }}
                      </span>
                      <span class="path-key-order">{{ getAttemptOrder(attempt) }}</span>
                    </span>
                    <span class="path-key-body">
                      <span class="path-key-identity">
                        <span class="path-key-name">
                          <span>Key</span>
                          <strong
                            class="waterfall-key"
                            :title="getAttemptKeyDisplay(attempt)"
                          >{{ getAttemptKeyDisplay(attempt) }}</strong>
                        </span>
                        <span
                          v-if="getAttemptEndpointLabel(attempt)"
                          class="path-key-endpoint"
                          :title="getAttemptEndpointLabel(attempt)"
                        >{{ getAttemptEndpointLabel(attempt) }}</span>
                      </span>
                      <span class="path-key-meta">
                        <span class="path-key-metric">
                          <small>耗时</small>
                          <b>{{ formatLatency(attempt.latency_ms) }}</b>
                        </span>
                        <span
                          v-if="getAttemptFirstByteMs(attempt) !== null"
                          class="path-key-metric"
                        >
                          <small>首字</small>
                          <b>{{ formatLatency(getAttemptFirstByteMs(attempt)) }}</b>
                        </span>
                      </span>
                    </span>
                    <span
                      v-if="getAttemptCardReason(attempt)"
                      class="path-key-reason"
                      :title="getAttemptCardReason(attempt)"
                    >{{ getAttemptCardReason(attempt) }}</span>
                  </button>
                </template>
              </div>
            </section>
          </template>

          <span class="path-link path-link--out" aria-hidden="true" />

          <section
            class="path-node path-response"
            :class="`path-response--${responseState}`"
            data-request-path-response
            aria-label="请求结果"
          >
            <span class="path-response-head">
              <span class="path-node-kicker">请求结果</span>
              <span class="path-response-status">{{ responseStatusText }}</span>
            </span>
            <span class="path-response-total">
              总耗时
              <b data-trace-latency>{{ formatLatency(totalTraceLatency) }}</b>
            </span>
          </section>
        </div>
      </div>

      <span class="sr-only" data-route-summary>
        {{ attemptedProviderCount }} 家供应商，{{ attemptedCallCount }} 次上游请求
      </span>

      <section
        v-if="openedAttempt"
        class="waterfall-detail path-detail"
        :data-execution-detail="openedAttempt.id"
        :data-execution-inspector="openedAttempt.id"
        :data-attempt-id="openedAttempt.id"
        :data-status="getTraceLineStatus(openedAttempt)"
        aria-live="polite"
      >
        <dl class="waterfall-detail-facts">
          <div>
            <dt>HTTP</dt>
            <dd class="mono" data-attempt-http-status>{{ openedAttempt.status_code ?? (getDisplayStatus(openedAttempt) === 'success' ? 200 : '-') }}</dd>
          </div>
          <div>
            <dt>耗时</dt>
            <dd class="mono">{{ getAttemptTimeRange(openedAttempt)?.durationLabel || formatLatency(openedAttempt.latency_ms) }}</dd>
          </div>
          <div v-if="getAttemptTimeRange(openedAttempt)" class="waterfall-detail-time">
            <dt>时间</dt>
            <dd class="mono">
              {{ formatTime(getAttemptTimeRange(openedAttempt)!.startIso) }}
              <span class="waterfall-time-arrow">→</span>
              {{ getAttemptTimeRange(openedAttempt)?.endIso ? formatTime(getAttemptTimeRange(openedAttempt)!.endIso!) : '进行中' }}
            </dd>
          </div>
          <div v-if="openedAttempt.key_preview">
            <dt>凭据</dt>
            <dd class="mono">{{ openedAttempt.key_preview }}</dd>
          </div>
        </dl>

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

        <div v-if="openedAttempt.status === 'success' && usageData" class="usage-section">
          <span class="usage-label">输入 {{ formatNumber(usageData.tokens.input) }}</span>
          <span class="usage-label">输出 {{ formatNumber(usageData.tokens.output) }}</span>
          <span class="usage-label">缓存创建 {{ formatNumber(usageData.tokens.cache_creation || 0) }}</span>
          <span class="usage-label">缓存读取 {{ formatNumber(usageData.tokens.cache_read || 0) }}</span>
        </div>

        <RequestAttemptErrorPanel
          v-if="getAttemptError(openedAttempt)"
          :key="openedAttempt.id"
          :error="getAttemptError(openedAttempt)!"
          :is-dark="isDark"
        />
      </section>
    </section>

    <div
      v-else
      class="execution-waterfall execution-waterfall--empty"
    >
      没有已发送的上游请求
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
import { isAttemptedCandidate, TIMELINE_STATUS } from '../utils/timelineCandidates'

interface AttemptTimeRange {
  startIso: string
  endIso?: string
  durationLabel?: string
}

/** 同一供应商的 Key 调用分组及其在整体调度顺序中的位置。 */
interface ProviderRound {
  /** 供应商的稳定身份，用于分组和节点定位。 */
  providerId: string
  /** 本次调度节点的唯一身份；供应商重访时仍保持独立节点。 */
  nodeKey: string
  /** 供应商标题中展示的稳定两位序号。 */
  order: string
  providerName: string
  /** 供应商固定背景色调索引，同一供应商身份在不同刷新和重排中保持一致。 */
  colorTone: number
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

/** 真实发出的上游调用；未尝试、未使用和冷却跳过不进入请求路径。 */
const pathTimeline = computed<CandidateRecord[]>(() => (
  timeline.value.filter(isAttemptedCandidate)
))

// 只合并连续的同供应商调用；供应商再次出现时另起节点，保持真实调度顺序。
const providerRounds = computed<ProviderRound[]>(() => {
  const rounds: ProviderRound[] = []
  let activeProviderId: string | null = null
  const colorToneByProvider = new Map<string, number>()

  pathTimeline.value.forEach((attempt, attemptIndex) => {
    const providerId = String(attempt.provider_id || attempt.provider_name || '__unknown__')
    const activeRound = rounds[rounds.length - 1]
    if (activeRound && activeProviderId === providerId) {
      const round = activeRound
      round.attempts.push(attempt)
      round.attemptIndexes.push(attemptIndex)
      return
    }

    activeProviderId = providerId
    const existingTone = colorToneByProvider.get(providerId)
    const colorTone = existingTone ?? colorToneByProvider.size % 6
    if (existingTone == null) colorToneByProvider.set(providerId, colorTone)
    rounds.push({
      providerId,
      nodeKey: `${providerId}@${rounds.length}`,
      order: String(rounds.length + 1).padStart(2, '0'),
      providerName: getProviderDisplayName(attempt),
      colorTone,
      attempts: [attempt],
      attemptIndexes: [attemptIndex],
      firstAttemptIndex: attemptIndex,
    })
  })

  return rounds
})

/** 实际发出的上游调用数；本地冷却跳过项没有调用上游，不计入。 */
const attemptedCallCount = computed(() => (
  pathTimeline.value.length
))

/** 路径可能因 A → B → A 产生多个节点，但摘要里的供应商数仍按真实身份去重。 */
const attemptedProviderCount = computed(() => (
  new Set(pathTimeline.value.map(attempt => String(
    attempt.provider_id || attempt.provider_name || '__unknown__',
  ))).size
))

/** Key 节点展示统一结果措辞，供无障碍标签说明真实的单次调用结果。 */
function formatAttemptOutcome(attempt: CandidateRecord): string {
  return getStatusLabel(getDisplayStatus(attempt))
}

/** 将候选状态归一为路径节点的成功、失败、跳过或进行中语义。 */
function getTraceLineStatus(attempt: CandidateRecord): 'failed' | 'success' | 'skipped' | 'pending' | undefined {
  const status = getDisplayStatus(attempt)
  if (status === 'failed' || status === 'cancelled' || status === 'stream_interrupted') return 'failed'
  if (status === 'success') return 'success'
  if (status === 'skipped') return 'skipped'
  if (isLiveCandidate(attempt)) return 'pending'
  return undefined
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

/** 当前展开详情始终读取对应记录，刷新重排时由记录 ID 保持身份。 */
const openedAttempt = computed(() => (
  openedAttemptId.value
    ? pathTimeline.value.find(attempt => attempt.id === openedAttemptId.value) ?? null
    : null
))

/** 请求入口优先使用后端记录的真实路径，缺失时才展示通用 Responses 路由。 */
const requestPath = computed(() => (
  trace.value?.request_path?.trim() || '/v1/responses'
))

/** 请求结果优先使用请求级终态，避免用最后一个 Key 的 HTTP 替代整体结论。 */
const responseState = computed<'success' | 'failed' | 'pending'>(() => {
  const status = computedFinalStatus.value
  if (status === 'success') return 'success'
  if (status === 'pending' || status === 'streaming') return 'pending'
  return 'failed'
})

/** 请求结果状态码来自请求终态或最后一个真实 Key，未知时保留横杠。 */
const responseStatusText = computed(() => {
  const status = computedFinalStatus.value
  if (status === 'pending' || status === 'streaming') return '进行中'
  if (status === 'success') {
    return String(props.overrideStatusCode ?? pathTimeline.value.find(attempt => getDisplayStatus(attempt) === 'success')?.status_code ?? '-')
  }
  const lastAttempt = pathTimeline.value[pathTimeline.value.length - 1]
  return String(props.overrideStatusCode ?? lastAttempt?.status_code ?? '-')
})

/** 方框与连线使用不同比例列，单个/两个供应商优先铺满，更多供应商才内部滚动。 */
const pathGridTemplate = computed(() => {
  const providerColumns = providerRounds.value.flatMap(() => (
    'minmax(32px, 0.45fr) minmax(220px, 1.55fr)'
  ))
  return [
    'minmax(140px, 0.9fr)',
    ...providerColumns,
    'minmax(32px, 0.45fr)',
    'minmax(140px, 0.9fr)',
  ].join(' ')
})

/** Key 卡片只显示真实 HTTP 状态；未知状态使用横杠而非伪造成功码。 */
function getAttemptHttpLabel(attempt: CandidateRecord): string {
  if (typeof attempt.status_code === 'number') return String(attempt.status_code)
  return '-'
}

/** Key 卡片优先展示实际端点名称，缺失时回退到该次调用的上游 API 格式。 */
function getAttemptEndpointLabel(attempt: CandidateRecord): string {
  const endpoint = attempt.endpoint_name?.trim()
    || String(attempt.extra_data?.provider_api_format ?? '').trim()
  return endpoint ? formatApiFormat(endpoint) : ''
}

/** 首字耗时只读取候选记录中的真实值；缺失或非正数时不补造展示数据。 */
function getAttemptFirstByteMs(attempt: CandidateRecord): number | null {
  const value = attempt.extra_data?.first_byte_time_ms
  if (typeof value !== 'number' || !Number.isFinite(value) || value <= 0) return null
  return Math.round(value)
}

/** Key 卡片使用极简结果符号，与成功/失败语义色共同区分状态。 */
function getAttemptStateSymbol(attempt: CandidateRecord): string {
  const status = getTraceLineStatus(attempt)
  if (status === 'success') return '✓'
  if (status === 'failed') return '✕'
  return '…'
}

/** 同一供应商内按真实调用次序编号，避免与供应商序号混淆。 */
function getAttemptOrder(attempt: CandidateRecord): string {
  const round = providerRounds.value.find(candidate => candidate.attempts.some(item => item.id === attempt.id))
  const index = round?.attempts.findIndex(item => item.id === attempt.id) ?? 0
  return String(index + 1).padStart(2, '0')
}

/** Key 卡片只展示一行真实原因；完整文本保留在 title 中，不撑高固定卡片。 */
function getAttemptCardReason(attempt: CandidateRecord): string {
  const error = getAttemptError(attempt)
  if (error?.message) return error.message
  if (getTraceLineStatus(attempt) === 'success') return ''
  if (!isLiveCandidate(attempt)) return getStatusLabel(getDisplayStatus(attempt))
  return attempt.status === 'streaming' ? '传输中' : '进行中'
}

/** 读屏标签补充卡片上可见的耗时、首字和端点；成功状态与失败原因仍不重复。 */
function getAttemptAriaLabel(attempt: CandidateRecord): string {
  const parts = [getAttemptKeyDisplay(attempt), formatAttemptOutcome(attempt)]
  parts.push(`耗时 ${formatLatency(attempt.latency_ms)}`)
  const firstByteMs = getAttemptFirstByteMs(attempt)
  if (firstByteMs !== null) parts.push(`首字 ${formatLatency(firstByteMs)}`)
  const endpoint = getAttemptEndpointLabel(attempt)
  if (endpoint) parts.push(endpoint)
  const reason = getAttemptCardReason(attempt)
  if (reason) parts.push(reason)
  parts.push('查看详情')
  return parts.join('，')
}

/** 展开行读取自身记录的时间范围，不受导航选择下标变化影响。 */
function getAttemptTimeRange(attempt: CandidateRecord): AttemptTimeRange | null {
  return resolveAttemptTimeRange(attempt)
}

// 只有展开的 Key 才对外成为当前尝试；路径加载不会隐式选中未展开记录。
watch(openedAttempt, (attempt) => {
  emit('selectAttempt', attempt)
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

/** 展开行使用各自的图片进度，不读取另一个 Key 的活动状态。 */
function getAttemptImageProgress(attempt: CandidateRecord): ImageProgress | null {
  return normalizeImageProgress(attempt.image_progress)
    ?? normalizeImageProgress(extractObject(attempt.extra_data)?.image_progress)
}

const currentDetailImageProgress = computed<ImageProgress | null>(() => (
  openedAttempt.value ? getAttemptImageProgress(openedAttempt.value) : null
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

const SKIP_REASON_LABELS: Record<string, string> = {
  auth_api_key_concurrency_limit_reached: '调用方 API Key 并发已达上限',
  api_key_concurrency_limit_reached: '调用方 API Key 并发已达上限',
  provider_concurrency_limit_reached: '上游提供商并发已达上限',
  provider_key_concurrency_limit_reached: '上游账号并发已达上限',
  provider_request_body_build_failed: '上游请求体转换失败',
  provider_request_body_missing: '无法构建上游请求体',
}
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

const hasActiveImageProgress = computed(() => {
  return rawTimeline.value.some((candidate) => {
    const progress = normalizeImageProgress(candidate.image_progress)
      ?? normalizeImageProgress(extractObject(candidate.extra_data)?.image_progress)
    if (!progress?.phase) return false
    return progress.phase !== 'upstream_completed' && progress.phase !== 'failed'
  })
})

/** 点击 Key 时切换选中记录；再次点击同一 Key 收起其真实详情。 */
function selectAttempt(attempt: CandidateRecord): void {
  const index = timeline.value.findIndex(candidate => candidate.id === attempt.id)
  if (index < 0) return
  if (openedAttemptId.value === attempt.id) {
    openedAttemptId.value = null
    return
  }
  selectedAttemptIndex.value = index
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

// 刷新和重排时按记录身份保持已展开 Key；记录消失则收起旧详情。
watch(timeline, (attempts, previous) => {
  const sameRequest = previous?.[0]?.request_id === attempts[0]?.request_id
  if (openedAttemptId.value && sameRequest) {
    const selectedId = openedAttemptId.value
    const index = attempts.findIndex(attempt => attempt.id === selectedId)
    if (index >= 0) {
      selectedAttemptIndex.value = index
      return
    }
  }
  if (openedAttemptId.value) openedAttemptId.value = null
  selectedAttemptIndex.value = 0
}, { immediate: true })

watch(
  [() => props.requestId, () => props.traceData],
  ([requestId], previous) => {
    if (requestId !== previous?.[0] || props.traceData) {
      traceLoadVersion += 1
      traceLoadInFlight = null
      if (requestId !== previous?.[0]) {
        openedAttemptId.value = null
        selectedAttemptIndex.value = 0
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
  --path-success: #15803d;
  --path-success-soft: #f0fdf4;
  --path-success-border: #86efac;
  --path-failed: #dc2626;
  --path-failed-soft: #fff1f1;
  --path-failed-border: #f87171;
  --path-pending: #2563eb;
  --path-pending-soft: #eff6ff;
  --path-pending-border: #93c5fd;
  width: 100%;
  min-width: 0;
  container-type: inline-size;
  font-family: var(--sans-serif);
}

.is-dark .minimal-request-timeline {
  --path-success: #4ade80;
  --path-success-soft: color-mix(in srgb, #166534 42%, var(--card));
  --path-success-border: #22c55e;
  --path-failed: #f87171;
  --path-failed-soft: color-mix(in srgb, #991b1b 35%, var(--card));
  --path-failed-border: #ef4444;
  --path-pending: #60a5fa;
  --path-pending-soft: color-mix(in srgb, #1e40af 32%, var(--card));
  --path-pending-border: #3b82f6;
}

.execution-waterfall {
  min-width: 0;
  overflow: hidden;
  border: 0;
  border-radius: 0;
  background: transparent;
}
.execution-waterfall--loading { padding: 0.7rem; }
.execution-waterfall--error { padding: 0.8rem 1rem; color: var(--destructive); font-size: 0.75rem; }
.execution-waterfall--empty { padding: 1rem; color: var(--muted-foreground); font-size: 0.75rem; text-align: center; }

.path-scroll {
  width: 100%;
  max-width: 100%;
  overflow-x: auto;
  overflow-y: hidden;
  overscroll-behavior-inline: contain;
  scrollbar-width: thin;
}
.path-scroll::-webkit-scrollbar { height: 6px; }
.path-scroll::-webkit-scrollbar-thumb { border-radius: 999px; background: color-mix(in srgb, var(--muted-foreground) 28%, transparent); }
.path-stage {
  display: grid;
  align-items: center;
  width: 100%;
  min-width: min-content;
  max-width: 100%;
  padding: 13px 10px 12px;
  background-color: color-mix(in srgb, var(--card) 94%, var(--muted));
  background-image:
    radial-gradient(
      circle at 1px 1px,
      color-mix(in srgb, var(--muted-foreground) 14%, transparent) 1px,
      transparent 0
    ),
    linear-gradient(135deg, color-mix(in srgb, var(--primary) 3%, transparent), transparent 42%);
  background-size: 18px 18px, 100% 100%;
}
.path-node,
.path-provider { min-width: 0; }
.path-node {
  position: relative;
  display: flex;
  min-height: 72px;
  flex-direction: column;
  justify-content: center;
  gap: 5px;
  padding: 12px 14px;
  border: 1px dashed var(--border);
  border-radius: 13px;
  background: color-mix(in srgb, var(--card) 88%, var(--muted));
  box-shadow: 0 7px 22px -20px color-mix(in srgb, var(--foreground) 55%, transparent);
}
.path-entry { border-color: color-mix(in srgb, var(--primary) 42%, var(--border)); }
.path-stage--success .path-entry { border-color: color-mix(in srgb, var(--path-success) 48%, var(--border)); }
.path-stage--failed .path-entry { border-color: color-mix(in srgb, var(--path-failed) 48%, var(--border)); }
.path-stage--pending .path-entry { border-color: color-mix(in srgb, var(--path-pending) 48%, var(--border)); }
.path-node-kicker { color: var(--muted-foreground); font-size: 11px; font-weight: 600; letter-spacing: 0.04em; }
.path-entry-line { display: flex; min-width: 0; align-items: baseline; gap: 6px; }
.path-method { flex: none; color: var(--primary); font-family: var(--monospace); font-size: 11px; font-weight: 700; }
.path-stage--success .path-method { color: var(--path-success); }
.path-stage--failed .path-method { color: var(--path-failed); }
.path-stage--pending .path-method { color: var(--path-pending); }
.path-endpoint { min-width: 0; overflow: hidden; color: var(--foreground); font-family: var(--monospace); font-size: 11px; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.path-link {
  --path-link-color: color-mix(in srgb, var(--primary) 78%, var(--foreground));
  position: relative;
  display: flex;
  min-width: 28px;
  align-items: center;
  align-self: center;
  justify-content: center;
  height: 32px;
}
.path-link::before {
  flex: 0 0 100%;
  width: 100%;
  height: 2px;
  border-radius: 999px;
  background:
    linear-gradient(90deg, transparent 48%, color-mix(in srgb, #fff 90%, transparent) 50%, transparent 52%) 0 0 / 44px 100%,
    linear-gradient(90deg, color-mix(in srgb, var(--path-link-color) 22%, transparent), var(--path-link-color));
  box-shadow: 0 0 10px color-mix(in srgb, var(--path-link-color) 26%, transparent);
  content: '';
  animation: path-link-flow 2.4s linear infinite;
}
.path-link::after {
  position: absolute;
  right: -1px;
  width: 7px;
  height: 7px;
  border-top: 2px solid var(--path-link-color);
  border-right: 2px solid var(--path-link-color);
  filter: drop-shadow(0 0 3px color-mix(in srgb, var(--path-link-color) 42%, transparent));
  content: '';
  transform: rotate(45deg);
}
.path-link--out { --path-link-color: var(--path-success); }
.path-link--out::after { border-color: var(--path-success); }
.path-stage--failed .path-link--out { --path-link-color: var(--path-failed); }
.path-stage--failed .path-link--out::after { border-color: var(--path-failed); }
.path-stage--pending .path-link--out { --path-link-color: var(--path-pending); }
.path-stage--pending .path-link--out::after { border-color: var(--path-pending); }
.path-provider {
  --provider-tone-border: color-mix(in srgb, var(--foreground) 68%, transparent);
  --provider-tone-fill: color-mix(in srgb, var(--card) 82%, #f5f3ff);
  --provider-tone-accent: #8b5cf6;
  position: relative;
  padding: 27px 11px 11px;
  border: 1px solid color-mix(in srgb, var(--provider-tone-accent) 38%, var(--border));
  border-radius: 17px;
  background:
    linear-gradient(180deg, color-mix(in srgb, #fff 20%, transparent), transparent 44%),
    var(--provider-tone-fill);
  box-shadow:
    inset 0 0 0 1px color-mix(in srgb, #fff 24%, transparent),
    0 12px 28px -26px color-mix(in srgb, var(--provider-tone-accent) 55%, transparent);
}
.path-provider::after {
  position: absolute;
  inset: 4px;
  z-index: 0;
  border: 1px dashed color-mix(in srgb, var(--provider-tone-accent) 24%, transparent);
  border-radius: 12px;
  content: '';
  pointer-events: none;
}
.path-provider > * { position: relative; z-index: 1; }
.path-provider--tone-0 {
  --provider-tone-fill: #f6f2ff;
  --provider-tone-accent: #8b5cf6;
}
.path-provider--tone-1 {
  --provider-tone-fill: #eefaf7;
  --provider-tone-accent: #0d9488;
}
.path-provider--tone-2 {
  --provider-tone-fill: #eef5ff;
  --provider-tone-accent: #2563eb;
}
.path-provider--tone-3 {
  --provider-tone-fill: #fff8e9;
  --provider-tone-accent: #d97706;
}
.path-provider--tone-4 {
  --provider-tone-fill: #fff2f8;
  --provider-tone-accent: #db2777;
}
.path-provider--tone-5 {
  --provider-tone-fill: #eefaf4;
  --provider-tone-accent: #059669;
}
.path-provider-label {
  position: absolute;
  top: -12px;
  left: 18px;
  display: inline-flex;
  max-width: calc(100% - 36px);
  align-items: baseline;
  gap: 6px;
  padding: 1px 8px;
  border: 1px solid color-mix(in srgb, var(--provider-tone-accent) 18%, transparent);
  border-radius: 999px;
  background: color-mix(in srgb, var(--provider-tone-fill) 86%, var(--card));
  box-shadow: 0 2px 8px -7px color-mix(in srgb, var(--foreground) 50%, transparent);
  font-size: 11px;
  line-height: 1.7;
  white-space: nowrap;
}
.waterfall-provider-order { color: var(--foreground); font-family: var(--monospace); font-size: 9px; font-weight: 700; }
.waterfall-provider-name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; font-weight: 700; }
.path-key-stack { display: grid; min-width: 0; }
.path-key {
  display: flex;
  width: 100%;
  height: 88px;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: 13px;
  background: var(--card);
  box-shadow: 0 3px 10px -8px color-mix(in srgb, var(--foreground) 35%, transparent);
  contain: inline-size;
  color: var(--foreground);
  text-align: left;
  transition: border-color 150ms ease, box-shadow 150ms ease;
}
.path-key--failed { border-color: var(--path-failed-border); box-shadow: 0 4px 13px -11px color-mix(in srgb, var(--path-failed) 58%, transparent); }
.path-key--success { border-color: var(--path-success-border); box-shadow: 0 0 0 3px color-mix(in srgb, var(--path-success) 8%, transparent), 0 5px 14px -12px color-mix(in srgb, var(--path-success) 65%, transparent); }
.path-key--pending { border-color: var(--path-pending-border); }
.path-key:focus-visible { outline: 1.5px solid color-mix(in srgb, var(--ring) 55%, transparent); outline-offset: 2px; }
.path-key-head { display: flex; flex: none; align-items: center; justify-content: space-between; gap: 8px; padding: 4px 9px; border-bottom: 1px solid color-mix(in srgb, var(--border) 42%, transparent); background: color-mix(in srgb, var(--muted) 52%, transparent); }
.path-key--failed .path-key-head { background: var(--path-failed-soft); }
.path-key--success .path-key-head { background: var(--path-success-soft); }
.path-key--pending .path-key-head { background: var(--path-pending-soft); }
.path-key-http { font-family: var(--monospace); font-size: 10px; font-weight: 700; font-variant-numeric: tabular-nums; }
.path-key--failed .path-key-http { color: var(--path-failed); }
.path-key--success .path-key-http { color: var(--path-success); }
.path-key--pending .path-key-http { color: var(--path-pending); }
.path-key-order { color: var(--muted-foreground); font-family: var(--monospace); font-size: 9px; }
.path-key-body { display: flex; min-width: 0; flex: 1; align-items: center; justify-content: space-between; gap: 8px; padding: 5px 10px 0; }
.path-key-identity { display: grid; min-width: 0; flex: 1; align-content: center; gap: 2px; }
.path-key-name { display: flex; min-width: 0; align-items: baseline; gap: 5px; }
.path-key-name > span { flex: none; color: var(--muted-foreground); font-size: 9px; }
.waterfall-key { min-width: 0; overflow: hidden; color: var(--foreground); font-family: var(--monospace); font-size: 11px; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.path-key-endpoint { min-width: 0; overflow: hidden; color: color-mix(in srgb, var(--muted-foreground) 84%, var(--foreground)); font-family: var(--monospace); font-size: 8.5px; text-overflow: ellipsis; white-space: nowrap; }
.path-key-meta { display: flex; flex: none; align-items: center; gap: 7px; color: var(--muted-foreground); font-size: 8px; }
.path-key-metric { display: grid; min-width: 31px; justify-items: end; gap: 0; line-height: 1.2; white-space: nowrap; }
.path-key-metric small { color: var(--muted-foreground); font-size: 8px; }
.path-key-metric b { color: var(--foreground); font-family: var(--monospace); font-size: 10px; font-weight: 600; font-variant-numeric: tabular-nums; }
.path-key-reason { display: block; flex: none; min-width: 0; max-width: 100%; overflow: hidden; padding: 3px 10px 5px; color: var(--muted-foreground); font-family: var(--monospace); font-size: 8.5px; line-height: 1.3; text-overflow: ellipsis; white-space: nowrap; word-break: normal; }
.path-key--failed .path-key-reason { color: var(--path-failed); }
.path-key--success .path-key-reason { color: var(--path-success); }
.path-key-hop { position: relative; display: block; min-height: 26px; }
.path-key-hop::before { position: absolute; top: 0; bottom: 0; left: 50%; width: 1px; background: var(--path-failed); content: ''; }
.path-key-hop::after { position: absolute; bottom: 0; left: calc(50% - 3px); width: 6px; height: 6px; border-right: 1px solid var(--path-failed); border-bottom: 1px solid var(--path-failed); content: ''; transform: rotate(45deg); }
.path-response { border-color: color-mix(in srgb, var(--path-success) 48%, var(--border)); }
.path-response--failed { border-color: color-mix(in srgb, var(--path-failed) 48%, var(--border)); }
.path-response--pending { border-color: color-mix(in srgb, var(--path-pending) 48%, var(--border)); }
.path-response-head { display: flex; min-width: 0; align-items: center; justify-content: space-between; gap: 8px; }
.path-response-status { flex: none; color: var(--path-success); font-family: var(--monospace); font-size: 11px; font-weight: 700; }
.path-response--failed .path-response-status { color: var(--path-failed); }
.path-response--pending .path-response-status { color: var(--path-pending); }
.path-response-total { display: flex; min-width: 0; align-items: baseline; gap: 4px; color: var(--muted-foreground); font-size: 9px; }
.path-response-total b { min-width: 0; overflow: hidden; color: var(--foreground); font-family: var(--monospace); font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.waterfall-detail { min-width: 0; margin: 0; padding: 0.72rem 0.9rem 0.8rem; border-top: 1px solid color-mix(in srgb, var(--border) 52%, transparent); background: transparent; }
.waterfall-detail-facts { display: flex; min-width: 0; flex-wrap: wrap; align-items: baseline; gap: 0.38rem 1.15rem; margin: 0; }
.waterfall-detail-facts > div { display: inline-flex; min-width: 0; align-items: baseline; gap: 0.34rem; }
.waterfall-detail-facts dt { flex: none; color: var(--muted-foreground); font-size: 0.62rem; }
.waterfall-detail-facts dd { min-width: 0; margin: 0; overflow-wrap: anywhere; font-size: 0.72rem; font-weight: 500; }
.waterfall-time-arrow { margin: 0 0.2rem; color: var(--muted-foreground); }

.image-progress-block { margin-top: 0.7rem; padding-top: 0.65rem; border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent); }
.image-progress-header { display: flex; align-items: center; justify-content: space-between; gap: 0.7rem; margin-bottom: 0.55rem; }
.image-progress-title { font-size: 0.65rem; font-weight: 600; }
.image-progress-phase { padding: 0.1rem 0.35rem; border: 1px solid var(--border); border-radius: 99px; font-size: 0.58rem; }
.image-progress-phase.phase-connecting, .image-progress-phase.phase-streaming { color: #3978b8; }
.image-progress-phase.phase-completed { color: #278461; }
.image-progress-phase.phase-failed { color: #c14b4b; }
.image-progress-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 0.48rem 0.7rem; }
.image-progress-item { display: grid; min-width: 0; gap: 0.13rem; }
.image-progress-item.full-width { grid-column: 1 / -1; }
.image-progress-label { color: var(--muted-foreground); font-size: 0.56rem; }
.image-progress-value { font-size: 0.65rem; font-weight: 500; }
.image-progress-code { min-width: 0; max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 0.08rem 0.25rem; border-radius: 0.2rem; background: color-mix(in srgb, var(--muted) 65%, transparent); color: var(--muted-foreground); font-family: var(--monospace); font-size: 0.65rem; }

.usage-section { display: flex; flex-wrap: wrap; gap: 0.4rem 0.85rem; margin-top: 0.7rem; padding-top: 0.65rem; border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent); }
.usage-label { color: var(--muted-foreground); font-size: 0.6rem; }
.mono { font-family: var(--monospace); font-variant-numeric: tabular-nums; }

/* 深色主题通过组件主题类应用同一语义状态色，避免 scoped :global 裁掉后代选择器。 */
.is-dark .path-key--failed .path-key-head { background: color-mix(in srgb, #991b1b 35%, var(--card)); }
.is-dark .path-key--success .path-key-head { background: color-mix(in srgb, #166534 42%, var(--card)); }
.is-dark .path-key--pending .path-key-head { background: color-mix(in srgb, #1e40af 32%, var(--card)); }
.is-dark .path-stage {
  background-color: color-mix(in srgb, var(--card) 93%, var(--muted));
  background-image:
    radial-gradient(
      circle at 1px 1px,
      color-mix(in srgb, var(--foreground) 16%, transparent) 1px,
      transparent 0
    ),
    linear-gradient(135deg, color-mix(in srgb, var(--primary) 7%, transparent), transparent 42%);
  background-size: 18px 18px, 100% 100%;
}
.is-dark .path-provider { box-shadow: inset 0 0 0 1px color-mix(in srgb, #fff 3%, transparent), 0 12px 28px -26px color-mix(in srgb, #000 85%, transparent); }
.is-dark .path-provider--tone-0 { --provider-tone-fill: color-mix(in srgb, #4c1d95 34%, var(--card)); }
.is-dark .path-provider--tone-1 { --provider-tone-fill: color-mix(in srgb, #134e4a 34%, var(--card)); }
.is-dark .path-provider--tone-2 { --provider-tone-fill: color-mix(in srgb, #1e3a8a 34%, var(--card)); }
.is-dark .path-provider--tone-3 { --provider-tone-fill: color-mix(in srgb, #78350f 34%, var(--card)); }
.is-dark .path-provider--tone-4 { --provider-tone-fill: color-mix(in srgb, #831843 34%, var(--card)); }
.is-dark .path-provider--tone-5 { --provider-tone-fill: color-mix(in srgb, #064e3b 34%, var(--card)); }
@keyframes path-link-flow {
  from { background-position: -44px 0, 0 0; }
  to { background-position: calc(100% + 44px) 0, 0 0; }
}
@media (prefers-reduced-motion: reduce) {
  .path-key { transition: none; }
  .path-link::before { animation: none; }
}
</style>
