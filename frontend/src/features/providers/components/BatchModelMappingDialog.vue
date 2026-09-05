<template>
  <Dialog
    :model-value="open"
    title="批量添加模型映射"
    description="选择客户端模型，并为它们指定一个提供商模型"
    :icon="Tags"
    size="5xl"
    :no-padding="true"
    @update:model-value="handleDialogUpdate"
  >
    <div class="flex min-h-0 flex-col">
      <div class="flex flex-wrap items-center justify-between gap-3 border-b border-border/70 bg-muted/20 px-4 py-3 sm:px-6">
        <p class="text-sm text-muted-foreground">
          每次选择一个提供商模型生成配对草稿，保存前可逐项检查。
        </p>
        <div class="flex flex-wrap items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            class="h-8"
            :disabled="models.length === 0 || upstreamModels.length === 0"
            @click="autoMatch"
          >
            <Sparkles
              class="mr-1.5 h-3.5 w-3.5"
              aria-hidden="true"
            />
            按名称自动匹配
          </Button>
          <div
            class="flex items-center gap-2 text-xs"
            aria-live="polite"
          >
            <span class="rounded-md bg-primary/10 px-2 py-1 text-primary">
              已选客户端 {{ selectedClientCount }}
            </span>
            <span class="rounded-md bg-muted px-2 py-1 text-muted-foreground">
              待保存 {{ pendingCount }}
            </span>
          </div>
        </div>
      </div>

      <div class="grid min-h-0 gap-4 p-4 sm:p-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.1fr)]">
        <section
          class="flex h-72 min-h-0 flex-col rounded-lg border border-border/70 bg-card lg:h-[min(48dvh,26rem)]"
          aria-labelledby="batch-client-title"
        >
          <div class="shrink-0 border-b border-border/60 p-3">
            <div class="mb-2 flex items-center justify-between gap-2">
              <div>
                <h2
                  id="batch-client-title"
                  class="text-sm font-semibold"
                >
                  客户端模型
                </h2>
                <p class="text-xs text-muted-foreground">
                  可多选，待保存目标会显示在每一行
                </p>
              </div>
              <button
                type="button"
                class="text-xs font-medium text-primary hover:underline disabled:cursor-not-allowed disabled:opacity-50"
                :disabled="filteredClients.length === 0"
                :aria-pressed="allClientsSelected"
                @click="toggleAllClients"
              >
                {{ allClientsSelected ? '取消全选' : '全选' }}
              </button>
            </div>
            <div class="relative">
              <Search
                class="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground"
                aria-hidden="true"
              />
              <Input
                v-model="clientSearch"
                class="h-9 pl-8"
                placeholder="搜索客户端模型..."
                aria-label="搜索客户端模型"
                autofocus
              />
            </div>
          </div>

          <fieldset
            class="min-h-0 flex-1 overflow-y-auto p-2"
            aria-label="客户端模型列表"
          >
            <legend class="sr-only">
              客户端模型列表
            </legend>
            <div
              v-for="model in filteredClients"
              :key="model.id"
              class="mb-1 flex min-h-12 items-center gap-2 rounded-md border border-transparent px-2.5 py-2 transition-colors hover:bg-muted/60"
              :class="drafts[model.id] ? 'border-primary/30 bg-primary/5' : ''"
            >
              <label
                :for="`batch-client-${model.id}`"
                class="flex min-w-0 flex-1 cursor-pointer items-center gap-2"
              >
                <input
                  :id="`batch-client-${model.id}`"
                  v-model="selectedClientIds"
                  type="checkbox"
                  :value="model.id"
                  class="h-4 w-4 shrink-0 rounded border-border/70 accent-primary"
                >
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-sm font-medium">
                    {{ clientModelLabel(model) }}
                  </span>
                  <span class="block truncate font-mono text-xs text-muted-foreground">
                    {{ model.provider_model_name }}
                  </span>
                </span>
              </label>

              <span
                v-if="!drafts[model.id]"
                class="shrink-0 text-xs text-muted-foreground"
                :title="existingMappingsTitle(model)"
              >
                默认映射 {{ existingDefaultMappingCount(model) }}
              </span>
              <div
                v-else
                class="flex min-w-0 max-w-32 items-center gap-1 rounded-md bg-primary/10 px-2 py-1 text-xs text-primary sm:max-w-44"
              >
                <span
                  class="truncate font-mono"
                  :title="drafts[model.id]"
                >
                  {{ drafts[model.id] }}
                </span>
                <button
                  type="button"
                  class="shrink-0 rounded p-0.5 hover:bg-primary/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/50"
                  :aria-label="`清除 ${clientModelLabel(model)} 的待保存映射`"
                  title="清除草稿"
                  @click="clearDraft(model.id)"
                >
                  <X
                    class="h-3.5 w-3.5"
                    aria-hidden="true"
                  />
                </button>
              </div>
            </div>

            <div
              v-if="filteredClients.length === 0"
              class="flex min-h-48 items-center justify-center px-4 text-center text-sm text-muted-foreground"
            >
              {{ clientSearch ? '无匹配客户端模型' : '暂无客户端模型' }}
            </div>
          </fieldset>
        </section>

        <section
          class="flex h-72 min-h-0 flex-col rounded-lg border border-border/70 bg-card lg:h-[min(48dvh,26rem)]"
          aria-labelledby="batch-upstream-title"
        >
          <div class="shrink-0 border-b border-border/60 p-3">
            <div class="mb-2 flex items-center justify-between gap-2">
              <div>
                <h2
                  id="batch-upstream-title"
                  class="text-sm font-semibold"
                >
                  提供商模型
                </h2>
                <p class="text-xs text-muted-foreground">
                  一次选择一个目标模型
                </p>
              </div>
              <button
                type="button"
                class="rounded-md p-2 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                :disabled="loadingUpstream"
                title="刷新提供商模型"
                aria-label="刷新提供商模型"
                @click="fetchUpstreamModels(true)"
              >
                <Loader2
                  v-if="loadingUpstream"
                  class="h-4 w-4 animate-spin"
                  aria-hidden="true"
                />
                <RefreshCw
                  v-else
                  class="h-4 w-4"
                  aria-hidden="true"
                />
              </button>
            </div>
            <div class="relative">
              <Search
                class="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground"
                aria-hidden="true"
              />
              <Input
                v-model="upstreamSearch"
                class="h-9 pl-8"
                placeholder="搜索提供商模型..."
                aria-label="搜索提供商模型"
              />
            </div>
          </div>

          <div
            class="min-h-0 flex-1 overflow-y-auto p-2"
            role="radiogroup"
            aria-label="提供商模型列表"
          >
            <div
              v-if="loadingUpstream"
              class="flex min-h-48 items-center justify-center text-muted-foreground"
            >
              <Loader2
                class="h-6 w-6 animate-spin text-primary"
                aria-hidden="true"
              />
              <span class="sr-only">正在加载提供商模型</span>
            </div>

            <template v-else>
              <label
                v-if="canAddCustom"
                class="mb-2 flex min-h-11 cursor-pointer items-center gap-2 rounded-md border border-dashed border-primary/40 bg-primary/5 px-3 py-2 text-sm hover:bg-primary/10"
              >
                <input
                  type="radio"
                  name="batch-upstream-model"
                  :value="upstreamSearch.trim()"
                  class="h-4 w-4 accent-primary"
                  :checked="selectedUpstreamName === upstreamSearch.trim()"
                  @change="selectCustomModel"
                >
                <Plus
                  class="h-4 w-4 text-primary"
                  aria-hidden="true"
                />
                <span class="truncate font-mono">
                  使用自定义提供商模型“{{ upstreamSearch.trim() }}”
                </span>
              </label>

              <label
                v-for="model in filteredUpstreamModels"
                :key="model.id"
                class="mb-1 flex min-h-11 cursor-pointer items-center gap-2 rounded-md px-2.5 py-2 transition-colors hover:bg-muted/60"
                :class="selectedUpstreamName === model.id ? 'bg-primary/5 text-primary' : ''"
              >
                <input
                  v-model="selectedUpstreamName"
                  type="radio"
                  name="batch-upstream-model"
                  :value="model.id"
                  class="h-4 w-4 shrink-0 accent-primary"
                >
                <span class="min-w-0 flex-1 truncate font-mono text-sm">
                  {{ model.id }}
                </span>
                <span
                  v-if="model.owned_by"
                  class="shrink-0 text-xs text-muted-foreground"
                >
                  {{ model.owned_by }}
                </span>
              </label>

              <div
                v-if="filteredUpstreamModels.length === 0 && !canAddCustom"
                class="flex min-h-40 flex-col items-center justify-center text-center text-muted-foreground"
              >
                <CloudDownload
                  class="mb-2 h-8 w-8 opacity-30"
                  aria-hidden="true"
                />
                <p class="text-sm">
                  {{ upstreamSearch ? '无匹配提供商模型' : '暂无提供商模型' }}
                </p>
                <p class="mt-1 text-xs">
                  点击右上角刷新按钮获取模型
                </p>
              </div>
            </template>
          </div>

          <div class="shrink-0 space-y-2 border-t border-border/60 bg-muted/20 p-3">
            <div class="text-xs text-muted-foreground">
              <span>默认作用于全部端点和请求</span>
            </div>
            <Button
              class="w-full"
              :disabled="selectedClientCount === 0 || !selectedUpstreamName"
              @click="applyPair"
            >
              <Link2
                class="mr-2 h-4 w-4"
                aria-hidden="true"
              />
              应用到已选客户端
            </Button>
          </div>
        </section>
      </div>

      <div class="border-t border-border/70 bg-muted/20 px-4 py-3 sm:px-6">
        <p class="text-xs leading-5 text-muted-foreground">
          批量添加默认作用于全部端点和请求，精细范围可在保存后通过单条编辑调整。
        </p>
        <p
          v-if="errorMessage"
          class="mt-2 rounded-md bg-destructive/10 px-3 py-2 text-xs text-destructive"
          role="alert"
        >
          {{ errorMessage }}
        </p>
        <p
          v-if="successMessage"
          class="mt-2 rounded-md bg-emerald-500/10 px-3 py-2 text-xs text-emerald-700 dark:text-emerald-300"
          role="status"
          aria-live="polite"
        >
          {{ successMessage }}
        </p>
      </div>
    </div>

    <template #footer>
      <div class="flex w-full flex-wrap items-center justify-between gap-3">
        <span
          class="text-xs text-muted-foreground"
          aria-live="polite"
        >
          {{ pendingCount ? `${pendingCount} 个客户端模型待保存` : '没有待保存草稿' }}
        </span>
        <div class="flex items-center gap-2">
          <Button
            variant="outline"
            :disabled="saving"
            @click="closeDialog"
          >
            {{ pendingCount ? '取消' : '关闭' }}
          </Button>
          <Button
            :disabled="saving || pendingCount === 0"
            @click="saveMappings"
          >
            <Loader2
              v-if="saving"
              class="mr-2 h-4 w-4 animate-spin"
              aria-hidden="true"
            />
            {{ saving ? '保存中...' : `保存 ${pendingCount} 条映射` }}
          </Button>
        </div>
      </div>
    </template>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  CloudDownload,
  Link2,
  Loader2,
  Plus,
  RefreshCw,
  Search,
  Sparkles,
  Tags,
  X,
} from 'lucide-vue-next'
import { Button, Dialog, Input } from '@/components/ui'
import { updateModel } from '@/api/endpoints/models'
import type { Model, ProviderModelMapping, UpstreamModel } from '@/api/endpoints'
import { useConfirm } from '@/composables/useConfirm'
import { useToast } from '@/composables/useToast'
import { parseApiError } from '@/utils/errorParser'
import { useUpstreamModelsCache } from '../composables/useUpstreamModelsCache'

