<template>
  <!-- 全供应商客户端模型批量映射工作台，桌面为表格、窄内容宽度为卡片。 -->
  <Dialog
    :model-value="open"
    title="批量添加模型映射"
    description="为每个客户端模型维护提供商模型集合，更改统一保存"
    :icon="ListPlus"
    size="4xl"
    :no-padding="true"
    opaque-backdrop
    @update:model-value="handleDialogUpdate"
  >
    <template #header-actions>
      <div
        class="flex shrink-0 items-center gap-1.5"
        data-batch-mapping-header-actions
      >
        <span
          class="hidden whitespace-nowrap rounded-md bg-muted px-2 py-1 text-[11px] font-medium tabular-nums text-muted-foreground sm:inline-block"
          data-batch-mapping-change-badge
        >
          {{ hasUnsavedChanges ? `${changedClientCount} 项待保存` : '没有待保存更改' }}
        </span>
        <Button
          type="button"
          variant="outline"
          size="icon"
          class="h-9 w-9 shrink-0 rounded-lg"
          :disabled="saving || loadingUpstream || !hasUnsavedChanges"
          title="恢复为已保存状态"
          aria-label="恢复为已保存状态"
          @click="resetDraft"
        >
          <RotateCcw class="h-4 w-4" />
        </Button>
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
      </div>
    </template>

    <div class="bg-background">
      <div class="border-b border-border bg-muted/10 px-4 py-3 sm:px-6">
        <div class="flex flex-wrap items-center gap-2">
          <label class="relative min-w-[12rem] flex-1 sm:max-w-[20rem]">
            <span class="sr-only">搜索客户端或提供商模型</span>
            <Search class="pointer-events-none absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              v-model="searchQuery"
              class="h-9 rounded-lg border-border/60 bg-background pl-9"
              placeholder="搜索客户端或提供商模型…"
              :disabled="saving"
            />
          </label>

          <div
            class="flex flex-wrap items-center gap-1 rounded-lg border border-border bg-background p-1"
            role="group"
            aria-label="映射状态筛选"
          >
            <button
              v-for="option in statusFilterOptions"
              :key="option.value"
              type="button"
              class="rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
              :class="statusFilter === option.value
                ? 'bg-primary text-primary-foreground'
                : 'text-muted-foreground hover:bg-muted hover:text-foreground'"
              :aria-pressed="statusFilter === option.value"
              :disabled="saving"
              @click="statusFilter = option.value"
            >
              {{ option.label }} {{ option.count }}
            </button>
          </div>

          <label class="flex shrink-0 cursor-pointer items-center gap-1.5 text-[11px] text-muted-foreground">
            <input
              v-model="showHiddenModels"
              type="checkbox"
              class="h-3.5 w-3.5 rounded border-border accent-primary"
              :disabled="saving"
            >
            显示已隐藏（{{ hiddenModels.length }}）
          </label>

          <label
            v-if="changedClientCount > 0"
            class="flex shrink-0 cursor-pointer items-center gap-1.5 text-[11px] text-muted-foreground"
          >
            <input
              v-model="showOnlyChanged"
              type="checkbox"
              class="h-3.5 w-3.5 rounded border-border accent-primary"
              :disabled="saving"
            >
            只看已修改
          </label>
        </div>

        <p class="mt-2 text-[11px] text-muted-foreground">
          {{ loadingUpstream
            ? '正在加载提供商模型…'
            : `共 ${models.length} 个客户端模型 · ${mappedClientCount} 个已映射 · ${unmappedClientCount} 个未映射` }}
        </p>
      </div>

      <div class="px-4 py-3 sm:px-6">
        <div
          v-if="loadingUpstream && allUpstreamModels.length === 0"
          class="flex min-h-48 items-center justify-center gap-2 rounded-xl border border-border bg-card text-sm text-muted-foreground"
        >
          <Loader2 class="h-4 w-4 animate-spin" />
          正在加载提供商模型
        </div>

        <div
          v-else
          class="responsive-list"
        >
          <div
            class="responsive-list-table"
            data-layout-surface="table"
          >
            <div class="overflow-hidden rounded-xl border border-border bg-card">
              <table class="w-full table-fixed text-sm">
                <colgroup>
                  <col class="w-[20%]">
                  <col class="w-[48%]">
                  <col class="w-[14%]">
                  <col class="w-[18%]">
                </colgroup>
                <thead class="border-b border-border bg-muted/30">
                  <tr>
                    <th
                      scope="col"
                      class="px-3 py-2.5 text-left text-xs font-medium text-muted-foreground"
                    >
                      客户端模型
                    </th>
                    <th
                      scope="col"
                      class="px-3 py-2.5 text-left text-xs font-medium text-muted-foreground"
                    >
                      提供商模型
                    </th>
                    <th
                      scope="col"
                      class="px-3 py-2.5 text-left text-xs font-medium text-muted-foreground"
                    >
                      状态
                    </th>
                    <th
                      scope="col"
                      class="px-3 py-2.5 text-right text-xs font-medium text-muted-foreground"
                    >
                      操作
                    </th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="entry in visibleEntries"
                    :key="entry.model.id"
                    class="border-b border-border/45 transition-colors last:border-b-0"
                    :class="entry.isHidden ? 'bg-muted/20 opacity-70' : 'hover:bg-muted/20'"
                  >
                    <td class="px-3 py-3 align-top">
                      <ModelIdentity
                        :model="entry.model"
                        :hidden="entry.isHidden"
                        @copy="copyModelId"
                      />
                    </td>
                    <td class="px-3 py-3 align-top">
                      <MappingEditor
                        surface="table"
                        :model-id="entry.model.id"
                        :model-name="clientModelLabel(entry.model)"
                        :names="entry.upstreamNames"
                        :options="allUpstreamModels"
                        :editing="editingModelId === entry.model.id"
                        :search="editingModelId === entry.model.id ? addSearch : ''"
                        :disabled="saving || entry.isHidden"
                        @start="beginAdd(entry.model.id)"
                        @cancel="cancelAdd"
                        @update-search="addSearch = $event"
                        @add="addMappingName"
                        @remove="removeMappingName(entry.model.id, $event)"
                      />
                    </td>
                    <td class="px-3 py-3 align-top">
                      <MappingStatus
                        :mapped-count="entry.upstreamNames.length"
                        :changed="entry.isChanged"
                        :hidden="entry.isHidden"
                      />
                    </td>
                    <td class="px-3 py-3 text-right align-top">
                      <div class="flex items-center justify-end gap-1">
                        <button
                          v-if="entry.upstreamNames.length > 0"
                          type="button"
                          class="inline-flex shrink-0 items-center gap-1 whitespace-nowrap rounded-md px-2 py-1.5 text-[11px] font-medium text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:pointer-events-none disabled:opacity-50"
                          :disabled="saving || entry.isHidden"
                          :aria-label="`清空 ${clientModelLabel(entry.model)} 的映射`"
                          @click="clearMapping(entry.model.id)"
                        >
                          <X class="h-3 w-3" />
                          清空
                        </button>
                        <button
                          type="button"
                          class="inline-flex shrink-0 items-center gap-1 whitespace-nowrap rounded-md px-2 py-1.5 text-[11px] font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:pointer-events-none disabled:opacity-50"
                          :disabled="saving"
                          :aria-label="`${entry.isHidden ? '显示' : '隐藏'} ${clientModelLabel(entry.model)}`"
                          @click="toggleHidden(entry.model.id)"
                        >
                          {{ entry.isHidden ? '显示' : '隐藏' }}
                        </button>
                      </div>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>

          <div
            class="responsive-list-cards divide-y divide-border/50 overflow-hidden rounded-xl border border-border bg-card"
            data-layout-surface="cards"
          >
            <div
              v-for="entry in visibleEntries"
              :key="entry.model.id"
              class="space-y-3 p-3"
              :class="entry.isHidden ? 'bg-muted/20 opacity-70' : ''"
            >
              <div class="flex items-start justify-between gap-3">
                <ModelIdentity
                  :model="entry.model"
                  :hidden="entry.isHidden"
                  @copy="copyModelId"
                />
                <MappingStatus
                  :mapped-count="entry.upstreamNames.length"
                  :changed="entry.isChanged"
                  :hidden="entry.isHidden"
                />
              </div>

              <div class="min-w-0">
                <p class="mb-1.5 text-[11px] font-medium text-muted-foreground">
                  提供商模型
                </p>
                <MappingEditor
                  surface="cards"
                  :model-id="entry.model.id"
                  :model-name="clientModelLabel(entry.model)"
                  :names="entry.upstreamNames"
                  :options="allUpstreamModels"
                  :editing="editingModelId === entry.model.id"
                  :search="editingModelId === entry.model.id ? addSearch : ''"
                  :disabled="saving || entry.isHidden"
                  @start="beginAdd(entry.model.id)"
                  @cancel="cancelAdd"
                  @update-search="addSearch = $event"
                  @add="addMappingName"
                  @remove="removeMappingName(entry.model.id, $event)"
                />
              </div>

              <div class="flex items-center justify-end gap-1 border-t border-border/50 pt-2">
                <button
                  v-if="entry.upstreamNames.length > 0"
                  type="button"
                  class="inline-flex items-center gap-1 rounded-md px-2 py-1.5 text-[11px] font-medium text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:pointer-events-none disabled:opacity-50"
                  :disabled="saving || entry.isHidden"
                  :aria-label="`清空 ${clientModelLabel(entry.model)} 的映射`"
                  @click="clearMapping(entry.model.id)"
                >
                  <X class="h-3 w-3" />
                  清空
                </button>
                <button
                  type="button"
                  class="inline-flex items-center gap-1 rounded-md px-2 py-1.5 text-[11px] font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:pointer-events-none disabled:opacity-50"
                  :disabled="saving"
                  :aria-label="`${entry.isHidden ? '显示' : '隐藏'} ${clientModelLabel(entry.model)}`"
                  @click="toggleHidden(entry.model.id)"
                >
                  {{ entry.isHidden ? '显示' : '隐藏' }}
                </button>
              </div>
            </div>
          </div>

          <div
            v-if="visibleEntries.length === 0"
            class="flex min-h-48 flex-col items-center justify-center rounded-xl border border-border bg-card px-4 text-center text-muted-foreground"
          >
            <Tag class="mb-2 h-8 w-8 opacity-30" />
            <p class="text-sm">
              {{ emptyTitle }}
            </p>
            <p class="mt-1 text-xs">
              {{ emptyDescription }}
            </p>
          </div>
        </div>
      </div>

      <div
        class="pb-1"
        v-if="feedback"
      >
        <div
          class="border-t border-border bg-[var(--color-background-mute)] px-4 pb-3 sm:px-6"
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
    </div>

    <template #footer>
      <div
        class="flex w-full flex-wrap items-center justify-between gap-3"
        data-batch-mapping-footer
      >
        <div class="min-w-0 space-y-0.5">
          <p
            class="text-xs text-muted-foreground sm:hidden"
            aria-live="polite"
          >
            {{ hasUnsavedChanges ? `${changedClientCount} 项待保存` : '没有待保存更改' }}
          </p>
          <p class="text-[11px] text-muted-foreground">
            {{ mappedClientCount }} 个已映射 · {{ unmappedClientCount }} 个未映射 · {{ mappedNameCount }} 个提供商模型映射
          </p>
        </div>
        <div class="flex shrink-0 items-center gap-2">
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
            />
            {{ saving ? '保存中...' : hasUnsavedChanges ? `保存 ${changedClientCount} 项更改` : '保存' }}
          </Button>
        </div>
      </div>
    </template>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import {
  ListPlus,
  Loader2,
  RefreshCw,
  RotateCcw,
  Search,
  Tag,
  X,
} from 'lucide-vue-next'
import { Button, Dialog, Input } from '@/components/ui'
import { updateModel } from '@/api/endpoints/models'
import type { Model, ProviderModelMapping, UpstreamModel } from '@/api/endpoints'
import { useClipboard } from '@/composables/useClipboard'
import { useConfirm } from '@/composables/useConfirm'
import { useToast } from '@/composables/useToast'
import { parseApiError } from '@/utils/errorParser'
import { useUpstreamModelsCache } from '../composables/useUpstreamModelsCache'
import ModelIdentity from './ModelMappingBatchIdentity.vue'
import MappingEditor from './ModelMappingBatchEditor.vue'
import MappingStatus from './ModelMappingBatchStatus.vue'

