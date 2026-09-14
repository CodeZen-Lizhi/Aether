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
            <span class="error-type">原因说明</span>
            <h5 class="error-title">
              {{ presentation.title }}
            </h5>
          </div>
        </div>
        <p
          v-if="error.skipped && error.skipReasonLabel && error.skipReason !== 'key_circuit_open'"
          class="error-description"
        >
          {{ error.skipReasonLabel }}
        </p>
        <p class="error-description">
          <span>{{ presentation.description }}</span>
          {{ ' ' }}
          <span>{{ presentation.guidance }}</span>
        </p>
        <p
          v-if="error.technicalMessage && error.technicalMessage !== presentation.description"
          class="error-message-preview"
          :title="error.technicalMessage"
        >
          {{ error.technicalMessage }}
        </p>
      </div>
    </div>

    <details
      v-if="hasTechnicalDetails"
      class="error-details"
    >
      <summary class="error-details-toggle">
        <span class="error-details-label">
          <ChevronRight
            class="error-details-icon h-4 w-4"
            aria-hidden="true"
          />
          技术详情
        </span>
        <span class="error-details-meta">
          <span v-if="error.statusCode != null">HTTP {{ error.statusCode }} · </span>原始错误与响应数据
        </span>
      </summary>
      <div class="error-details-content">
        <div
          v-if="error.technicalMessage"
          class="error-technical-message"
        >
          <span class="error-technical-label">原始错误</span>
          <code>{{ error.technicalMessage }}</code>
        </div>
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
        <div
          v-if="error.diagnostic"
          class="error-json error-diagnostic-json"
        >
          <JsonContentPanel
            :data="error.diagnostic"
            :is-dark="isDark"
            title="失败诊断"
            empty-message="无失败诊断信息"
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
  technicalMessage: string
  presentationSource: string
  statusCode?: number
  upstreamResponse: Record<string, unknown> | null
  diagnostic: Record<string, unknown> | null
  skipReason?: string
  skipReasonLabel?: string
  skipped?: boolean
}

type AttemptErrorTone = 'warning' | 'danger'

interface AttemptErrorPresentation {
  title: string
  description: string
  guidance: string
  tone: AttemptErrorTone
}

const props = defineProps<{
  error: AttemptRequestError
  isDark: boolean
}>()

const hasTechnicalDetails = computed(() => Boolean(
  props.error.technicalMessage
    || props.error.upstreamResponse
    || props.error.diagnostic,
))