interface Props {
  open: boolean
  providerId: string
  models: Model[]
  hasAutoFetchKey?: boolean
}

interface DraftEntry {
  modelId: string
  upstreamName: string
}

interface SaveResult extends DraftEntry {
  status: 'fulfilled' | 'rejected'
  changed: boolean
  reason?: unknown
}

const props = defineProps<Props>()
const emit = defineEmits<{
  'update:open': [value: boolean]
  saved: []
}>()

const { fetchModels: fetchCachedModels } = useUpstreamModelsCache()
const { confirm } = useConfirm()
const {
  error: showError,
  success: showSuccess,
  warning: showWarning,
} = useToast()

const clientSearch = ref('')
const upstreamSearch = ref('')
const selectedClientIds = ref<string[]>([])
const selectedUpstreamName = ref('')
const upstreamModels = ref<UpstreamModel[]>([])
const customModels = ref<string[]>([])
const drafts = ref<Record<string, string>>({})
const locallySavedNames = ref<Record<string, string[]>>({})
const loadingUpstream = ref(false)
const saving = ref(false)
const errorMessage = ref('')
const successMessage = ref('')

const filteredClients = computed(() => {
  const query = clientSearch.value.trim().toLowerCase()
  return props.models.filter(model => {
    if (!query) return true
    const searchableText = `${clientModelLabel(model)} ${model.provider_model_name}`.toLowerCase()
    return searchableText.includes(query)
  })
})

