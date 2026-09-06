<template>
  <Dialog
    :model-value="open"
    title="批量添加模型映射"
    description="逐个选择客户端模型，并为每个模型选择多个提供商模型"
    :icon="Tags"
    size="6xl"
    :no-padding="true"
    @update:model-value="handleDialogUpdate"
  >
    <div class="flex min-h-0 flex-col bg-background">
      <div class="border-b border-[var(--color-border)] bg-[var(--color-background-mute)] px-4 py-4 sm:px-6">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div class="flex min-w-0 items-center gap-2 text-xs text-muted-foreground">
            <span class="flex h-6 w-6 items-center justify-center rounded-full bg-primary text-xs font-semibold text-primary-foreground">1</span>
            <span :class="selectedClientId ? 'text-foreground' : 'font-medium text-primary'">选择客户端</span>
            <ArrowRight
              class="h-3.5 w-3.5 shrink-0 text-muted-foreground"
              aria-hidden="true"
            />
            <span
              class="flex h-6 w-6 items-center justify-center rounded-full text-xs font-semibold"
              :class="selectedUpstreamNames.length > 0 ? 'bg-primary text-primary-foreground' : 'border border-border bg-background text-muted-foreground'"
            >2</span>
            <span :class="selectedUpstreamNames.length > 0 ? 'text-foreground' : 'text-muted-foreground'">选择提供商模型</span>
            <ArrowRight
              class="h-3.5 w-3.5 shrink-0 text-muted-foreground"
              aria-hidden="true"
            />
            <span
              class="flex h-6 w-6 items-center justify-center rounded-full text-xs font-semibold"
              :class="pendingMappingCount > 0 ? 'bg-primary text-primary-foreground' : 'border border-border bg-background text-muted-foreground'"
            >3</span>
            <span :class="pendingMappingCount > 0 ? 'text-foreground' : 'text-muted-foreground'">统一保存</span>
          </div>

          <Button
            variant="outline"
            size="sm"
            class="h-9 shrink-0"
            :disabled="models.length === 0 || upstreamModels.length === 0"
            @click="autoMatch"
          >
            <Sparkles
              class="mr-1.5 h-3.5 w-3.5"
              aria-hidden="true"
            />
            按名称自动匹配
          </Button>
        </div>

        <div
          class="mt-3 flex flex-wrap items-center gap-2"
          aria-live="polite"
        >
          <div class="inline-flex items-center gap-2 rounded-lg border border-[var(--color-primary-soft)] bg-[var(--color-primary-mute)] px-3 py-1.5 text-sm text-primary">
            <CheckCheck
              class="h-4 w-4"
              aria-hidden="true"
            />
            <span class="font-medium">{{ `已暂存客户端 ${pendingClientCount}` }}</span>
          </div>
          <div class="inline-flex items-center gap-2 rounded-lg border border-[var(--color-border)] bg-background px-3 py-1.5 text-sm text-muted-foreground">
            <Link2
              class="h-4 w-4"
              aria-hidden="true"
            />
            <span>待保存 {{ pendingMappingCount }}</span>
          </div>
          <span
            v-if="selectedClientModel"
            class="min-w-0 max-w-full truncate rounded-lg border border-[var(--color-primary-soft)] bg-[var(--color-active)] px-3 py-1.5 font-mono text-xs text-primary"
            :title="clientModelLabel(selectedClientModel)"
          >
            当前客户端：{{ clientModelLabel(selectedClientModel) }}
          </span>
        </div>
      </div>

      <div class="grid gap-4 p-4 sm:p-6 lg:grid-cols-[minmax(0,1.05fr)_minmax(20rem,0.95fr)]">
        <section
          class="rounded-xl border border-[var(--color-border)] bg-card shadow-sm"
          aria-labelledby="batch-config-title"
        >
          <div class="border-b border-[var(--color-border)] p-4 sm:p-5">
            <h2
              id="batch-config-title"
              class="text-sm font-semibold"
            >
              配置映射
            </h2>
            <p class="mt-1 text-xs text-muted-foreground">
              先选择一个客户端模型，再从下拉列表中多选提供商模型。
            </p>
          </div>

          <div class="space-y-5 p-4 sm:p-5">
            <div class="space-y-2">
              <Label
                for="batch-client-model"
                class="text-sm font-medium"
              >客户端模型</Label>
              <Select
                :model-value="selectedClientId"
                @update:model-value="selectClientModel"
              >
                <SelectTrigger
                  id="batch-client-model"
                  class="h-10 rounded-lg"
                >
                  <SelectValue placeholder="请选择客户端模型" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="model in models"
                    :key="model.id"
                    :value="model.id"
                    :text-value="`${clientModelLabel(model)} ${model.provider_model_name}`"
                  >
                    <div class="flex min-w-0 items-center gap-2">
                      <span class="min-w-0 truncate font-medium">{{ clientModelLabel(model) }}</span>
                      <span class="shrink-0 font-mono text-xs text-muted-foreground">{{ model.provider_model_name }}</span>
                    </div>
                  </SelectItem>
                </SelectContent>
              </Select>
              <p class="text-xs text-muted-foreground">
                {{ selectedClientModel
                  ? '切换到其他客户端后，当前选择会保留为草稿。'
                  : '请先选择客户端模型' }}
              </p>
            </div>

            <div class="space-y-2">
              <div class="flex items-center justify-between gap-3">
                <Label
                  id="batch-upstream-model-label"
                  class="text-sm font-medium"
                >提供商模型</Label>
                <span
                  class="shrink-0 text-xs"
                  :class="selectedUpstreamNames.length ? 'text-primary' : 'text-muted-foreground'"
                >
                  {{ selectedUpstreamNames.length ? `已选 ${selectedUpstreamNames.length} 个` : '未选择' }}
                </span>
              </div>
              <div
                role="group"
                aria-labelledby="batch-upstream-model-label"
              >
                <MultiSelect
                  v-model="selectedUpstreamNames"
                  :options="upstreamOptions"
                  :disabled="!selectedClientId || loadingUpstream"
                  :placeholder="selectedClientId ? '选择一个或多个提供商模型' : '请先选择客户端模型'"
                  empty-text="暂无提供商模型"
                  no-results-text="无匹配提供商模型"
                  search-placeholder="搜索提供商模型..."
                  trigger-class="h-10 rounded-lg"
                  :search-threshold="0"
                  teleport
                />
              </div>
              <p class="text-xs text-muted-foreground">
                {{ loadingUpstream
                  ? '正在加载提供商模型'
                  : selectedClientId
                    ? '可多选；每个客户端模型各自保留一组待保存映射。'
                    : '选择客户端模型后即可多选。' }}
              </p>
            </div>

            <div class="rounded-lg border border-dashed border-[var(--color-border)] bg-[var(--color-background-mute)] p-3">
              <div class="flex flex-col gap-2 sm:flex-row sm:items-end">
                <div class="min-w-0 flex-1 space-y-1.5">
                  <Label
                    for="batch-custom-upstream-model"
                    class="text-xs"
                  >添加自定义提供商模型</Label>
                  <Input
                    id="batch-custom-upstream-model"
                    v-model="customModelName"
                    class="h-9 rounded-lg"
                    placeholder="输入自定义提供商模型名称..."
                    :disabled="!selectedClientId"
                    @keydown.enter.prevent="addCustomModel"
                  />
                </div>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  class="h-9 shrink-0"
                  :disabled="!canAddCustom"
                  @click="addCustomModel"
                >
                  <Plus
                    class="mr-1.5 h-3.5 w-3.5"
                    aria-hidden="true"
                  />
                  添加并选中
                </Button>
              </div>
            </div>

            <div class="flex items-center justify-between gap-3 border-t border-[var(--color-border)] pt-4">
              <span class="text-xs text-muted-foreground">默认作用于全部端点和请求</span>
              <Button
                type="button"
                variant="outline"
                size="sm"
                class="h-9 shrink-0"
                :disabled="loadingUpstream"
                title="刷新提供商模型"
                @click="fetchUpstreamModels(true)"
              >
                <Loader2
                  v-if="loadingUpstream"
                  class="mr-1.5 h-3.5 w-3.5 animate-spin"
                  aria-hidden="true"
                />
                <RefreshCw
                  v-else
                  class="mr-1.5 h-3.5 w-3.5"
                  aria-hidden="true"
                />
                刷新提供商模型
              </Button>
            </div>
          </div>
        </section>

        <section
          class="flex min-h-72 flex-col overflow-hidden rounded-xl border border-[var(--color-border)] bg-card shadow-sm"
          aria-labelledby="batch-drafts-title"
        >
          <div class="flex items-start justify-between gap-3 border-b border-[var(--color-border)] p-4 sm:p-5">
            <div class="min-w-0">
              <h2
                id="batch-drafts-title"
                class="text-sm font-semibold"
              >
                映射草稿
              </h2>
              <p class="mt-1 text-xs text-muted-foreground">
                已暂存的映射将在保存时统一提交。
              </p>
            </div>
            <span class="shrink-0 rounded-md bg-muted px-1.5 py-0.5 text-[11px] font-medium tabular-nums text-muted-foreground">
              {{ pendingMappingCount }}
            </span>
          </div>

          <div class="min-h-0 flex-1 space-y-2 overflow-y-auto p-3 sm:p-4">
            <article
              v-for="draft in pendingDrafts"
              :key="draft.model.id"
              class="rounded-lg border border-[var(--color-border)] bg-background p-3"
            >
              <div class="flex items-start justify-between gap-3">
                <button
                  type="button"
                  class="min-w-0 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
                  :title="`继续编辑 ${clientModelLabel(draft.model)}`"
                  @click="selectClientModel(draft.model.id)"
                >
                  <span class="block truncate text-sm font-medium">{{ clientModelLabel(draft.model) }}</span>
                  <span class="mt-0.5 block truncate font-mono text-xs text-muted-foreground">{{ draft.model.provider_model_name }}</span>
                  <span class="mt-1.5 inline-flex text-xs text-primary">继续编辑</span>
                </button>
                <button
                  type="button"
                  class="shrink-0 rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
                  :aria-label="`清空 ${clientModelLabel(draft.model)} 的草稿`"
                  title="清空草稿"
                  @click="clearDraft(draft.model.id)"
                >
                  <X
                    class="h-4 w-4"
                    aria-hidden="true"
                  />
                </button>
              </div>
              <div class="mt-3 flex flex-wrap gap-1.5">
                <span
                  v-for="upstreamName in draft.upstreamNames"
                  :key="upstreamName"
                  class="inline-flex max-w-full items-center gap-1 rounded-md bg-[var(--color-primary-mute)] py-1 pl-2 pr-1 font-mono text-xs text-primary"
                >
                  <span class="truncate">{{ upstreamName }}</span>
                  <button
                    type="button"
                    class="shrink-0 rounded p-0.5 hover:bg-[var(--color-primary-soft)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
                    :aria-label="`移除 ${upstreamName}`"
                    @click="removeDraftUpstreamName(draft.model.id, upstreamName)"
                  >
                    <X
                      class="h-3 w-3"
                      aria-hidden="true"
                    />
                  </button>
                </span>
              </div>
            </article>

            <div
              v-if="pendingDrafts.length === 0"
              class="flex min-h-48 flex-col items-center justify-center px-4 text-center text-muted-foreground"
            >
              <Link2
                class="mb-2 h-8 w-8 opacity-30"
                aria-hidden="true"
              />
              <p class="text-sm">
                暂无待保存映射
              </p>
              <p class="mt-1 text-xs">
                先选择客户端模型和提供商模型，草稿会显示在这里。
              </p>
            </div>
          </div>
        </section>
      </div>

      <div class="border-t border-[var(--color-border)] bg-[var(--color-background-mute)] px-4 py-3 sm:px-6">
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
          {{ pendingMappingCount ? `${pendingClientCount} 个客户端模型，${pendingMappingCount} 条映射待保存` : '没有待保存草稿' }}
        </span>
        <div class="flex items-center gap-2">
          <Button
            variant="outline"
            :disabled="saving"
            @click="closeDialog"
          >
            {{ pendingMappingCount ? '取消' : '关闭' }}
          </Button>
          <Button
            :disabled="saving || pendingMappingCount === 0"
            @click="saveMappings"
          >
            <Loader2
              v-if="saving"
              class="mr-2 h-4 w-4 animate-spin"
              aria-hidden="true"
            />
            {{ saving ? '保存中...' : `保存 ${pendingMappingCount} 条映射` }}
          </Button>
        </div>
      </div>
    </template>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  ArrowRight,
  CheckCheck,
  Link2,
  Loader2,
  Plus,
  RefreshCw,
  Sparkles,
  Tags,
  X,
} from 'lucide-vue-next'
import {
  Button,
  Dialog,
  Input,
  Label,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui'
import MultiSelect, { type MultiSelectOption } from '@/components/common/MultiSelect.vue'
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
  upstreamNames: string[]
}