const presentation = computed<AttemptErrorPresentation>(() => {
  const { error } = props
  const source = `${error.presentationSource}\n${error.message}`.toLowerCase()
  const isFirstByteTimeout = /stream first byte timeout|first[-_ ]byte[^\n]*timeout/.test(source)
  const isTimeout = isFirstByteTimeout
    || error.statusCode === 408
    || error.statusCode === 504
    || /timed?\s*out|timeout|超时/.test(source)
  const isConversionFailure = /格式转换失败|conversion|cannot be converted|unsupported provider stream/.test(source)

  if (error.skipReason === 'key_circuit_open') {
    return {
      title: error.skipped ? '密钥熔断，未发送请求' : '密钥熔断保护生效',
      description: '当前 Key 暂停接收请求。',
      guidance: '可查看密钥健康度及熔断恢复状态。',
      tone: 'warning',
    }
  }
  if (error.skipped) {
    return {
      title: '当前候选未发送请求',
      description: error.message || error.skipReason || '此候选被调度跳过。',
      guidance: '请查看跳过原因与诊断信息。',
      tone: 'warning',
    }
  }
  if (isFirstByteTimeout) {
    return {
      title: '上游服务响应超时',
      description: '请求已发送，但上游服务在规定时间内没有返回首个响应。',
      guidance: '这通常是临时性的服务或网络问题，建议稍后重试；如果持续发生，请检查 Endpoint、代理和上游服务状态。',
      tone: 'warning',
    }
  }
  if (isConversionFailure) {
    return {
      title: '请求格式转换失败',
      description: error.message || '请求无法无损转换为上游服务所需的格式。',
      guidance: '请根据技术详情中的字段路径检查格式映射。',
      tone: 'danger',
    }
  }
  if (error.statusCode === 429) {
    return {
      title: '上游请求过于频繁',
      description: '上游服务触发了频率或额度限制。',
      guidance: '建议稍后重试，并检查当前 Key 的配额与限流配置。',
      tone: 'warning',
    }
  }
  if (error.statusCode === 401 || error.statusCode === 403) {
    return {
      title: '上游鉴权失败',
      description: '上游服务拒绝了当前凭据或权限。',
      guidance: '请检查 API Key、授权范围和 Endpoint 配置。',
      tone: 'danger',
    }
  }
  if (isTimeout) {
    return {
      title: '上游服务响应超时',
      description: '请求在等待上游服务响应时超时。',
      guidance: '建议稍后重试；如果持续发生，请检查 Endpoint、代理和上游服务状态。',
      tone: 'warning',
    }
  }
  if (error.statusCode != null && error.statusCode >= 500) {
    return {
      title: '上游服务暂时不可用',
      description: '上游服务未能正常完成请求。',
      guidance: '建议稍后重试；如果持续发生，请检查 Endpoint、代理和上游服务状态。',
      tone: 'warning',
    }
  }
  if (error.statusCode != null && error.statusCode >= 400) {
    return {
      title: '上游请求未被接受',
      description: '上游服务未能处理当前请求。',
      guidance: '请检查请求参数、模型名称和 Endpoint 配置。',
      tone: 'danger',
    }
  }

  return {
    title: '请求处理失败',
    description: error.message || '当前请求未能完成。',
    guidance: '请查看技术详情定位原因，并根据原始错误调整请求或服务配置。',
    tone: 'danger',
  }
})
</script>

<style scoped>
.error-block { min-width: 0; margin-top: 0.75rem; padding-top: 0.875rem; border-top: 1px solid var(--border); }
.error-content, .error-title-group { min-width: 0; }
.error-type { display: block; margin-bottom: 0.35rem; color: var(--muted-foreground); font-size: 0.7rem; }
.error-title { margin: 0; font-size: 0.875rem; font-weight: 600; line-height: 1.5; overflow-wrap: anywhere; }
.error-description { margin: 0.25rem 0 0; font-size: 0.75rem; color: var(--muted-foreground); line-height: 1.65; overflow-wrap: anywhere; }
.error-message-preview { margin: 0.4rem 0 0; font-family: ui-monospace, monospace; color: var(--muted-foreground); font-size: 0.7rem; line-height: 1.6; overflow-wrap: anywhere; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; overflow: hidden; }
.error-details { margin-top: 0.25rem; }
.error-details-toggle { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 0.5rem; min-height: 36px; cursor: pointer; list-style: none; color: var(--muted-foreground); }
.error-details-toggle::-webkit-details-marker { display: none; }
.error-details-toggle:hover { color: var(--foreground); }
.error-details-toggle:focus-visible { outline: 2px solid var(--ring); outline-offset: 2px; border-radius: 4px; }
.error-details-label { display: inline-flex; align-items: center; gap: 0.35rem; font-size: 0.75rem; }
.error-details-meta { font-size: 0.7rem; }
.error-details-icon { flex: none; transition: transform 150ms; }
.error-details[open] .error-details-icon { transform: rotate(90deg); }
.error-details-content { min-width: 0; padding: 0.5rem 0 0; }
.error-technical-message { display: grid; gap: 0.35rem; padding: 0.75rem; border: 1px solid var(--border); border-radius: 8px; background: var(--muted); }
.error-technical-label { color: var(--muted-foreground); font-size: 0.7rem; }
.error-technical-message code { min-width: 0; overflow-wrap: anywhere; font-size: 0.75rem; line-height: 1.6; white-space: pre-wrap !important; }
.error-json { margin-top: 0.75rem; }
@media (prefers-reduced-motion: reduce) { .error-details-icon { transition: none; } }
</style>