const allUpstreamModels = computed<UpstreamModel[]>(() => {
  const byId = new Map<string, UpstreamModel>()
  for (const model of upstreamModels.value) {
    byId.set(model.id, model)
  }
  for (const id of customModels.value) {
    if (!byId.has(id)) {
      byId.set(id, { id, api_formats: [] })
    }
  }
  return [...byId.values()]
})

const filteredUpstreamModels = computed(() => {
  const query = upstreamSearch.value.trim().toLowerCase()
  return allUpstreamModels.value.filter(model => !query || model.id.toLowerCase().includes(query))
})

const selectedClientCount = computed(() => selectedClientIds.value.length)
const pendingCount = computed(() => Object.keys(drafts.value).length)
const allClientsSelected = computed(() => {
  return filteredClients.value.length > 0
    && filteredClients.value.every(model => selectedClientIds.value.includes(model.id))
})
const canAddCustom = computed(() => {
  const name = upstreamSearch.value.trim()
  return Boolean(name) && !allUpstreamModels.value.some(model => model.id === name)
})

function clientModelLabel(model: Model): string {
  return model.global_model_display_name || model.global_model_name || model.provider_model_name
}

function hasScopeValues(values?: string[]): boolean {
  return Array.isArray(values) && values.length > 0
}

function isDefaultScopeMapping(mapping: ProviderModelMapping): boolean {
  return !hasScopeValues(mapping.api_formats)
    && !hasScopeValues(mapping.endpoint_ids)
    && !hasScopeValues(mapping.operations)
}

