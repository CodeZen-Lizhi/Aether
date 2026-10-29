import {
  apiFormatPermissionCovers,
  normalizeApiFormatAlias,
} from '@/api/endpoints/types/api-format'
import type { ProviderModelMapping } from '@/api/endpoints/types'
import type { TestModelRequest } from '@/api/endpoints/providers'
import {
  modelTestKeySupportsEndpoint,
  modelSupportsImageGeneration,
  normalizeModelTestStringList,
  type ModelTestImageSource,
} from './model-test-capabilities'

export {
  formatModelTestDiagnostic,
  getOpenAiImageModelTestCapability,
  getOpenAiImageModelTestMaxGenerationCount,
  isModelTestableApiFormat,
  isModelTestableEndpoint,
  modelTestKeySupportsEndpoint,
  selectPreferredModelTestEndpoint,
} from './model-test-capabilities'
export type {
  ModelTestEndpointSource,
  ModelTestImageSource,
  ModelTestKeySource,
} from './model-test-capabilities'
export {
  extractModelTestImagePreviews,
  extractModelTestResponsePreview,
} from './model-test-preview'
export type { ModelTestImagePreview } from './model-test-preview'

const DEFAULT_MODEL_TEST_MESSAGE = 'Hello! This is a test message.'

function buildModelTestSearchSessionId(): string {
  const id = globalThis.crypto?.randomUUID?.()
    ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`
  return `aether-model-test-${id}`
}

type ModelTestMappingSource = {
  provider_model_name: string
  provider_model_mappings?: ProviderModelMapping[] | null
}

export type ModelTestMappingEndpoint = {
  id: string
  api_format: string
}

export type ModelTestMappedModelOption = {
  name: string
  priority: number
}

/** 模型测试候选 Key 的最小字段集合，用于前端与后端保持相同的端点及模型权限筛选。 */
export type ModelTestCandidateKey = {
  id: string
  api_formats?: string[] | null
  is_active?: boolean | null
  allowed_models?: string[] | null
}

/** 测试 Key 选择项的最小展示字段；只用于生成用户可识别的名称。 */
export type ModelTestKeyDisplaySource = {
  id: string
  name?: string | null
  api_key_masked?: string | null
}

/** 返回测试 Key 的名称；没有名称时才回退到掩码或标识，不拼接认证类型。 */
export function formatModelTestKeyName(key: ModelTestKeyDisplaySource): string {
  return key.name?.trim() || key.api_key_masked?.trim() || key.id
}

/** 单个“映射 × Key”候选；mappingName 为空表示模型没有映射时的直连回退。 */
export type ModelTestCandidate = {
  mappingName: string | null
  effectiveModel: string
  keyId: string
}

export type BuildModelTestCandidatePlanInput = {
  requestedModelName: string
  model: ModelTestMappingSource | null | undefined
  endpoint: ModelTestMappingEndpoint | null | undefined
  keys: ModelTestCandidateKey[]
  selectedMappingName?: string | null
  selectedKeyId?: string | null
}

export function modelTestMappingScopeMatchesEndpoint(
  apiFormats: string[] | null | undefined,
  endpointIds: string[] | null | undefined,
  endpoint: ModelTestMappingEndpoint,
): boolean {
  const normalizedApiFormats = normalizeModelTestStringList(apiFormats)
  const endpointFormat = normalizeApiFormatAlias(endpoint.api_format)
  const apiFormatMatches = normalizedApiFormats.length === 0
    || normalizedApiFormats.some(format => apiFormatPermissionCovers(format, endpointFormat))
  const normalizedEndpointIds = normalizeModelTestStringList(endpointIds)
  const endpointMatches = normalizedEndpointIds.length === 0
    || normalizedEndpointIds.includes(endpoint.id)
  return apiFormatMatches && endpointMatches
}

export function listModelTestMappedModelOptions(
  model: ModelTestMappingSource | null | undefined,
  endpoint: ModelTestMappingEndpoint | null | undefined,
): ModelTestMappedModelOption[] {
  if (!model || !endpoint || !Array.isArray(model.provider_model_mappings)) return []

  const matchedMappings = model.provider_model_mappings
    .filter(mapping => mapping.name.trim())
    .filter(mapping => modelTestMappingScopeMatchesEndpoint(
      mapping.api_formats,
      mapping.endpoint_ids,
      endpoint,
    ))
    .sort((left, right) => {
      const leftPriority = Number.isFinite(left.priority) ? left.priority : 1
      const rightPriority = Number.isFinite(right.priority) ? right.priority : 1
      return leftPriority - rightPriority || left.name.localeCompare(right.name)
    })
  const seen = new Set<string>()
  return matchedMappings.flatMap((mapping) => {
    const name = mapping.name.trim()
    const dedupeKey = name.toLowerCase()
    if (seen.has(dedupeKey)) return []
    seen.add(dedupeKey)
    return [{
      name,
      priority: Number.isFinite(mapping.priority) ? mapping.priority : 1,
    }]
  })
}

export function normalizeModelTestMappedModelSelection(
  options: ModelTestMappedModelOption[],
  preferredName: string | null | undefined,
): string | null {
  const preferred = preferredName?.trim()
  if (!preferred) return null
  return options.find(option => option.name === preferred)?.name ?? null
}

const MODEL_DIRECTIVE_SUFFIXES = new Set([
  'none',
  'minimal',
  'low',
  'medium',
  'high',
  'xhigh',
  'max',
  'fast',
])

/** 解析已启用的模型名后缀，返回上游实际模型名；普通模型名返回空。 */
export function modelTestDirectiveBaseModel(modelName: string): string | null {
  let base = modelName.trim()
  let matched = false
  let matchedReasoning = false
  let matchedServiceTier = false

  while (true) {
    const separator = base.lastIndexOf('-')
    if (separator <= 0) break
    const suffix = base.slice(separator + 1).toLowerCase()
    if (!MODEL_DIRECTIVE_SUFFIXES.has(suffix)) break
    const isServiceTier = suffix === 'fast'
    if ((isServiceTier && matchedServiceTier) || (!isServiceTier && matchedReasoning)) {
      return null
    }
    base = base.slice(0, separator).trim()
    matched = true
    matchedServiceTier ||= isServiceTier
    matchedReasoning ||= !isServiceTier
  }

  return matched && base ? base : null
}

/** Key 的空 allowed_models 表示不限制；否则请求模型或映射后的模型任一命中即可。 */
export function modelTestKeyAllowsModel(
  key: Pick<ModelTestCandidateKey, 'allowed_models'>,
  requestedModelName: string,
  effectiveModelName: string,
): boolean {
  const allowedModels = normalizeModelTestStringList(key.allowed_models)
  if (allowedModels.length === 0) return true

  const requested = requestedModelName.trim().toLowerCase()
  const effective = effectiveModelName.trim().toLowerCase()
  const directiveBase = modelTestDirectiveBaseModel(requestedModelName)?.toLowerCase() ?? null
  return allowedModels.some((model) => {
    const normalized = model.toLowerCase()
    return normalized === requested
      || normalized === effective
      || (directiveBase !== null && normalized === directiveBase)
  })
}

/**
 * 生成当前端点下的“映射 × Key”候选。
 * 有映射时仅枚举映射；没有映射时才回退到请求模型。Key 不兼容端点或缺少模型权限时不生成组合。
 */
export function buildModelTestCandidatePlan(
  input: BuildModelTestCandidatePlanInput,
): ModelTestCandidate[] {
  const requestedModelName = input.requestedModelName.trim()
  const endpoint = input.endpoint
  if (!requestedModelName || !endpoint) return []

  const mappingOptions = listModelTestMappedModelOptions(input.model, endpoint)
  const selectedMapping = normalizeModelTestMappedModelSelection(
    mappingOptions,
    input.selectedMappingName,
  )
  const effectiveModels = selectedMapping
    ? [{ name: selectedMapping, mapped: true }]
    : mappingOptions.length > 0
      ? mappingOptions.map(option => ({ name: option.name, mapped: true }))
      : [{ name: requestedModelName, mapped: false }]
  const selectedKeyId = input.selectedKeyId?.trim() || ''

  return effectiveModels.flatMap(({ name, mapped }) => input.keys.flatMap((key) => {
    if (selectedKeyId && key.id !== selectedKeyId) return []
    if (!modelTestKeySupportsEndpoint(key, endpoint)) return []
    if (!modelTestKeyAllowsModel(key, requestedModelName, name)) return []

    return [{
      mappingName: mapped ? name : null,
      effectiveModel: name,
      keyId: key.id,
    }]
  }))
}

export function setModelTestRequestBodyModel(draft: string, modelName: string): string {
  const parsed = parseModelTestRequestBodyDraft(draft)
  if (!parsed.value || parsed.error) return draft

  return JSON.stringify({
    ...parsed.value,
    model: modelName,
  }, null, 2)
}

export function syncModelTestRequestBodyDraft(
  draft: string,
  resetValue: string,
  nextResetValue: string,
  modelName?: string | null,
): { draft: string; resetValue: string } {
  const nextReset = modelName?.trim()
    ? setModelTestRequestBodyModel(nextResetValue, modelName.trim())
    : nextResetValue
  const draftIsUntouched = !draft || draft === resetValue

  if (draftIsUntouched) {
    return {
      draft: nextReset,
      resetValue: nextReset,
    }
  }

  return {
    draft: modelName?.trim()
      ? setModelTestRequestBodyModel(draft, modelName.trim())
      : draft,
    resetValue: nextReset,
  }
}

export function buildExactModelMappingTestRequest(
  providerId: string,
  modelName: string,
  apiFormat: string | null | undefined,
): TestModelRequest {
  return {
    provider_id: providerId,
    model_name: modelName,
    mode: 'direct',
    apply_model_mapping: false,
    api_format: apiFormat || undefined,
  }
}

export function buildDefaultModelTestRequestBody(
  modelName: string,
  apiFormat?: string | null,
  model?: ModelTestImageSource | null,
): string {
  const normalizedApiFormat = normalizeApiFormatAlias(apiFormat ?? '')

  if (normalizedApiFormat === 'aliyun:multimodal_embedding') {
    return JSON.stringify({
      model: modelName,
      input: [
        { text: 'White running shoes, lightweight and breathable.' },
        { image: 'https://dashscope.oss-cn-beijing.aliyuncs.com/images/256_1.png' },
      ],
      parameters: {
        enable_fusion: true,
      },
    }, null, 2)
  }

  if (normalizedApiFormat.endsWith(':embedding')) {
    return JSON.stringify({
      model: modelName,
      input: 'This is a test embedding input.',
    }, null, 2)
  }

  if (normalizedApiFormat.endsWith(':rerank')) {
    return JSON.stringify({
      model: modelName,
      query: 'Apple',
      documents: [
        'apple',
        'banana',
        'fruit',
        'vegetable',
      ],
      return_documents: true,
      top_n: 4,
    }, null, 2)
  }

  if (normalizedApiFormat === 'openai:image') {
    return JSON.stringify({
      model: modelName,
      prompt: DEFAULT_MODEL_TEST_MESSAGE,
      n: 1,
      size: '1024x1024',
      stream: true,
    }, null, 2)
  }

  if (normalizedApiFormat === 'gemini:interactions') {
    return JSON.stringify({
      model: modelName,
      input: DEFAULT_MODEL_TEST_MESSAGE,
      stream: true,
    }, null, 2)
  }

  if (normalizedApiFormat === 'openai:search') {
    return JSON.stringify({
      id: buildModelTestSearchSessionId(),
      model: modelName,
      input: DEFAULT_MODEL_TEST_MESSAGE,
      commands: {
        search_query: [{ q: DEFAULT_MODEL_TEST_MESSAGE }],
      },
      max_output_tokens: 256,
    }, null, 2)
  }

  if (normalizedApiFormat === 'openai:responses' && modelSupportsImageGeneration(model)) {
    return JSON.stringify({
      model: modelName,
      input: DEFAULT_MODEL_TEST_MESSAGE,
      tools: [
        {
          type: 'image_generation',
          size: '1024x1024',
          output_format: 'png',
        },
      ],
      tool_choice: {
        type: 'image_generation',
      },
      stream: true,
    }, null, 2)
  }

  return JSON.stringify({
    model: modelName,
    messages: [
      {
        role: 'user',
        content: DEFAULT_MODEL_TEST_MESSAGE,
      },
    ],
    max_tokens: 30,
    stream: true,
  }, null, 2)
}

export function buildDefaultModelTestRequestHeaders(): string {
  return JSON.stringify({}, null, 2)
}

function parseModelTestJsonObjectDraft(
  draft: string,
  options: {
    emptyValue: Record<string, unknown> | null
    emptyError: string | null
    invalidTypeError: string
  },
): { value: Record<string, unknown> | null; error: string | null } {
  const normalized = draft.trim()
  if (!normalized) {
    return {
      value: options.emptyValue,
      error: options.emptyError,
    }
  }

  try {
    const parsed = JSON.parse(normalized)
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return {
        value: null,
        error: options.invalidTypeError,
      }
    }
    return {
      value: parsed as Record<string, unknown>,
      error: null,
    }
  } catch (error) {
    return {
      value: null,
      error: error instanceof Error ? error.message : '无效的 JSON',
    }
  }
}

export function parseModelTestRequestBodyDraft(
  draft: string,
): { value: Record<string, unknown> | null; error: string | null } {
  return parseModelTestJsonObjectDraft(draft, {
    emptyValue: null,
    emptyError: '测试请求体不能为空',
    invalidTypeError: '测试请求体必须是 JSON 对象',
  })
}

export function parseModelTestRequestHeadersDraft(
  draft: string,
): { value: Record<string, unknown> | null; error: string | null } {
  return parseModelTestJsonObjectDraft(draft, {
    emptyValue: {},
    emptyError: null,
    invalidTypeError: '测试请求头必须是 JSON 对象',
  })
}
