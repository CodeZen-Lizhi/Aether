<template>
  <Dialog
    :model-value="open"
    title="批量添加模型映射"
    description="选择客户端模型，查看并修改默认范围映射"
    :icon="Tags"
    size="5xl"
    :no-padding="true"
    @update:model-value="handleDialogUpdate"
  >
    <template #header-actions>
      <Button
        type="button"
        variant="outline"
        size="icon"
        class="h-9 w-9 shrink-0 rounded-lg"
        :disabled="saving || loadingUpstream"
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
      </Button>
    </template>

    <div class="flex min-h-0 flex-col bg-background">
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
                :disabled="saving"
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
                  ? '切换到其他客户端后，当前选择会保留在映射列表中。'
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
                  :disabled="saving || !selectedClientId || loadingUpstream"
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
                    ? '可多选；已配置的默认范围映射会在这里回显，可直接修改。'
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
                    :disabled="saving || !selectedClientId"
                    @keydown.enter.prevent="addCustomModel"
                  />
                </div>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  class="h-9 shrink-0"
                  :disabled="saving || !canAddCustom"
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

            <p class="border-t border-[var(--color-border)] pt-4 text-xs text-muted-foreground">
              默认作用于全部端点和请求
            </p>
          </div>
        </section>

        <section
          class="flex min-h-72 flex-col overflow-hidden rounded-xl border border-[var(--color-border)] bg-card shadow-sm"
          aria-labelledby="batch-mappings-title"
        >
          <div class="flex items-start justify-between gap-3 border-b border-[var(--color-border)] p-4 sm:p-5">
            <div class="min-w-0">
              <h2
                id="batch-mappings-title"
                class="text-sm font-semibold"
              >
                映射列表
              </h2>
              <p class="mt-1 text-xs text-muted-foreground">
                默认范围映射会在保存时更新。
              </p>
            </div>
            <span class="shrink-0 rounded-md bg-muted px-1.5 py-0.5 text-[11px] font-medium tabular-nums text-muted-foreground">
              {{ mappedMappingCount }}
            </span>
          </div>

          <div class="min-h-0 flex-1 space-y-2 overflow-y-auto p-3 sm:p-4">
            <article
              v-for="entry in mappingEntries"
              :key="entry.model.id"
              class="rounded-lg border border-[var(--color-border)] bg-background p-3"
            >
              <div class="flex items-start justify-between gap-3">
                <button
                  type="button"
                  class="min-w-0 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
                  :title="`继续编辑 ${clientModelLabel(entry.model)}`"
                  :disabled="saving"
                  @click="selectClientModel(entry.model.id)"
                >
                  <span class="block truncate text-sm font-medium">{{ clientModelLabel(entry.model) }}</span>
                  <span class="mt-0.5 block truncate font-mono text-xs text-muted-foreground">{{ entry.model.provider_model_name }}</span>
                  <span class="mt-1.5 inline-flex text-xs text-primary">继续编辑</span>
                </button>
                <button
                  type="button"
                  class="shrink-0 rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
                  :aria-label="`清空 ${clientModelLabel(entry.model)} 的映射`"
                  title="清空映射"
                  :disabled="saving"
                  @click="clearMapping(entry.model.id)"
                >
                  <X
                    class="h-4 w-4"
                    aria-hidden="true"
                  />
                </button>
              </div>
              <div
                v-if="entry.upstreamNames.length"
                class="mt-3 flex flex-wrap gap-1.5"
              >
                <span
                  v-for="upstreamName in entry.upstreamNames"
                  :key="upstreamName"
                  class="inline-flex max-w-full items-center gap-1 rounded-md bg-[var(--color-primary-mute)] py-1 pl-2 pr-1 font-mono text-xs text-primary"
                >
                  <span class="truncate">{{ upstreamName }}</span>
                  <button
                    type="button"
                    class="shrink-0 rounded p-0.5 hover:bg-[var(--color-primary-soft)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
                    :aria-label="`移除 ${upstreamName}`"
                    :disabled="saving"
                    @click="removeMappingName(entry.model.id, upstreamName)"
                  >
                    <X
                      class="h-3 w-3"
                      aria-hidden="true"
                    />
                  </button>
                </span>
              </div>
              <p
                v-else
                class="mt-3 text-xs text-muted-foreground"
              >
                默认范围映射已清空
              </p>
            </article>

            <div
              v-if="mappingEntries.length === 0"
              class="flex min-h-48 flex-col items-center justify-center px-4 text-center text-muted-foreground"
            >
              <Link2
                class="mb-2 h-8 w-8 opacity-30"
                aria-hidden="true"
              />
              <p class="text-sm">
                暂无默认范围映射
              </p>
              <p class="mt-1 text-xs">
                选择客户端模型并添加提供商模型，映射会显示在这里。
              </p>
            </div>
          </div>
        </section>
      </div>

      <div
        v-if="feedback"
        class="border-t border-[var(--color-border)] bg-[var(--color-background-mute)] px-4 py-3 sm:px-6"
      >
        <div
          class="min-w-0 rounded-md px-3 py-2 text-xs leading-5"
          :class="feedback.warning ? 'bg-amber-500/10 text-amber-600 dark:text-amber-400' : 'bg-destructive/10 text-destructive'"
          :role="feedback.warning ? 'status' : 'alert'"
        >
          <p class="font-medium">
            {{ feedback.title }}
          </p>
          <div class="mt-2 space-y-3">
            <pre
              v-for="(detail, index) in feedback.details"
              :key="index"
              class="m-0 whitespace-pre-wrap font-sans [overflow-wrap:anywhere]"
            >{{ detail }}</pre>
          </div>
        </div>
      </div>
    </div>

    <template #footer>
      <div class="flex w-full flex-wrap items-center justify-between gap-3">
        <span
          class="text-xs text-muted-foreground"
          aria-live="polite"
        >
          {{ hasUnsavedChanges ? `${changedClientCount} 个客户端模型有待保存更改` : '没有待保存更改' }}
        </span>
        <div class="flex items-center gap-2">
          <Button
            variant="outline"
            :disabled="saving"
            @click="closeDialog"
          >
            {{ hasUnsavedChanges ? '取消' : '关闭' }}
          </Button>
          <Button
            :disabled="saving || !hasUnsavedChanges"
            @click="saveMappings"
          >
            <Loader2
              v-if="saving"
              class="mr-2 h-4 w-4 animate-spin"
              aria-hidden="true"
            />
            {{ saving ? '保存中...' : `保存 ${changedClientCount} 项更改` }}
          </Button>
        </div>
      </div>
    </template>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  Link2,
  Loader2,
  Plus,
  RefreshCw,
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
  reason?: unknown
}