function defaultMappings(model: Model): ProviderModelMapping[] {
  return currentMappings(model).filter(isDefaultScopeMapping)
}

function existingDefaultMappingCount(model: Model): number {
  return defaultMappings(model).length
}

function existingMappingsTitle(model: Model): string {
  const names = defaultMappings(model).map(mapping => mapping.name)
  return names.length > 0 ? names.join(', ') : '尚未配置默认范围映射'
}

function currentMappings(model: Model): ProviderModelMapping[] {
  const existing = [...(model.provider_model_mappings ?? [])]
  for (const name of locallySavedNames.value[model.id] ?? []) {
    if (!existing.some(mapping => isDefaultScopeMapping(mapping) && mapping.name === name)) {
      existing.push({ name, priority: 1 })
    }
  }
  return existing
}

function hasDefaultMapping(model: Model, name: string): boolean {
  return currentMappings(model).some(mapping => {
    return isDefaultScopeMapping(mapping) && mapping.name === name
  })
}

function toggleAllClients() {
  const visibleIds = new Set(filteredClients.value.map(model => model.id))
  selectedClientIds.value = allClientsSelected.value
    ? selectedClientIds.value.filter(id => !visibleIds.has(id))
    : [...new Set([...selectedClientIds.value, ...visibleIds])]
}

