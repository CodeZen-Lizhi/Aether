import client from '../client'
import type { EndpointAPIKey, AllowedModels, ProxyConfig } from './types'

// Re-export types for convenience
export type { EndpointAPIKey, AllowedModels }

interface KeyRequestOptions {
  timeout?: number
}

/**
 * 能力定义类型
 */
export interface CapabilityDefinition {
  name: string
  display_name: string
  description: string
  match_mode: 'exclusive' | 'compatible'
  config_mode?: 'user_configurable' | 'auto_detect' | 'request_param'
  short_name?: string
}

/**
 * 模型支持的能力响应类型
 */
export interface ModelCapabilitiesResponse {
  model: string
  global_model_id?: string
  global_model_name?: string
  supported_capabilities: string[]
  capability_details: CapabilityDefinition[]
  error?: string
}

/**
 * 获取所有能力定义
 */
export async function getAllCapabilities(): Promise<CapabilityDefinition[]> {
  const response = await client.get('/api/capabilities')
  return response.data.capabilities
}

/**
 * 获取用户可配置的能力列表
 */
export async function getUserConfigurableCapabilities(): Promise<CapabilityDefinition[]> {
  const response = await client.get('/api/capabilities/user-configurable')
  return response.data.capabilities
}

/**
 * 获取指定模型支持的能力列表
 */
export async function getModelCapabilities(modelName: string): Promise<ModelCapabilitiesResponse> {
  const response = await client.get(`/api/capabilities/model/${encodeURIComponent(modelName)}`)
  return response.data
}

/**
 * 获取完整的 API Key（用于查看和复制）
 */
export interface RevealKeyResult {
  auth_type: 'api_key' | 'service_account' | 'bearer'
  api_key?: string
  refresh_token?: string
  auth_config?: string | Record<string, unknown>
}

export async function revealEndpointKey(keyId: string): Promise<RevealKeyResult> {
  const response = await client.get(`/api/admin/endpoints/keys/${keyId}/reveal`)
  return response.data
}

/**
 * 删除 Key
 */
export async function deleteEndpointKey(keyId: string): Promise<{ message: string }> {
  const response = await client.delete(`/api/admin/endpoints/keys/${keyId}`)
  return response.data
}

/**
 * 批量删除 Keys
 */
export interface BatchDeleteKeysResult {
  success_count: number
  failed_count: number
  failed: Array<{ id: string; error: string }>
}

export async function batchDeleteEndpointKeys(ids: string[]): Promise<BatchDeleteKeysResult> {
  const response = await client.post('/api/admin/endpoints/keys/batch-delete', { ids })
  return response.data
}


// ========== Provider 级别的 Keys API ==========


/**
 * 获取 Provider 的所有 Keys
 */
export interface ProviderKeysPageResponse {
  total: number
  page: number
  page_size: number
  keys: EndpointAPIKey[]
}

export interface ProviderKeysPageQuery {
  page?: number
  page_size?: number
}

type ProviderKeysPagePayload = ProviderKeysPageResponse | EndpointAPIKey[]

function normalizeProviderKeysPage(
  value: ProviderKeysPagePayload,
  page: number,
  pageSize: number,
): ProviderKeysPageResponse {
  if (Array.isArray(value)) {
    const start = value.length > pageSize ? (page - 1) * pageSize : 0
    const keys = value.slice(start, start + pageSize)
    return {
      total: value.length,
      page,
      page_size: pageSize,
      keys,
    }
  }

  const keys = Array.isArray(value.keys) ? value.keys : []
  return {
    total: typeof value.total === 'number' && Number.isFinite(value.total)
      ? value.total
      : keys.length,
    page: typeof value.page === 'number' && Number.isFinite(value.page)
      ? value.page
      : page,
    page_size: typeof value.page_size === 'number' && Number.isFinite(value.page_size)
      ? value.page_size
      : pageSize,
    keys,
  }
}

export async function getProviderKeysPage(
  providerId: string,
  params: ProviderKeysPageQuery = {},
): Promise<ProviderKeysPageResponse> {
  const page = params.page ?? 1
  const pageSize = params.page_size ?? 20
  const response = await client.get<ProviderKeysPagePayload>(
    `/api/admin/endpoints/providers/${providerId}/keys`,
    { params: { page, page_size: pageSize } },
  )
  return normalizeProviderKeysPage(response.data, page, pageSize)
}

export async function getProviderKeys(providerId: string): Promise<EndpointAPIKey[]> {
  // 后端默认 limit=100，这里主动分页拉取，避免账号数 >100 时前端被截断
  const pageSize = 1000
  let skip = 0
  const allKeys: EndpointAPIKey[] = []

  while (true) {
    const response = await client.get(`/api/admin/endpoints/providers/${providerId}/keys`, {
      params: { skip, limit: pageSize },
    })

    const batch = Array.isArray(response.data) ? (response.data as EndpointAPIKey[]) : []
    allKeys.push(...batch)

    if (batch.length < pageSize) break
    skip += pageSize
  }

  return allKeys
}