interface Props {
  /** 控制批量映射弹窗是否打开。 */
  open: boolean
  /** 当前供应商标识，用于读取上游模型和更新映射。 */
  providerId: string
  /** 当前供应商下的客户端模型及其已保存映射。 */
  models: Model[]
  /** 是否存在可用于自动拉取上游模型的活跃密钥。 */
  hasAutoFetchKey?: boolean
}

interface DraftEntry {
  /** 需要保存变更的客户端模型标识。 */
  modelId: string
  /** 提交给该客户端模型的提供商模型名称。 */
  upstreamNames: string[]
}

interface SaveResult extends DraftEntry {
  /** 当前模型映射是否已成功写入。 */
  status: 'fulfilled' | 'rejected'
  /** 保存失败时的原始错误，用于展示完整详情。 */
  reason?: unknown
}

interface MappingEntry {
  /** 客户端模型实体。 */
  model: Model
  /** 当前草稿中的提供商模型名称。 */
  upstreamNames: string[]
  /** 当前草稿是否不同于打开弹窗时的基线。 */
  isChanged: boolean
  /** 是否仅在当前工作台中隐藏，不改变映射。 */
  isHidden: boolean
}

interface MappingFeedback {
  /** 面向用户的错误或提示摘要。 */
  title: string
  /** 保留换行和原始内容的详情列表。 */
  details: string[]
  /** 为真时以提示展示，否则以错误展示。 */
  warning?: boolean
}