function selectCustomModel() {
  const name = upstreamSearch.value.trim()
  if (!name) return
  customModels.value = [...new Set([...customModels.value, name])]
  selectedUpstreamName.value = name
}

function clearDraft(modelId: string) {
  const next = { ...drafts.value }
  delete next[modelId]
  drafts.value = next
  successMessage.value = ''
}

function applyPair() {
  const upstreamName = selectedUpstreamName.value.trim()
  if (!upstreamName || selectedClientIds.value.length === 0) return

  const next = { ...drafts.value }
  let changedCount = 0
  for (const modelId of selectedClientIds.value) {
    const model = props.models.find(item => item.id === modelId)
    if (!model) continue

    if (hasDefaultMapping(model, upstreamName)) {
      if (next[modelId]) {
        delete next[modelId]
      }
      continue
    }

    if (next[modelId] !== upstreamName) {
      next[modelId] = upstreamName
      changedCount += 1
    }
  }

  drafts.value = next
  selectedClientIds.value = []
  selectedUpstreamName.value = ''
  upstreamSearch.value = ''
  errorMessage.value = ''
  successMessage.value = changedCount > 0
    ? `已生成 ${changedCount} 条映射草稿`
    : '没有需要新增的映射，重复项已忽略'
}

function exactUpstreamMatch(model: Model): string | undefined {
  const upstreamNames = upstreamModels.value.map(item => item.id)
  const candidates = [model.provider_model_name, model.global_model_name]
    .map(name => name?.trim())
    .filter((name): name is string => Boolean(name))

  for (const candidate of candidates) {
    if (upstreamNames.includes(candidate)) return candidate
  }

  const namesByLowerCase = new Map<string, string[]>()
  for (const name of upstreamNames) {
    const key = name.toLowerCase()
    namesByLowerCase.set(key, [...(namesByLowerCase.get(key) ?? []), name])
  }
  for (const candidate of candidates) {
    const matches = namesByLowerCase.get(candidate.toLowerCase()) ?? []
    if (matches.length === 1) return matches[0]
  }
  return undefined
}

function autoMatch() {
  const next = { ...drafts.value }
  let matchedCount = 0

  for (const model of filteredClients.value) {
    if (next[model.id] || existingDefaultMappingCount(model) > 0) continue
    const upstreamName = exactUpstreamMatch(model)
    if (!upstreamName || hasDefaultMapping(model, upstreamName)) continue
    next[model.id] = upstreamName
    matchedCount += 1
  }

  drafts.value = next
  selectedClientIds.value = []
  selectedUpstreamName.value = ''
  errorMessage.value = ''
  successMessage.value = matchedCount > 0
    ? `已按名称生成 ${matchedCount} 条映射草稿`
    : '当前列表没有可确定的同名映射'
}

async function fetchUpstreamModels(forceRefresh = false) {
  if (!props.providerId) return
  loadingUpstream.value = true
  errorMessage.value = ''
  try {
    const result = await fetchCachedModels(props.providerId, undefined, forceRefresh)
    upstreamModels.value = result.models
    if (result.warning) {
      showWarning(result.warning, '获取提供商模型提示')
    }
    if (result.error) {
      errorMessage.value = result.error
    }
  } catch (error: unknown) {
    errorMessage.value = parseApiError(error, '获取提供商模型失败')
  } finally {
    loadingUpstream.value = false
  }
}

function resetState() {
  clientSearch.value = ''
  upstreamSearch.value = ''
  selectedClientIds.value = []
  selectedUpstreamName.value = ''
  upstreamModels.value = []
  customModels.value = []
  drafts.value = {}
  locallySavedNames.value = {}
  errorMessage.value = ''
  successMessage.value = ''
}

