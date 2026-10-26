import type { RequestDetail, RequestErrorDomain, RequestSchedulingFailure } from '@/api/dashboard'

export interface RequestFailureNotice {
  title: string
  message: string
  meta: string[]
  isSchedulingFailure: boolean
}

function nonEmptyString(value: string | null | undefined): string | null {
  const trimmed = value?.trim()
  return trimmed ? trimmed : null
}

function normalizeErrorDomain(domain: RequestErrorDomain | null | undefined): RequestErrorDomain | null {
  if (!nonEmptyString(domain?.message)) return null
  return domain ?? null
}

function formatHttpStatus(statusCode: number | null | undefined): string | null {
  return typeof statusCode === 'number' && (statusCode < 200 || statusCode >= 300)
    ? `HTTP ${statusCode}`
    : null
}

/** 将本地限流冷却截止时刻转成本地时分秒，缺失或非法时不生成推测文案。 */
function formatCooldownDeadline(timestampSecs: number | null | undefined): string | null {
  if (typeof timestampSecs !== 'number' || !Number.isFinite(timestampSecs) || timestampSecs <= 0) {
    return null
  }
  return new Date(timestampSecs * 1000).toLocaleTimeString('zh-CN', {
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

function uniqueMeta(values: Array<string | null | undefined>): string[] {
  return Array.from(new Set(values.map(value => value?.trim()).filter((value): value is string => Boolean(value))))
}

function schedulingFailureMessage(
  failure: RequestSchedulingFailure,
  fallbackDomain: RequestErrorDomain | null,
  fallbackErrorMessage: string | null,
): string | null {
  return nonEmptyString(failure.message)
    ?? nonEmptyString(fallbackDomain?.message)
    ?? fallbackErrorMessage
    ?? nonEmptyString(failure.reason_label)
    ?? nonEmptyString(failure.reason)
}

export function resolveRequestFailureNotice(detail: RequestDetail | null | undefined): RequestFailureNotice | null {
  if (!detail) return null

  const fallbackDomain = normalizeErrorDomain(detail.failure_summary)
    ?? normalizeErrorDomain(detail.client_error)
    ?? normalizeErrorDomain(detail.upstream_error)
    ?? normalizeErrorDomain(detail.request_error)
  const fallbackErrorMessage = nonEmptyString(detail.error_message ?? null)
  const schedulingFailure = detail.scheduling_failure ?? null

  if (schedulingFailure) {
    const message = schedulingFailureMessage(schedulingFailure, fallbackDomain, fallbackErrorMessage)
    if (message) {
      const isRateLimitCooldown = schedulingFailure.reason === 'key_rate_limit_cooldown'
      const cooldownLabel = formatCooldownDeadline(
        schedulingFailure.rate_limit_cooldown_until_unix_secs,
      )
      return {
        title: isRateLimitCooldown
          ? '本地限流冷却中，请求未发送'
          : nonEmptyString(schedulingFailure.title) ?? '本地调度失败',
        message,
        isSchedulingFailure: true,
        meta: uniqueMeta([
          cooldownLabel ? `本地限流冷却至 ${cooldownLabel}` : null,
          nonEmptyString(schedulingFailure.reason_summary),
          nonEmptyString(schedulingFailure.reason_label),
          nonEmptyString(schedulingFailure.reason),
          formatHttpStatus(schedulingFailure.status_code ?? detail.status_code),
          schedulingFailure.no_upstream_attempt ? '未进入上游执行' : null,
        ]),
      }
    }
  }

  const domain = fallbackDomain
  const message = nonEmptyString(domain?.message) ?? fallbackErrorMessage
  if (!message) return null

  return {
    title: '执行失败原因',
    message,
    isSchedulingFailure: false,
    meta: uniqueMeta([
      formatHttpStatus(domain?.status_code ?? detail.status_code),
      nonEmptyString(domain?.type),
      nonEmptyString(domain?.source),
    ]),
  }
}