/** 工作台可见行筛选；不改变草稿、隐藏集合或保存载荷。 */
type MappingStatusFilter = 'all' | 'mapped' | 'unmapped' | 'changed'

const props = defineProps<Props>()
const emit = defineEmits<{
  'update:open': [value: boolean]
  saved: []
}>()

const { fetchModels: fetchCachedModels } = useUpstreamModelsCache()
const { confirm } = useConfirm()
const { copyToClipboard } = useClipboard()
const { error: showError, success: showSuccess, warning: showWarning } = useToast()

const searchQuery = ref('')
const statusFilter = ref<MappingStatusFilter>('all')
const showOnlyChanged = ref(false)
const showHiddenModels = ref(false)
const editingModelId = ref<string | null>(null)
const addSearch = ref('')
const upstreamModels = ref<UpstreamModel[]>([])
const drafts = ref<Record<string, string[]>>({})
const initialMappings = ref<Record<string, string[]>>({})
const hiddenModelIds = ref<Set<string>>(new Set())
const loadingUpstream = ref(false)
const saving = ref(false)
const feedback = ref<MappingFeedback | null>(null)

/** 合并上游返回与草稿，保留上游暂未返回的历史映射名称。 */
const allUpstreamModels = computed<UpstreamModel[]>(() => {
  const byId = new Map<string, UpstreamModel>()
  for (const model of upstreamModels.value) byId.set(model.id, model)
  for (const names of Object.values(drafts.value)) {
    for (const name of names) {
      if (!byId.has(name)) byId.set(name, { id: name, api_formats: [] })
    }
  }
  return [...byId.values()]
})