interface MappingEntry {
  model: Model
  upstreamNames: string[]
}

interface MappingFeedback {
  title: string
  details: string[]
  warning?: boolean
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
const drafts = ref<Record<string, string[]>>({})
const initialMappings = ref<Record<string, string[]>>({})
const loadingUpstream = ref(false)
const saving = ref(false)
const feedback = ref<MappingFeedback | null>(null)

const selectedClientModel = computed(() => {
  return props.models.find(model => model.id === selectedClientId.value) ?? null
})

const allUpstreamModels = computed<UpstreamModel[]>(() => {
  const byId = new Map<string, UpstreamModel>()
  for (const model of upstreamModels.value) {
    byId.set(model.id, model)
  }
  for (const names of Object.values(drafts.value)) {
    for (const name of names) {
      if (!byId.has(name)) {
        byId.set(name, { id: name, api_formats: [] })
      }
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

const mappingEntries = computed<MappingEntry[]>(() => {
  return props.models.flatMap(model => {
    const upstreamNames = drafts.value[model.id] ?? []
    return upstreamNames.length > 0 || isMappingChanged(model.id)
      ? [{ model, upstreamNames }]
      : []
  })
})

const changedEntries = computed<DraftEntry[]>(() => {
  return props.models.flatMap(model => {
    const upstreamNames = drafts.value[model.id] ?? []
    return isMappingChanged(model.id) ? [{ modelId: model.id, upstreamNames }] : []
  })
})

const mappedMappingCount = computed(() => {
  return mappingEntries.value.reduce((count, entry) => count + entry.upstreamNames.length, 0)
})
const changedClientCount = computed(() => changedEntries.value.length)
const hasUnsavedChanges = computed(() => changedEntries.value.length > 0)
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

function sameUpstreamNames(left: string[], right: string[]): boolean {
  const normalizedLeft = normalizeUpstreamNames(left).sort()
  const normalizedRight = normalizeUpstreamNames(right).sort()
  return normalizedLeft.length === normalizedRight.length
    && normalizedLeft.every((name, index) => name === normalizedRight[index])
}

function isMappingChanged(modelId: string): boolean {
  return !sameUpstreamNames(
    drafts.value[modelId] ?? [],
    initialMappings.value[modelId] ?? [],
  )
}

function clearSelectionFeedback() {
  feedback.value = null
}

function selectClientModel(modelId: string) {
  selectedClientId.value = modelId
  customModelName.value = ''
  clearSelectionFeedback()
}

function updateDraftNames(modelId: string, names: string[]) {
  if (saving.value) return
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

function defaultMappingNames(model: Model): string[] {
  return normalizeUpstreamNames(
    (model.provider_model_mappings ?? [])
      .filter(isDefaultScopeMapping)
      .map(mapping => mapping.name),
  )
}

function initialMappingsFromModels(): Record<string, string[]> {
  const mappings: Record<string, string[]> = {}
  for (const model of props.models) {
    const names = defaultMappingNames(model)
    if (names.length > 0) {
      mappings[model.id] = names
    }
  }
  return mappings
}

function replaceDefaultMappings(model: Model, names: string[]): ProviderModelMapping[] {
  const existingMappings = model.provider_model_mappings ?? []
  const existingDefaultsByName = new Map<string, ProviderModelMapping>()
  for (const mapping of existingMappings) {
    if (isDefaultScopeMapping(mapping) && !existingDefaultsByName.has(mapping.name)) {
      existingDefaultsByName.set(mapping.name, mapping)
    }
  }

  const nextDefaultMappings = normalizeUpstreamNames(names).map(name => (
    existingDefaultsByName.get(name) ?? { name, priority: 1 }
  ))

  return [
    ...existingMappings.filter(mapping => !isDefaultScopeMapping(mapping)),
    ...nextDefaultMappings,
  ]
}

function clearMapping(modelId: string) {
  updateDraftNames(modelId, [])
}

function removeMappingName(modelId: string, upstreamName: string) {
  updateDraftNames(
    modelId,
    (drafts.value[modelId] ?? []).filter(name => name !== upstreamName),
  )
}

async function fetchUpstreamModels(forceRefresh = false) {
  if (!props.providerId) return
  loadingUpstream.value = true
  feedback.value = null
  try {
    const result = await fetchCachedModels(props.providerId, undefined, forceRefresh)
    upstreamModels.value = result.models
    if (result.warning) {
      feedback.value = {
        title: '获取提供商模型提示',
        details: [result.warning],
        warning: true,
      }
      showWarning(result.warning, '获取提供商模型提示')
    }
    if (result.error) {
      feedback.value = { title: '获取提供商模型失败', details: [result.error] }
    }
  } catch (error: unknown) {
    feedback.value = {
      title: '获取提供商模型失败',
      details: [parseApiError(error, '获取提供商模型失败')],
    }
  } finally {
    loadingUpstream.value = false
  }
}

function resetState() {
  const initial = initialMappingsFromModels()
  selectedClientId.value = ''
  customModelName.value = ''
  upstreamModels.value = []
  initialMappings.value = initial
  drafts.value = Object.fromEntries(
    Object.entries(initial).map(([modelId, names]) => [modelId, [...names]]),
  )
  feedback.value = null
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
  if (hasUnsavedChanges.value) {
    const confirmed = await confirm({
      title: '放弃更改',
      message: '有未保存的映射更改，确定要关闭吗？',
      confirmText: '放弃并关闭',
      cancelText: '继续编辑',
      variant: 'warning',
    })
    if (!confirmed) return
  }
  emit('update:open', false)
}

async function saveDraft(entry: DraftEntry): Promise<SaveResult> {
  const model = props.models.find(item => item.id === entry.modelId)
  if (!model) {
    return {
      ...entry,
      status: 'rejected',
      reason: new Error('客户端模型不存在'),
    }
  }

  try {
    const nextMappings = replaceDefaultMappings(model, entry.upstreamNames)
    await updateModel(props.providerId, model.id, {
      provider_model_mappings: nextMappings.length > 0 ? nextMappings : null,
    })
    return { ...entry, status: 'fulfilled' }
  } catch (reason: unknown) {
    return { ...entry, status: 'rejected', reason }
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
  if (saving.value || !hasUnsavedChanges.value) return

  saving.value = true
  feedback.value = null
  const entries = changedEntries.value

  try {
    const results = await saveDrafts(entries)
    const failures = results.filter(result => result.status === 'rejected')
    const successes = results.filter(result => result.status === 'fulfilled')

    for (const result of successes) {
      const next = { ...initialMappings.value }
      if (result.upstreamNames.length > 0) {
        next[result.modelId] = normalizeUpstreamNames(result.upstreamNames)
      } else {
        delete next[result.modelId]
      }
      initialMappings.value = next
    }

    if (successes.length > 0) {
      emit('saved')
    }

    if (failures.length > 0) {
      selectedClientId.value = failures[0].modelId
      const title = successes.length > 0
        ? `已保存 ${successes.length} 个客户端模型，${failures.length} 个客户端模型的更改仍待保存。`
        : `${failures.length} 个客户端模型的更改保存失败，仍待保存。`
      feedback.value = {
        title,
        details: failures.map(failure => {
          const model = props.models.find(item => item.id === failure.modelId)
          return `${model ? clientModelLabel(model) : failure.modelId}: ${parseApiError(failure.reason, '保存失败')}`
        }),
      }
      showError(title, '批量保存失败')
      return
    }

    showSuccess(`已成功保存 ${successes.length} 个客户端模型的映射。`)
    emit('update:open', false)
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