interface SaveResult extends DraftEntry {
  status: 'fulfilled' | 'rejected'
  addedNames: string[]
  reason?: unknown
}

interface PendingDraft {
  model: Model
  upstreamNames: string[]
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

const selectedClientId = ref('')
const customModelName = ref('')
const upstreamModels = ref<UpstreamModel[]>([])
const customModels = ref<string[]>([])
const drafts = ref<Record<string, string[]>>({})
const locallySavedNames = ref<Record<string, string[]>>({})
const loadingUpstream = ref(false)
const saving = ref(false)
const errorMessage = ref('')
const successMessage = ref('')

const selectedClientModel = computed(() => {
  return props.models.find(model => model.id === selectedClientId.value) ?? null
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

const upstreamOptions = computed<MultiSelectOption[]>(() => {
  return allUpstreamModels.value.map(model => ({
    value: model.id,
    label: model.id,
  }))
})

const selectedUpstreamNames = computed<string[]>({
  get() {
    if (!selectedClientId.value) return []
    return drafts.value[selectedClientId.value] ?? []
  },
  set(names) {
    if (!selectedClientId.value) return
    updateDraftNames(selectedClientId.value, names)
  },
})

const pendingDrafts = computed<PendingDraft[]>(() => {
  return props.models.flatMap(model => {
    const upstreamNames = drafts.value[model.id] ?? []
    return upstreamNames.length > 0 ? [{ model, upstreamNames }] : []
  })
})

const pendingClientCount = computed(() => pendingDrafts.value.length)
const pendingMappingCount = computed(() => {
  return pendingDrafts.value.reduce((count, draft) => count + draft.upstreamNames.length, 0)
})
const canAddCustom = computed(() => {
  const name = customModelName.value.trim()
  return Boolean(
    selectedClientId.value
      && name
      && !allUpstreamModels.value.some(model => model.id === name),
  )
})

function clientModelLabel(model: Model): string {
  return model.global_model_display_name || model.global_model_name || model.provider_model_name
}

function normalizeUpstreamNames(names: string[]): string[] {
  return [...new Set(names.map(name => name.trim()).filter(Boolean))]
}

function clearSelectionFeedback() {
  errorMessage.value = ''
  successMessage.value = ''
}

function selectClientModel(modelId: string) {
  selectedClientId.value = modelId
  customModelName.value = ''
  clearSelectionFeedback()
}

function updateDraftNames(modelId: string, names: string[]) {
  const normalizedNames = normalizeUpstreamNames(names)
  const next = { ...drafts.value }
  if (normalizedNames.length > 0) {
    next[modelId] = normalizedNames
  } else {
    delete next[modelId]
  }
  drafts.value = next
  clearSelectionFeedback()
}

function addCustomModel() {
  const name = customModelName.value.trim()
  if (!name || !selectedClientId.value) return

  customModels.value = [...new Set([...customModels.value, name])]
  selectedUpstreamNames.value = [...selectedUpstreamNames.value, name]
  customModelName.value = ''
}

function hasScopeValues(values?: string[]): boolean {
  return Array.isArray(values) && values.length > 0
}

function isDefaultScopeMapping(mapping: ProviderModelMapping): boolean {
  return !hasScopeValues(mapping.api_formats)
    && !hasScopeValues(mapping.endpoint_ids)
    && !hasScopeValues(mapping.operations)
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

function clearDraft(modelId: string) {
  updateDraftNames(modelId, [])
}

function removeDraftUpstreamName(modelId: string, upstreamName: string) {
  updateDraftNames(
    modelId,
    (drafts.value[modelId] ?? []).filter(name => name !== upstreamName),
  )
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

  for (const model of props.models) {
    const upstreamName = exactUpstreamMatch(model)
    if (!upstreamName || hasDefaultMapping(model, upstreamName)) continue

    const names = normalizeUpstreamNames(next[model.id] ?? [])
    if (names.includes(upstreamName)) continue
    next[model.id] = [...names, upstreamName]
    matchedCount += 1
  }

  drafts.value = next
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
  selectedClientId.value = ''
  customModelName.value = ''
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
  if (pendingMappingCount.value > 0) {
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

function rememberSavedMappings(modelId: string, upstreamNames: string[]) {
  locallySavedNames.value = {
    ...locallySavedNames.value,
    [modelId]: normalizeUpstreamNames([
      ...(locallySavedNames.value[modelId] ?? []),
      ...upstreamNames,
    ]),
  }
}

async function saveDraft(entry: DraftEntry): Promise<SaveResult> {
  const model = props.models.find(item => item.id === entry.modelId)
  if (!model) {
    return {
      ...entry,
      status: 'rejected',
      addedNames: [],
      reason: new Error('客户端模型不存在'),
    }
  }

  const namesToAdd = normalizeUpstreamNames(entry.upstreamNames)
    .filter(name => !hasDefaultMapping(model, name))
  if (namesToAdd.length === 0) {
    return { ...entry, status: 'fulfilled', addedNames: [] }
  }

  try {
    await updateModel(props.providerId, model.id, {
      provider_model_mappings: [
        ...currentMappings(model),
        ...namesToAdd.map(name => ({ name, priority: 1 })),
      ],
    })
    return { ...entry, status: 'fulfilled', addedNames: namesToAdd }
  } catch (reason: unknown) {
    return { ...entry, status: 'rejected', addedNames: [], reason }
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
  if (saving.value || pendingMappingCount.value === 0) return

  saving.value = true
  errorMessage.value = ''
  successMessage.value = ''
  const entries = Object.entries(drafts.value)
    .map(([modelId, upstreamNames]) => ({ modelId, upstreamNames }))
    .filter(entry => entry.upstreamNames.length > 0)

  try {
    const results = await saveDrafts(entries)
    const failures = results.filter(result => result.status === 'rejected')
    const successes = results.filter(result => result.status === 'fulfilled')
    const addedMappingCount = successes.reduce(
      (count, result) => count + result.addedNames.length,
      0,
    )

    for (const result of successes) {
      if (result.addedNames.length > 0) {
        rememberSavedMappings(result.modelId, result.addedNames)
      }
    }

    const failedDrafts: Record<string, string[]> = {}
    for (const failure of failures) {
      failedDrafts[failure.modelId] = failure.upstreamNames
    }
    drafts.value = failedDrafts

    if (addedMappingCount > 0) {
      emit('saved')
    }

    if (failures.length > 0) {
      selectedClientId.value = failures[0].modelId
      const failedMappingCount = failures.reduce(
        (count, failure) => count + failure.upstreamNames.length,
        0,
      )
      const firstError = parseApiError(failures[0].reason, '保存失败')
      errorMessage.value = addedMappingCount > 0
        ? `已保存 ${addedMappingCount} 条，${failedMappingCount} 条失败并保留草稿。${firstError}`
        : `${failedMappingCount} 条映射保存失败，草稿已保留。${firstError}`
      showError(errorMessage.value, '批量保存失败')
      return
    }

    selectedClientId.value = ''
    customModelName.value = ''
    successMessage.value = addedMappingCount > 0
      ? `已成功保存 ${addedMappingCount} 条映射，可继续添加，完成后点击关闭。`
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