const mappingEntries = computed<MappingEntry[]>(() => props.models.map(model => {
  const upstreamNames = drafts.value[model.id] ?? []
  return {
    model,
    upstreamNames,
    isChanged: isMappingChanged(model.id),
    isHidden: hiddenModelIds.value.has(model.id),
  }
}))

const mappedClientCount = computed(() =>
  mappingEntries.value.filter(entry => entry.upstreamNames.length > 0).length)
const unmappedClientCount = computed(() => mappingEntries.value.length - mappedClientCount.value)
const mappedNameCount = computed(() =>
  mappingEntries.value.reduce((count, entry) => count + entry.upstreamNames.length, 0))
const hiddenModels = computed(() => mappingEntries.value.filter(entry => entry.isHidden))

const statusFilterOptions = computed(() => [
  { value: 'all' as const, label: '全部', count: props.models.length },
  { value: 'mapped' as const, label: '已映射', count: mappedClientCount.value },
  { value: 'unmapped' as const, label: '未映射', count: unmappedClientCount.value },
  {
    value: 'changed' as const,
    label: '已修改',
    count: props.models.filter(model => isMappingChanged(model.id)).length,
  },
])

const visibleEntries = computed(() => {
  const query = searchQuery.value.trim().toLowerCase()
  return mappingEntries.value.filter(entry => {
    if (entry.isHidden && !showHiddenModels.value) return false
    if (statusFilter.value === 'mapped' && entry.upstreamNames.length === 0) return false
    if (statusFilter.value === 'unmapped' && entry.upstreamNames.length > 0) return false
    if (statusFilter.value === 'changed' && !entry.isChanged) return false
    if (showOnlyChanged.value && !entry.isChanged) return false
    if (!query) return true
    return clientModelLabel(entry.model).toLowerCase().includes(query)
      || entry.model.provider_model_name.toLowerCase().includes(query)
      || entry.upstreamNames.some(name => name.toLowerCase().includes(query))
  })
})

const changedEntries = computed<DraftEntry[]>(() => props.models.flatMap(model => {
  const upstreamNames = drafts.value[model.id] ?? []
  return isMappingChanged(model.id) ? [{ modelId: model.id, upstreamNames }] : []
}))
const changedClientCount = computed(() => changedEntries.value.length)
const hasUnsavedChanges = computed(() => changedClientCount.value > 0)

