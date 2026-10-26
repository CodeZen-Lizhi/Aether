<template>
  <div
    class="error-block"
    :class="`is-${presentation.tone}`"
    role="alert"
  >
    <div class="error-summary">
      <div class="error-content">
        <div class="error-heading">
          <div class="error-title-group">
            <span class="error-kicker">{{ isLocalSkip ? '本地调度' : '上游响应' }}</span>
            <p class="error-title">
              {{ presentation.title }}
            </p>
          </div>
        </div>
        <p
          v-if="error.technicalMessage && !hasUpstreamResponse && error.technicalMessage !== presentation.title && error.message !== presentation.title"
          class="error-message-preview"
          :title="error.technicalMessage"
        >
          {{ error.technicalMessage }}
        </p>
      </div>
    </div>

    <details
      v-if="hasUpstreamResponse"
      class="error-details"
      data-upstream-response
    >
      <summary class="error-details-toggle">
        <span class="error-details-label">
          <ChevronRight
            class="error-details-icon h-4 w-4"
            aria-hidden="true"
          />
          上游响应
        </span>
      </summary>
      <div class="error-details-content">
        <div
          v-if="error.upstreamResponse"
          class="error-json error-upstream-response-json"
        >
          <JsonContentPanel
            :data="error.upstreamResponse"
            :is-dark="isDark"
            title="上游响应"
            empty-message="无上游响应"
          />
        </div>
      </div>
    </details>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { ChevronRight } from 'lucide-vue-next'
import JsonContentPanel from './JsonContentPanel.vue'

interface AttemptRequestError {
  message: string
  /** 不随状态分支变化的完整错误原文，供标题和三态原因直接使用。 */
  technicalMessage: string
  presentationSource: string
  statusCode?: number
  upstreamResponse: Record<string, unknown> | null
  skipReason?: string
  skipReasonLabel?: string
  skipped?: boolean
}

type AttemptErrorTone = 'warning' | 'danger'

interface AttemptErrorPresentation {
  title: string
  tone: AttemptErrorTone
}

const props = defineProps<{
  error: AttemptRequestError
  isDark: boolean
}>()

const hasUpstreamResponse = computed(() => Boolean(props.error.upstreamResponse))
const isLocalSkip = computed(() => Boolean(props.error.skipped))

const presentation = computed<AttemptErrorPresentation>(() => {
  const { error } = props
  const source = `${error.presentationSource}\n${error.message}`.toLowerCase()
  const hasUpstreamMessage = /^上游(?:返回 HTTP \\d+|限流|鉴权失败|执行失败)：.+/.test(error.message)
  const isFirstByteTimeout = /stream first byte timeout|first[-_ ]byte[^\n]*timeout/.test(source)
  const isTimeout = isFirstByteTimeout
    || error.statusCode === 408
    || error.statusCode === 504
    || /timed?\s*out|timeout|超时/.test(source)
  const isConversionFailure = /格式转换失败|conversion|cannot be converted|unsupported provider stream/.test(source)

  const primaryMessage = error.technicalMessage.trim()
  const readableMessage = error.message.trim()
  if (error.skipReason === 'key_rate_limit_cooldown') {
    return {
      title: error.skipReasonLabel || '本地限流冷却中',
      tone: 'warning',
    }
  }
  if (primaryMessage && !isTimeout && !isConversionFailure && primaryMessage !== '未知错误') {
    return {
      title: readableMessage || primaryMessage,
      tone: error.statusCode === 429 || error.statusCode === 503 || error.statusCode === 504 ? 'warning' : 'danger',
    }
  }

  if (error.skipReason === 'key_circuit_open') {
    return {
      title: error.skipped ? 'Key 处于本地熔断冷却，请求未发送' : 'Key 的本地冷却保护已生效',
      tone: 'warning',
    }
  }
  if (error.skipped) {
    return {
      title: readableMessage || primaryMessage || '当前候选未发送请求',
      tone: 'warning',
    }
  }
  if (hasUpstreamMessage) {
    return {
      title: error.message,
      tone: error.statusCode === 429 ? 'warning' : 'danger',
    }
  }
  if (isFirstByteTimeout) {
    return {
      title: '上游服务响应超时',
      tone: 'warning',
    }
  }
  if (isConversionFailure) {
    return {
      title: readableMessage || primaryMessage || '请求格式转换失败',
      tone: 'danger',
    }
  }
  if (error.statusCode === 429) {
    return {
      title: '上游请求过于频繁',
      tone: 'warning',
    }
  }
  if (error.statusCode === 401 || error.statusCode === 403) {
    return {
      title: '上游鉴权失败',
      tone: 'danger',
    }
  }
  if (isTimeout) {
    return {
      title: '上游服务响应超时',
      tone: 'warning',
    }
  }
  if (error.statusCode != null && error.statusCode >= 500) {
    return {
      title: '上游服务暂时不可用',
      tone: 'warning',
    }
  }
  if (error.statusCode != null && error.statusCode >= 400) {
    return {
      title: '上游请求未被接受',
      tone: 'danger',
    }
  }

  return {
    title: '请求处理失败',
    tone: 'danger',
  }
})
</script>

<style scoped>
.error-block { min-width: 0; margin-top: 0.95rem; padding-top: 0.85rem; border-top: 1px solid hsl(var(--border) / 0.8); }
.error-content, .error-title-group { min-width: 0; }
.error-title-group { display: grid; gap: 0.28rem; }
.error-kicker { color: var(--muted-foreground); font-size: 0.61rem; font-weight: 500; }
.error-title { margin: 0; font-size: 0.78rem; font-weight: 500; line-height: 1.65; overflow-wrap: anywhere; }
.error-message-preview { margin: 0.4rem 0 0; font-family: ui-monospace, monospace; color: var(--muted-foreground); font-size: 0.7rem; line-height: 1.6; overflow-wrap: anywhere; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; overflow: hidden; }
.error-details { margin-top: 0.4rem; }
.error-details-toggle { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 0.5rem; min-height: 36px; cursor: pointer; list-style: none; color: var(--muted-foreground); }
.error-details-toggle::-webkit-details-marker { display: none; }
.error-details-toggle:hover { color: var(--foreground); }
.error-details-toggle:focus-visible { outline: 2px solid var(--ring); outline-offset: 2px; border-radius: 4px; }
.error-details-label { display: inline-flex; align-items: center; gap: 0.35rem; font-size: 0.75rem; }
.error-details-icon { flex: none; transition: transform 150ms; }
.error-details[open] .error-details-icon { transform: rotate(90deg); }
.error-details-content { min-width: 0; padding: 0.5rem 0 0; }
.error-json { margin-top: 0.75rem; }
@media (prefers-reduced-motion: reduce) { .error-details-icon { transition: none; } }
</style>