/**
 * 为 Provider 添加 Key
 */
export async function addProviderKey(
  providerId: string,
  data: {
    api_formats: string[]  // 支持的 API 格式列表（必填）
    api_key: string
    auth_type?: 'api_key' | 'service_account' | 'bearer'  // 认证类型
    auth_type_by_format?: Record<string, 'api_key' | 'bearer'> | null
    allow_auth_channel_mismatch_formats?: string[] | null
    auth_config?: Record<string, unknown>  // 认证配置（Vertex AI Service Account JSON）
    name: string
    rate_multipliers?: Record<string, number> | null  // 遗留字段：按 API 格式覆盖倍率已废弃，计费只读 default_rate_multiplier
    internal_priority?: number  // 同一供应商内调度优先级，数值越小越优先
    default_rate_multiplier?: number  // Key 级成本倍率（该密钥所有请求按此计费）
    rpm_limit?: number | null  // RPM 限制（留空=自适应模式）
    concurrent_limit?: number | null  // 并发请求上限（留空或 0=不限制）
    cache_ttl_minutes?: number
    max_probe_interval_minutes?: number
    allowed_models?: AllowedModels
    capabilities?: Record<string, boolean>
    note?: string
    is_active?: boolean
    auto_fetch_models?: boolean  // 是否启用自动获取模型
    model_include_patterns?: string[]  // 模型包含规则
    model_exclude_patterns?: string[]  // 模型排除规则
    proxy?: ProxyConfig | null  // Key 级别代理配置
  }
): Promise<EndpointAPIKey> {
  const response = await client.post(`/api/admin/endpoints/providers/${providerId}/keys`, data)
  return response.data
}

/**
 * 更新 Key
 */
export async function updateProviderKey(
  keyId: string,
  data: Partial<{
    api_formats: string[]  // 支持的 API 格式列表
    api_key: string
    auth_type: 'api_key' | 'service_account' | 'bearer'  // 认证类型
    auth_type_by_format: Record<string, 'api_key' | 'bearer'> | null
    allow_auth_channel_mismatch_formats: string[] | null
    auth_config: Record<string, unknown>  // 认证配置（Vertex AI Service Account JSON）
    name: string
    rate_multipliers: Record<string, number> | null  // 遗留字段：传 null 清空存量覆盖值；计费已不读取
    internal_priority: number  // 同一供应商内调度优先级，数值越小越优先
    default_rate_multiplier: number  // Key 级成本倍率（该密钥所有请求按此计费）
    rpm_limit: number | null  // RPM 限制（留空=自适应模式）
    concurrent_limit: number | null  // 并发请求上限（留空或 0=不限制）
    cache_ttl_minutes: number
    max_probe_interval_minutes: number
    allowed_models: AllowedModels
    locked_models: string[]  // 被锁定的模型列表
    capabilities: Record<string, boolean> | null
    is_active: boolean
    note: string
    auto_fetch_models: boolean  // 是否启用自动获取模型
    model_include_patterns: string[]  // 模型包含规则
    model_exclude_patterns: string[]  // 模型排除规则
    proxy: import('./types').ProxyConfig | null  // Key 级别代理配置
  }>,
  requestOptions?: KeyRequestOptions,
): Promise<EndpointAPIKey> {
  const response = await client.put(
    `/api/admin/endpoints/keys/${keyId}`,
    data,
    requestOptions,
  )
  return response.data
}

/** R11-6/R11-8: grouped-by-format 视图的 Key 条目（api_formats 为空的 key 已由后端按端点格式回退）。 */
export interface GroupedEndpointKey {
  id: string
  provider_id: string
  name: string
  auth_type: string
  api_key_masked?: string | null
  rate_multipliers: Record<string, number> | null  // 遗留字段：按格式覆盖倍率已废弃
  default_rate_multiplier?: number
  is_active: boolean
  provider_active: boolean
  provider_name?: string | null
  api_formats: string[]
  api_format: string
  health_score?: number
  circuit_breaker_open?: boolean
}

/** 拉取全部 Key（按 API 格式分组的原始 payload），客户端按 key id 去重。 */
export async function getEndpointKeysGroupedByFormat(): Promise<GroupedEndpointKey[]> {
  const response = await client.get<Record<string, GroupedEndpointKey[]>>(
    '/api/admin/endpoints/keys/grouped-by-format',
  )
  const grouped = response.data ?? {}
  const byId = new Map<string, GroupedEndpointKey>()
  for (const items of Object.values(grouped)) {
    for (const key of Array.isArray(items) ? items : []) {
      if (key && typeof key.id === 'string' && !byId.has(key.id)) {
        byId.set(key.id, key)
      }
    }
  }
  return [...byId.values()]
}