const emptyTitle = computed(() => {
  if (props.models.length === 0) return '当前供应商还没有客户端模型'
  if (searchQuery.value.trim()) return '没有匹配的客户端模型'
  if (statusFilter.value === 'changed' || showOnlyChanged.value) return '没有已修改的客户端模型'
  if (statusFilter.value === 'mapped') return '没有已映射的客户端模型'
  if (statusFilter.value === 'unmapped') return '没有未映射的客户端模型'
  return '没有可显示的客户端模型'
})

const emptyDescription = computed(() =>
  props.models.length === 0
    ? '请先在模型列表中关联客户端模型。'
    : '调整搜索或筛选条件后重试。')

/** 客户端模型的工作台展示名，优先使用全局模型展示名。 */
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
  return !sameUpstreamNames(drafts.value[modelId] ?? [], initialMappings.value[modelId] ?? [])
}

function hasScopeValues(values?: string[]): boolean {
  return Array.isArray(values) && values.length > 0
}

// 批量操作只覆盖默认范围；历史特殊范围项保留给原模型映射入口。
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
    if (names.length > 0) mappings[model.id] = names
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

/** 清除上一次保存或上游刷新留下的反馈详情。 */
function clearFeedback() {
  feedback.value = null
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
  clearFeedback()
}

/** 进入行内添加状态，并聚焦当前布局中可见的唯一输入。 */
function beginAdd(modelId: string) {
  if (saving.value) return
  editingModelId.value = modelId
  addSearch.value = ''
  void nextTick(() => {
    const inputs = [...document.querySelectorAll<HTMLInputElement>('[data-mapping-add-input]')]
    const visibleInput = inputs.find(input => input.getClientRects().length > 0)
    ;(visibleInput ?? inputs[0])?.focus()
  })
}

/** 退出行内添加且不修改当前映射草稿。 */
function cancelAdd() {
  editingModelId.value = null
  addSearch.value = ''
}

/** 将当前编辑行的新提供商名称并入草稿，并在成功后退出添加态。 */
function addMappingName(name: string) {
  if (!editingModelId.value) return
  const normalized = name.trim()
  if (!normalized) return
  const currentNames = drafts.value[editingModelId.value] ?? []
  if (currentNames.includes(normalized)) return
  updateDraftNames(editingModelId.value, [...currentNames, normalized])
  cancelAdd()
}

/** 从指定客户端模型的草稿中移除一个提供商映射名称。 */
function removeMappingName(modelId: string, name: string) {
  updateDraftNames(modelId, (drafts.value[modelId] ?? []).filter(item => item !== name))
}

function clearMapping(modelId: string) {
  updateDraftNames(modelId, [])
}

/** 隐藏只改变工作台可见性，不删除映射，也不参与保存载荷。 */
function toggleHidden(modelId: string) {
  const next = new Set(hiddenModelIds.value)
  if (next.has(modelId)) {
    next.delete(modelId)
  } else {
    next.add(modelId)
    if (editingModelId.value === modelId) cancelAdd()
  }
  hiddenModelIds.value = next
}

/** 丢弃全部草稿与工作台隐藏状态，恢复到本次打开时的已保存基线。 */
function resetDraft() {
  drafts.value = Object.fromEntries(
    Object.entries(initialMappings.value).map(([modelId, names]) => [modelId, [...names]]),
  )
  hiddenModelIds.value = new Set()
  editingModelId.value = null
  addSearch.value = ''
  clearFeedback()
}

/** 复制客户端模型标识并沿用全局剪贴板成功或失败提示。 */
async function copyModelId(modelId: string) {
  await copyToClipboard(modelId)
}

async function fetchUpstreamModels(forceRefresh = false) {
  if (!props.providerId) return
  loadingUpstream.value = true
  feedback.value = null
  try {
    const result = await fetchCachedModels(props.providerId, undefined, forceRefresh)
    if (!result.error) upstreamModels.value = result.models
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
  drafts.value = Object.fromEntries(
    Object.entries(initial).map(([modelId, names]) => [modelId, [...names]]),
  )
  initialMappings.value = initial
  upstreamModels.value = []
  searchQuery.value = ''
  statusFilter.value = 'all'
  showOnlyChanged.value = false
  showHiddenModels.value = false
  hiddenModelIds.value = new Set()
  editingModelId.value = null
  addSearch.value = ''
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
    return { ...entry, status: 'rejected', reason: new Error('客户端模型不存在') }
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
    if (successes.length > 0) emit('saved')
    if (failures.length > 0) {
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
  if (!isOpen) {
    cancelAdd()
    return
  }
  resetState()
  if (props.hasAutoFetchKey) await fetchUpstreamModels()
}, { immediate: true })

defineExpose({
  /** 暴露当前草稿供受控预览读取，不改变保存语义。 */
  drafts,
})
</script>
