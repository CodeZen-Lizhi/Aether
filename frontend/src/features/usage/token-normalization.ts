type UsageTokenLike = {
  effective_input_tokens?: number | null
  input_tokens?: number | null
  cache_creation_input_tokens?: number | null
  cache_creation_ephemeral_5m_input_tokens?: number | null
  cache_creation_ephemeral_1h_input_tokens?: number | null
  cache_read_input_tokens?: number | null
  api_format?: string | null
  endpoint_api_format?: string | null
}

function toNonNegativeNumber(value: number | null | undefined): number {
  return typeof value === 'number' && Number.isFinite(value) ? Math.max(value, 0) : 0
}

function apiFamily(apiFormat: string | null | undefined): string {
  return String(apiFormat || '')
    .split(':', 1)[0]
    .trim()
    .toLowerCase()
}

export function getCacheCreationTokens(usage: UsageTokenLike): number {
  const explicit = toNonNegativeNumber(usage.cache_creation_input_tokens)
  const classified = toNonNegativeNumber(usage.cache_creation_ephemeral_5m_input_tokens)
    + toNonNegativeNumber(usage.cache_creation_ephemeral_1h_input_tokens)
  if (explicit === 0 && classified > 0) {
    return classified
  }
  return explicit
}

export function getCacheReadTokens(usage: UsageTokenLike): number {
  return toNonNegativeNumber(usage.cache_read_input_tokens)
}

export function getEffectiveInputTokens(usage: UsageTokenLike): number {
  const explicit = toNonNegativeNumber(usage.effective_input_tokens)
  if (explicit > 0) {
    return explicit
  }

  const inputTokens = toNonNegativeNumber(usage.input_tokens)
  const cacheReadTokens = toNonNegativeNumber(usage.cache_read_input_tokens)
  if (inputTokens === 0 || cacheReadTokens === 0) {
    return inputTokens
  }

  switch (apiFamily(usage.endpoint_api_format || usage.api_format)) {
    case 'openai':
    case 'gemini':
    case 'google':
      return Math.max(inputTokens - cacheReadTokens, 0)
    default:
      return inputTokens
  }
}

/**
 * 按后端缓存命中口径归一化输入上下文：新鲜输入、缓存写入与缓存读取之和。
 * OpenAI 的原始输入可能已包含缓存项，需先扣除；Claude 的输入不含缓存写入。
 */
export function getTotalInputContextTokens(usage: UsageTokenLike): number {
  const inputTokens = toNonNegativeNumber(usage.input_tokens)
  const cacheCreationTokens = getCacheCreationTokens(usage)
  const cacheReadTokens = getCacheReadTokens(usage)

  switch (apiFamily(usage.endpoint_api_format || usage.api_format)) {
    case 'openai':
      return Math.max(inputTokens - cacheCreationTokens - cacheReadTokens, 0)
        + cacheCreationTokens
        + cacheReadTokens
    case 'gemini':
    case 'google':
      return Math.max(inputTokens - cacheReadTokens, 0) + cacheReadTokens
    case 'claude':
    case 'anthropic':
      return inputTokens + cacheCreationTokens + cacheReadTokens
    default:
      return (cacheCreationTokens > 0 ? inputTokens + cacheCreationTokens : inputTokens)
        + cacheReadTokens
  }
}

/**
 * 返回缓存读取 Token 占归一化输入上下文的百分比，保留两位小数。
 * 没有输入上下文时返回 null，避免把缺失数据展示成 0% 命中。
 */
export function getCacheHitRate(usage: UsageTokenLike): number | null {
  const totalInputContext = getTotalInputContextTokens(usage)
  if (totalInputContext <= 0) {
    return null
  }
  const rate = getCacheReadTokens(usage) / totalInputContext * 100
  return Math.round(rate * 100) / 100
}
