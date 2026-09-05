<template>
  <div
    class="error-block"
    :class="`is-${presentation.tone}`"
    role="alert"
  >
    <div class="error-summary">
      <div
        class="error-icon"
        aria-hidden="true"
      >
        <TriangleAlert class="h-4 w-4" />
      </div>
      <div class="error-content">
        <div class="error-heading">
          <div class="error-title-group">
            <span class="error-type">错误信息</span>
            <h5 class="error-title">
              {{ presentation.title }}
            </h5>
          </div>
          <span
            v-if="error.statusCode != null"
            class="error-status-badge"
            :class="`is-${presentation.tone}`"
          >
            HTTP {{ error.statusCode }}
          </span>
        </div>
        <p class="error-description">
          {{ presentation.description }}
        </p>
        <p class="error-guidance">
          {{ presentation.guidance }}
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
        <span class="error-details-meta">原始错误与响应数据</span>
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
import { ChevronRight, TriangleAlert } from 'lucide-vue-next'
import JsonContentPanel from './JsonContentPanel.vue'

interface AttemptRequestError {
  message: string
  technicalMessage: string
  presentationSource: string
  statusCode?: number
  upstreamResponse: Record<string, unknown> | null
  diagnostic: Record<string, unknown> | null
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
.error-block {
  --error-accent: #b45309;
  --error-icon-background: #fef3c7;
  --error-surface: color-mix(in srgb, #f59e0b 7%, var(--card));
  --error-border: color-mix(in srgb, #f59e0b 28%, var(--border));

  margin-top: 1rem;
  overflow: hidden;
  background: var(--error-surface);
  border: 1px solid var(--error-border);
  border-radius: 8px;
}

.error-block.is-danger {
  --error-accent: var(--destructive);
  --error-icon-background: color-mix(in srgb, var(--destructive) 12%, var(--card));
  --error-surface: color-mix(in srgb, var(--destructive) 5%, var(--card));
  --error-border: color-mix(in srgb, var(--destructive) 25%, var(--border));
}

.error-summary {
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 1rem;
}

.error-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 2rem;
  height: 2rem;
  flex: 0 0 2rem;
  border-radius: 6px;
  background: var(--error-icon-background);
  color: var(--error-accent);
}

.error-content {
  min-width: 0;
  flex: 1;
}

.error-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.75rem;
}

.error-title-group {
  min-width: 0;
}

.error-title {
  margin: 0.125rem 0 0;
  color: var(--foreground);
  font-size: 0.95rem;
  font-weight: 650;
  line-height: 1.35;
}

.error-type {
  display: block;
  color: var(--error-accent);
  font-size: 0.75rem;
  font-weight: 600;
  line-height: 1.25;
}

.error-status-badge {
  flex-shrink: 0;
  padding: 0.2rem 0.5rem;
  border: 1px solid transparent;
  border-radius: 999px;
  font-family: ui-monospace, monospace;
  font-size: 0.72rem;
  font-variant-numeric: tabular-nums;
  line-height: 1.35;
}

.error-status-badge.is-warning {
  border-color: color-mix(in srgb, #d97706 24%, transparent);
  background: color-mix(in srgb, #f59e0b 14%, var(--card));
  color: #92400e;
}

.error-status-badge.is-danger {
  border-color: color-mix(in srgb, var(--destructive) 22%, transparent);
  background: color-mix(in srgb, var(--destructive) 10%, var(--card));
  color: var(--destructive);
}

.error-description {
  margin: 0.5rem 0 0;
  color: var(--foreground);
  font-size: 0.85rem;
  line-height: 1.55;
  word-break: break-word;
}

.error-guidance {
  margin: 0.25rem 0 0;
  color: var(--muted-foreground);
  font-size: 0.8rem;
  line-height: 1.55;
  word-break: break-word;
}

.error-details {
  border-top: 1px solid var(--error-border);
}

.error-details-toggle {
  display: flex;
  min-height: 44px;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.65rem 1rem;
  color: var(--muted-foreground);
  cursor: pointer;
  user-select: none;
  list-style: none;
}

.error-details-toggle::-webkit-details-marker {
  display: none;
}

.error-details-toggle:hover {
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
  color: var(--foreground);
}

.error-details-toggle:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: -2px;
}

.error-details-label {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  color: var(--foreground);
  font-size: 0.8rem;
  font-weight: 600;
}

.error-details-icon {
  flex-shrink: 0;
  transition: transform 180ms ease;
}

.error-details[open] .error-details-icon {
  transform: rotate(90deg);
}

.error-details-meta {
  font-size: 0.75rem;
}

.error-details-content {
  padding: 0 1rem 1rem;
}

.error-technical-message {
  display: grid;
  gap: 0.35rem;
  padding: 0.75rem;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--card);
}

.error-technical-label {
  color: var(--muted-foreground);
  font-size: 0.72rem;
  font-weight: 600;
}

.error-technical-message code {
  overflow-wrap: anywhere;
  color: var(--foreground);
  font-size: 0.8rem;
  line-height: 1.5;
  white-space: pre-wrap;
}

.error-json {
  margin-top: 0.75rem;
}

.dark .error-block.is-warning {
  --error-accent: #fbbf24;
  --error-icon-background: color-mix(in srgb, #f59e0b 16%, var(--card));
}

.dark .error-status-badge.is-warning {
  color: #fde68a;
}

@media (max-width: 640px) {
  .error-summary {
    gap: 0.625rem;
    padding: 0.875rem;
  }

  .error-heading {
    flex-wrap: wrap;
  }

  .error-details-toggle {
    padding-inline: 0.875rem;
  }

  .error-details-meta {
    display: none;
  }

  .error-details-content {
    padding: 0 0.875rem 0.875rem;
  }
}

@media (prefers-reduced-motion: reduce) {
  .error-details-icon {
    transition: none;
  }
}
</style>