async function handleDialogUpdate(value: boolean) {
  if (value) {
    emit('update:open', true)
    return
  }
  await closeDialog()
}

async function closeDialog() {
  if (saving.value) return
  if (pendingCount.value > 0) {
    const confirmed = await confirm({
      title: '放弃更改',
      message: '有未保存的映射草稿，确定要关闭吗？',
      confirmText: '放弃并关闭',
      cancelText: '继续编辑',
      variant: 'warning',
    })
    if (!confirmed) return
  }
  emit('update:open', false)
}

function rememberSavedMapping(modelId: string, upstreamName: string) {
  locallySavedNames.value = {
    ...locallySavedNames.value,
    [modelId]: [...new Set([
      ...(locallySavedNames.value[modelId] ?? []),
      upstreamName,
    ])],
  }
}

async function saveDraft(entry: DraftEntry): Promise<SaveResult> {
  const model = props.models.find(item => item.id === entry.modelId)
  if (!model) {
    return { ...entry, status: 'rejected', changed: false, reason: new Error('客户端模型不存在') }
  }

  if (hasDefaultMapping(model, entry.upstreamName)) {
    return { ...entry, status: 'fulfilled', changed: false }
  }

  try {
    await updateModel(props.providerId, model.id, {
      provider_model_mappings: [
        ...currentMappings(model),
        { name: entry.upstreamName, priority: 1 },
      ],
    })
    return { ...entry, status: 'fulfilled', changed: true }
  } catch (reason: unknown) {
    return { ...entry, status: 'rejected', changed: false, reason }
  }
}

async function saveDrafts(entries: DraftEntry[]): Promise<SaveResult[]> {
  const results: SaveResult[] = []
  const concurrency = 4
  for (let index = 0; index < entries.length; index += concurrency) {
    const chunk = entries.slice(index, index + concurrency)
    results.push(...await Promise.all(chunk.map(saveDraft)))
  }
  return results
}

async function saveMappings() {
  if (saving.value || pendingCount.value === 0) return

  saving.value = true
  errorMessage.value = ''
  successMessage.value = ''
  const entries = Object.entries(drafts.value).map(([modelId, upstreamName]) => ({
    modelId,
    upstreamName,
  }))

  try {
    const results = await saveDrafts(entries)
    const failures = results.filter(result => result.status === 'rejected')
    const successes = results.filter(result => result.status === 'fulfilled')
    const changedSuccesses = successes.filter(result => result.changed)

    for (const result of successes) {
      rememberSavedMapping(result.modelId, result.upstreamName)
    }

    const failedDrafts: Record<string, string> = {}
    for (const failure of failures) {
      failedDrafts[failure.modelId] = failure.upstreamName
    }
    drafts.value = failedDrafts

    if (changedSuccesses.length > 0) {
      emit('saved')
    }

    if (failures.length > 0) {
      selectedClientIds.value = failures.map(failure => failure.modelId)
      const firstError = parseApiError(failures[0].reason, '保存失败')
      errorMessage.value = changedSuccesses.length > 0
        ? `已保存 ${changedSuccesses.length} 条，${failures.length} 条失败并保留草稿。${firstError}`
        : `${failures.length} 条映射保存失败，草稿已保留。${firstError}`
      showError(errorMessage.value, '批量保存失败')
      return
    }

    successMessage.value = changedSuccesses.length > 0
      ? `已成功保存 ${changedSuccesses.length} 条映射，可继续添加，完成后点击关闭。`
      : '映射已存在，无需重复保存。'
    showSuccess(successMessage.value)
  } finally {
    saving.value = false
  }
}

watch(() => props.open, async isOpen => {
  if (!isOpen) return
  resetState()
  if (props.hasAutoFetchKey) {
    await fetchUpstreamModels()
  }
}, { immediate: true })
</script>
