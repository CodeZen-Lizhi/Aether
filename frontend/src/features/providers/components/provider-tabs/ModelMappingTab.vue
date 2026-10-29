<template>
  <Card class="overflow-hidden">
    <!-- 标题头部 -->
    <div class="p-4 border-b border-border/60">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <h3 class="text-sm font-semibold flex items-center gap-2">
          模型映射
        </h3>
        <Button
          variant="outline"
          size="sm"
          class="h-8"
          @click="openBatchDialog()"
        >
          <ListPlus class="w-3.5 h-3.5 mr-1.5" />
          管理映射
        </Button>
      </div>
    </div>

    <!-- 加载状态 -->
    <div
      v-if="isLoading"
      class="flex items-center justify-center py-12"
    >
      <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
    </div>

    <!-- 映射列表 -->
    <div
      v-else-if="combinedMappings.length > 0"
      ref="mappingsListRef"
      class="divide-y divide-border/40"
    >
      <div
        v-for="item in paginatedMappings"
        :key="item.key"
        class="transition-colors"
      >
        <!-- 行头部（可点击展开） -->
        <div
          class="flex items-start justify-between px-4 py-3 hover:bg-muted/20 cursor-pointer"
          data-mapping-group-toggle
          :data-mapping-group-key="item.key"
          @click="toggleExpand(item.key)"
        >
          <div class="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1.5">
            <!-- 展开/收起图标 -->
            <ChevronRight
              class="w-4 h-4 text-muted-foreground shrink-0 transition-transform self-start mt-0.5"
              :class="{ 'rotate-90': expandedItems.has(item.key) }"
            />
            <!-- 精确映射 -->
            <template v-if="item.type === 'exact'">
              <div class="flex min-w-0 flex-[1_1_12rem] flex-col">
                <span class="font-semibold text-sm truncate">
                  {{ item.targetModelName }}
                </span>
                <span
                  v-if="item.group?.model.provider_model_name"
                  class="text-xs text-muted-foreground truncate"
                >
                  {{ item.group.model.provider_model_name }}
                </span>
              </div>
              <!-- 映射数量 -->
              <span class="text-xs text-muted-foreground shrink-0">
                {{ item.mappings.length }} 个映射
              </span>
            </template>
            <!-- 正则映射 -->
            <template v-else>
              <div class="flex min-w-0 flex-[1_1_12rem] flex-col">
                <span class="font-semibold text-sm truncate">
                  {{ item.targetModelName }}
                </span>
                <span
                  v-if="item.globalModelName"
                  class="text-xs text-muted-foreground truncate"
                >
                  {{ item.globalModelName }}
                </span>
              </div>
              <!-- 类型标签 -->
              <Badge
                variant="secondary"
                class="text-xs shrink-0"
              >
                正则
              </Badge>
              <!-- 分隔符 + 映射数量 -->
              <span class="text-xs text-muted-foreground shrink-0">
                | {{ item.mappings.length }} 个映射
              </span>
              <!-- Key 数量 -->
              <span
                v-if="item.matchedKeys && item.matchedKeys.length > 0"
                class="text-xs text-muted-foreground shrink-0"
              >
                · {{ item.matchedKeys.length }} Key
              </span>
            </template>
          </div>
          <!-- 操作按钮（仅精确映射可编辑/删除） -->
          <div
            v-if="item.type === 'exact'"
            class="flex items-center gap-1.5 ml-4 shrink-0"
            @click.stop
          >
            <Button
              variant="ghost"
              size="icon"
              class="h-8 w-8"
              title="编辑映射"
              @click="item.group && editGroup(item.group)"
            >
              <Edit class="w-3.5 h-3.5" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              class="h-8 w-8 hover:text-destructive"
              title="删除映射"
              @click="item.group && deleteGroup(item.group)"
            >
              <Trash2 class="w-3.5 h-3.5" />
            </Button>
          </div>
        </div>

        <!-- 展开的映射详情 -->
        <div
          v-show="expandedItems.has(item.key)"
          class="bg-muted/30 border-t border-border/30"
        >
          <!-- 精确映射详情 -->
          <div
            v-if="item.type === 'exact'"
            class="px-4 py-2"
          >
            <TransitionGroup
              tag="div"
              name="mapping-priority"
              move-class="mapping-priority-move"
              class="mapping-priority-list space-y-1.5"
            >
              <div
                v-for="(mapping, index) in getExactMappingItems(item)"
                :key="mapping.name"
                class="mapping-priority-row flex min-w-0 items-center gap-2 rounded-lg border border-border/60 bg-background px-2.5 py-1.5"
                :class="{ 'mapping-priority-row--placeholder': draggedMappingName === mapping.name }"
                :data-mapping-name="mapping.name"
              >
                <span
                  class="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-primary/10 text-[10px] font-semibold tabular-nums text-primary"
                >
                  {{ index + 1 }}
                </span>
                <button
                  type="button"
                  class="flex h-7 w-7 shrink-0 touch-none select-none items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  :class="draggedMappingName === mapping.name ? 'cursor-grabbing text-primary' : 'cursor-grab'"
                  :disabled="isMappingReorderSaving(item)"
                  :aria-label="`拖动 ${mapping.name} 调整优先级`"
                  :title="`拖动 ${mapping.name} 调整优先级`"
                  @pointerdown="onMappingPointerDown($event, item, mapping.name)"
                >
                  <GripVertical
                    aria-hidden="true"
                    class="h-4 w-4 pointer-events-none"
                  />
                </button>
                <span
                  class="min-w-0 flex-1 truncate font-mono text-sm"
                  :title="mapping.name"
                >
                  {{ mapping.name }}
                </span>
              </div>
            </TransitionGroup>
          </div>

          <!-- 正则映射详情（按 Key 分组显示） -->
          <div
            v-else
            class="px-4 py-3 space-y-3"
          >
            <div
              v-for="keyItem in item.matchedKeys"
              :key="keyItem.keyId"
              class="bg-background rounded-md border p-3"
            >
              <!-- Key 信息和正则表达式（两列布局） -->
              <div class="flex items-center gap-3">
                <!-- 第一列：Key 名称 + sk -->
                <div class="flex flex-col shrink-0">
                  <span class="font-medium text-sm">{{ keyItem.keyName || '未命名密钥' }}</span>
                  <span class="text-xs text-muted-foreground font-mono">
                    {{ keyItem.maskedKey }}
                  </span>
                </div>
                <!-- 分隔线 -->
                <div
                  v-if="getKeyPatterns(keyItem).length > 0"
                  class="w-px h-8 bg-border shrink-0"
                />
                <!-- 第二列：正则表达式（限制2行） -->
                <div
                  v-if="getKeyPatterns(keyItem).length > 0"
                  class="flex-1 min-w-0"
                >
                  <div
                    class="font-mono text-[11px] text-muted-foreground line-clamp-2"
                    :title="getKeyPatterns(keyItem).join(', ')"
                  >
                    {{ getKeyPatterns(keyItem).join(', ') }}
                  </div>
                </div>
              </div>
              <!-- 匹配的模型列表 -->
              <div class="mt-2 space-y-1">
                <div
                  v-for="match in keyItem.matches"
                  :key="match.name"
                  class="flex items-center justify-between gap-2 py-1"
                >
                  <span class="font-mono text-sm truncate">{{ match.name }}</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
      <!-- 分页控制 -->
      <div
        v-if="shouldPaginateMappings"
        class="px-4 py-2 flex items-center justify-between text-xs text-muted-foreground"
      >
        <span>共 {{ combinedMappings.length }} 个映射</span>
        <div class="flex items-center gap-1.5">
          <Button
            variant="ghost"
            size="sm"
            class="h-6 px-2 text-xs"
            :disabled="currentMappingPage <= 1"
            @click="currentMappingPage--"
          >
            ‹
          </Button>
          <span class="tabular-nums">{{ currentMappingPage }} / {{ totalMappingPages }}</span>
          <Button
            variant="ghost"
            size="sm"
            class="h-6 px-2 text-xs"
            :disabled="currentMappingPage >= totalMappingPages"
            @click="currentMappingPage++"
          >
            ›
          </Button>
        </div>
      </div>
    </div>

    <!-- 空状态 -->
    <div
      v-else
      class="p-8 text-center text-muted-foreground"
    >
      <Tag class="w-12 h-12 mx-auto mb-3 opacity-50" />
      <p class="text-sm">
        暂无模型映射
      </p>
      <p class="text-xs mt-1">
        点击上方“管理映射”统一配置模型映射
      </p>
    </div>
  </Card>

  <Teleport to="body">
    <div
      v-if="draggedMappingPreview && dragPreviewStyle"
      class="mapping-drag-preview"
      :style="dragPreviewStyle"
      aria-hidden="true"
    >
      <span class="mapping-drag-preview__rank flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[10px] font-semibold text-primary">
        {{ draggedMappingPreview.index + 1 }}
      </span>
      <GripVertical
        class="h-4 w-4 shrink-0 text-primary"
        aria-hidden="true"
      />
      <span class="min-w-0 flex-1 truncate font-mono text-sm font-semibold">
        {{ draggedMappingPreview.name }}
      </span>
    </div>
  </Teleport>

  <!-- 统一模型映射工作台 -->
  <BatchModelMappingDialog
    v-if="batchDialogOpen"
    v-model:open="batchDialogOpen"
    :provider-id="provider.id"
    :models="models"
    :can-fetch-upstream-models="canFetchUpstreamModels"
    :initial-model-id="batchInitialModelId"
    @saved="onDialogSaved"
  />

  <!-- 删除确认对话框 -->
  <AlertDialog
    v-model="deleteConfirmOpen"
    title="删除映射"
    :description="deleteConfirmDescription"
    confirm-text="删除"
    cancel-text="取消"
    type="danger"
    @confirm="confirmDelete"
    @cancel="deleteConfirmOpen = false"
  />
</template>

<script setup lang="ts">
import { ref, computed, onBeforeUnmount, watch } from 'vue'
import { useSmartPagination } from '@/composables/useSmartPagination'
import { Tag, ListPlus, Edit, Trash2, ChevronRight, GripVertical } from 'lucide-vue-next'
import {
  Card, Button, Badge,
} from '@/components/ui'
import AlertDialog from '@/components/common/AlertDialog.vue'
import BatchModelMappingDialog from '../BatchModelMappingDialog.vue'
import { useToast } from '@/composables/useToast'
import {
  type Model,
  type ProviderModelAlias,
  type ProviderMappingPreviewResponse,
} from '@/api/endpoints'
import { updateModel } from '@/api/endpoints/models'
import { parseApiError } from '@/utils/errorParser'
import type { ProviderWithEndpointsSummary } from '@/api/endpoints'

interface MappingItem {
  name: string
  priority?: number
  pattern?: string
}

interface MatchedKeyInfo {
  keyId: string
  keyName: string
  maskedKey: string
  matches: MappingItem[]
}

interface CombinedMapping {
  key: string
  type: 'exact' | 'regex'
  targetModelName: string
  targetModelId?: string
  globalModelName?: string  // 正则映射的全局模型名称
  mappings: MappingItem[]
  matchedKeys?: MatchedKeyInfo[]  // 正则映射的 Key 信息
  group?: AliasGroup
}

/** 同一客户端模型的映射分组；数组顺序由保存的 priority 决定。 */
interface AliasGroup {
  model: Model
  /** @deprecated 历史 API 格式分组键，只用于兼容旧数据。 */
  apiFormatsKey: string
  /** @deprecated 历史 API 格式范围，只用于兼容旧数据。 */
  apiFormats: string[]
  endpointIdsKey: string
  endpointIds: string[]
  operationsKey: string
  operations: string[]
  aliases: ProviderModelAlias[]
}

const props = defineProps<{
  provider: ProviderWithEndpointsSummary
  models?: Model[]
  mappingPreview?: ProviderMappingPreviewResponse | null
  loading?: boolean
}>()

const emit = defineEmits<{
  'refresh': []
}>()

const { error: showError, success: showSuccess } = useToast()

// 状态
const localLoading = ref(false)
const batchDialogOpen = ref(false)
const deleteConfirmOpen = ref(false)
const batchInitialModelId = ref<string | null>(null)
const deletingGroup = ref<AliasGroup | null>(null)
const isLoading = computed(() => Boolean(props.loading) || localLoading.value)

// 使用 props 传入的数据
const models = computed(() => props.models ?? [])
const aliasMappingPreview = computed(() => props.mappingPreview ?? null)

// 有活跃密钥时允许弹窗手动拉取上游模型；该操作不依赖自动同步配置。
const canFetchUpstreamModels = computed(() => props.provider.active_keys > 0)

// 展开状态
const expandedItems = ref<Set<string>>(new Set())
const localMappingOrders = ref<Record<string, string[]>>({})
const reorderSavingGroupKey = ref<string | null>(null)
const draggedMappingGroup = ref<AliasGroup | null>(null)
const draggedMappingName = ref<string | null>(null)
const dragPointer = ref({ x: 0, y: 0 })
const dragOrigin = ref<{
  left: number
  top: number
  width: number
  startX: number
  startY: number
} | null>(null)

let activeDragPointerId: number | null = null
let activeDragHandle: HTMLElement | null = null
let activeDragList: HTMLElement | null = null
let dragInitialNames: string[] = []

/** 精确映射分组的稳定键；用于隔离各客户端模型的本地拖动顺序。 */
function getExactGroupKey(group: AliasGroup): string {
  return `exact-${group.model.id}-${group.apiFormatsKey}`
}

/** 返回当前展示顺序；拖动期间优先使用本地顺序，保存后等待父级刷新。 */
function getExactMappingNames(group: AliasGroup): string[] {
  return localMappingOrders.value[getExactGroupKey(group)]
    ?? group.aliases.map(alias => alias.name)
}

/** 按当前展示顺序返回精确映射项，供列表摘要与展开区共同使用。 */
function getExactMappingItems(item: CombinedMapping): MappingItem[] {
  const group = item.group
  if (item.type !== 'exact' || !group) return item.mappings
  const aliasesByName = new Map(group.aliases.map(alias => [alias.name, alias]))
  return getExactMappingNames(group).map((name, index) => ({
    name,
    priority: aliasesByName.get(name)?.priority ?? index + 1,
  }))
}

/** 当前分组正在保存优先级时禁用拖动，避免并发写入覆盖。 */
function isMappingReorderSaving(item: CombinedMapping): boolean {
  return Boolean(item.group && reorderSavingGroupKey.value === getExactGroupKey(item.group))
}

// 生成 API 格式唯一键，用于合并同一模型下的映射。
function getApiFormatsKey(formats: string[] | undefined): string {
  return getScopeKey(formats)
}

function normalizeStringList(values: string[] | undefined): string[] {
  const seen = new Set<string>()
  const result: string[] = []
  for (const value of values ?? []) {
    const normalized = value.trim()
    if (!normalized || seen.has(normalized)) continue
    seen.add(normalized)
    result.push(normalized)
  }
  return result
}

function getScopeKey(values: string[] | undefined): string {
  return normalizeStringList(values).sort().join(',')
}

// 精确映射分组（来自 provider_model_mappings）
const exactMappingGroups = computed<AliasGroup[]>(() => {
  const groups: AliasGroup[] = []
  const groupMap = new Map<string, AliasGroup>()

  for (const model of models.value) {
    if (!model.provider_model_mappings || !Array.isArray(model.provider_model_mappings)) continue

    for (const alias of model.provider_model_mappings) {
      const apiFormatsKey = getApiFormatsKey(alias.api_formats)
      const groupKey = `${model.id}|${apiFormatsKey}`

      if (!groupMap.has(groupKey)) {
        const group: AliasGroup = {
          model,
          apiFormatsKey,
          apiFormats: alias.api_formats || [],
          endpointIdsKey: '',
          endpointIds: [],
          operationsKey: '',
          operations: [],
          aliases: []
        }
        groupMap.set(groupKey, group)
        groups.push(group)
      }
      groupMap.get(groupKey)?.aliases.push(alias)
    }
  }

  for (const group of groups) {
    group.aliases.sort((a, b) => a.priority - b.priority || a.name.localeCompare(b.name))
  }

  return groups
})

// 正则映射（来自 aliasMappingPreview，按 GlobalModel 分组并保留 Key 信息）
const regexMappings = computed<CombinedMapping[]>(() => {
  if (!aliasMappingPreview.value) return []

  const result: CombinedMapping[] = []
  const modelMap = new Map<string, CombinedMapping>()

  const previewKeys = Array.isArray(aliasMappingPreview.value.keys)
    ? aliasMappingPreview.value.keys
    : []
  for (const keyInfo of previewKeys) {
    const matchingGlobalModels = Array.isArray(keyInfo.matching_global_models)
      ? keyInfo.matching_global_models
      : []
    for (const gm of matchingGlobalModels) {
      const matchedModels = Array.isArray(gm.matched_models) ? gm.matched_models : []
      if (!modelMap.has(gm.global_model_id)) {
        modelMap.set(gm.global_model_id, {
          key: `regex-${gm.global_model_id}`,
          type: 'regex',
          targetModelName: gm.display_name || gm.global_model_name || gm.global_model_id,
          targetModelId: gm.global_model_id,
          globalModelName: gm.global_model_name,
          mappings: [],
          matchedKeys: []
        })
        result.push(modelMap.get(gm.global_model_id) as CombinedMapping)
      }

      const mapping = modelMap.get(gm.global_model_id)
      if (!mapping) continue

      // 添加 Key 信息
      const keyMatches: MappingItem[] = matchedModels.map(m => ({
        name: m.allowed_model,
        pattern: m.mapping_pattern
      }))

      mapping.matchedKeys?.push({
        keyId: keyInfo.key_id,
        keyName: keyInfo.key_name,
        maskedKey: keyInfo.masked_key,
        matches: keyMatches
      })

      // 收集所有映射（去重）
      for (const match of matchedModels) {
        if (!mapping.mappings.some(m => m.name === match.allowed_model)) {
          mapping.mappings.push({
            name: match.allowed_model,
            pattern: match.mapping_pattern
          })
        }
      }
    }
  }

  return result
})

// 合并后的映射列表
const combinedMappings = computed<CombinedMapping[]>(() => {
  const result: CombinedMapping[] = []

  // 添加精确映射
  for (const group of exactMappingGroups.value) {
    const combined: CombinedMapping = {
      key: `exact-${group.model.id}-${group.apiFormatsKey}`,
      type: 'exact',
      targetModelName: group.model.global_model_display_name || group.model.provider_model_name,
      targetModelId: group.model.id,
      mappings: [],
      group
    }
    combined.mappings = getExactMappingItems(combined)
    result.push(combined)
  }

  // 添加正则映射
  for (const mapping of regexMappings.value) {
    result.push(mapping)
  }

  return result.sort((a, b) => {
    // 精确映射排在前面
    if (a.type !== b.type) return a.type === 'exact' ? -1 : 1
    return a.targetModelName.localeCompare(b.targetModelName)
  })
})

// ===== 模型映射智能分页 =====
const mappingsListRef = ref<HTMLElement | null>(null)
const {
  currentPage: currentMappingPage,
  totalPages: totalMappingPages,
  shouldPaginate: shouldPaginateMappings,
  paginatedItems: paginatedMappings,
} = useSmartPagination(combinedMappings, mappingsListRef)

// 刷新数据（通知父组件刷新）
function refresh() {
  emit('refresh')
}

// 删除确认描述
const deleteConfirmDescription = computed(() => {
  if (!deletingGroup.value) return ''
  const { model, aliases } = deletingGroup.value
  const modelName = model.global_model_display_name || model.provider_model_name
  const aliasNames = aliases.map(a => a.name).join(', ')
  return `确定要删除模型「${modelName}」下的 ${aliases.length} 个映射吗？\n\n映射名称：${aliasNames}`
})

// 切换展开状态
function toggleExpand(key: string) {
  if (expandedItems.value.has(key)) {
    expandedItems.value.delete(key)
  } else {
    expandedItems.value.add(key)
  }
}

// 获取单个 Key 的去重正则模式列表
function getKeyPatterns(keyItem: MatchedKeyInfo): string[] {
  const patterns = new Set<string>()
  for (const match of keyItem.matches) {
    if (match.pattern) {
      patterns.add(match.pattern)
    }
  }
  return Array.from(patterns)
}

/** 打开统一映射工作台；传入模型标识时只定位该客户端模型。 */
function openBatchDialog(initialModelId: string | null = null) {
  batchInitialModelId.value = initialModelId
  batchDialogOpen.value = true
}

/** 从映射列表进入统一工作台，并定位到当前客户端模型。 */
function editGroup(group: AliasGroup) {
  openBatchDialog(group.model.id)
}

// 删除分组
function deleteGroup(group: AliasGroup) {
  deletingGroup.value = group
  deleteConfirmOpen.value = true
}

const draggedMappingPreview = computed(() => {
  const group = draggedMappingGroup.value
  const name = draggedMappingName.value
  if (!group || !name) return null
  return {
    name,
    index: getExactMappingNames(group).indexOf(name),
  }
})

/** 拖动浮层跟随指针，并沿用调度策略列表的轻微倾斜与阴影反馈。 */
const dragPreviewStyle = computed(() => {
  const origin = dragOrigin.value
  if (!origin) return undefined
  const dx = dragPointer.value.x - origin.startX
  const dy = dragPointer.value.y - origin.startY
  const reduceMotion = typeof window !== 'undefined'
    && typeof window.matchMedia === 'function'
    && window.matchMedia('(prefers-reduced-motion: reduce)').matches
  const tiltY = Math.max(-3, Math.min(3, dx * 0.02))
  const tiltZ = Math.max(-1.8, Math.min(1.8, dx * 0.012))
  return {
    left: `${origin.left}px`,
    top: `${origin.top}px`,
    width: `${origin.width}px`,
    transform: reduceMotion
      ? `translate3d(${dx}px, ${dy}px, 0)`
      : `perspective(900px) translate3d(${dx}px, ${dy}px, 0) rotateX(1.5deg) rotateY(${tiltY}deg) rotateZ(${tiltZ}deg) scale(1.025)`,
  }
})

/** 将某个精确映射分组的本地顺序写回，不影响其他分组。 */
function setMappingOrder(group: AliasGroup, names: string[]) {
  localMappingOrders.value = {
    ...localMappingOrders.value,
    [getExactGroupKey(group)]: names,
  }
}

/** 取消拖动时丢弃本地顺序覆盖，恢复服务端返回的基线。 */
function clearMappingOrder(group: AliasGroup) {
  const next = { ...localMappingOrders.value }
  delete next[getExactGroupKey(group)]
  localMappingOrders.value = next
}

/** 开始拖动当前展开分组中的单个映射。 */
function onMappingPointerDown(event: PointerEvent, item: CombinedMapping, name: string) {
  const group = item.group
  if (item.type !== 'exact' || !group) return
  if (event.button !== 0 || reorderSavingGroupKey.value === getExactGroupKey(group)) return
  event.preventDefault()
  const handle = event.currentTarget as HTMLElement
  const row = handle.closest<HTMLElement>('[data-mapping-name]')
  const list = handle.closest<HTMLElement>('.mapping-priority-list')
  const rect = row?.getBoundingClientRect()
  if (!rect || !list) return

  draggedMappingGroup.value = group
  draggedMappingName.value = name
  dragInitialNames = getExactMappingNames(group)
  dragPointer.value = { x: event.clientX, y: event.clientY }
  dragOrigin.value = {
    left: rect.left,
    top: rect.top,
    width: rect.width,
    startX: event.clientX,
    startY: event.clientY,
  }
  activeDragPointerId = event.pointerId
  activeDragHandle = handle
  activeDragList = list
  handle.setPointerCapture?.(event.pointerId)
  window.addEventListener('pointermove', onMappingPointerMove)
  window.addEventListener('pointerup', onMappingPointerEnd)
  window.addEventListener('pointercancel', onMappingPointerCancel)
  window.addEventListener('blur', onMappingPointerCancel)
}

/** 指针跨过其他行中线时实时交换顺序，让列表连续让位。 */
function onMappingPointerMove(event: PointerEvent) {
  const group = draggedMappingGroup.value
  const name = draggedMappingName.value
  if (!group || !name || event.pointerId !== activeDragPointerId || !activeDragList) return
  event.preventDefault()
  dragPointer.value = { x: event.clientX, y: event.clientY }

  const rows = Array.from(activeDragList.querySelectorAll<HTMLElement>('[data-mapping-name]'))
  const next = [...getExactMappingNames(group)]
  const currentIndex = next.indexOf(name)
  if (currentIndex < 0) return

  let targetIndex = currentIndex
  for (const row of rows) {
    const rowName = row.dataset.mappingName
    if (!rowName || rowName === name) continue
    const rect = row.getBoundingClientRect()
    const rowIndex = next.indexOf(rowName)
    if (rowIndex < 0) continue
    if (event.clientY < rect.top) break
    if (event.clientY <= rect.bottom) {
      if (event.clientY >= rect.top + rect.height / 2) {
        targetIndex = rowIndex + 1
      } else {
        targetIndex = Math.min(targetIndex, rowIndex)
      }
      break
    }
    targetIndex = rowIndex + 1
  }

  if (targetIndex === currentIndex) return
  const [moved] = next.splice(currentIndex, 1)
  const insertAt = targetIndex > currentIndex ? targetIndex - 1 : targetIndex
  next.splice(insertAt, 0, moved)
  setMappingOrder(group, next)
}

function onMappingPointerEnd(event: PointerEvent) {
  if (event.pointerId !== activeDragPointerId) return
  finishMappingDrag(true)
}

function onMappingPointerCancel() {
  finishMappingDrag(false)
}

/** 结束拖动；松手时保存顺序，取消或失焦时恢复原顺序。 */
function finishMappingDrag(commit: boolean) {
  const group = draggedMappingGroup.value
  const finalNames = group ? [...getExactMappingNames(group)] : []
  const changed = group
    ? finalNames.some((name, index) => name !== dragInitialNames[index])
      || finalNames.length !== dragInitialNames.length
    : false

  window.removeEventListener('pointermove', onMappingPointerMove)
  window.removeEventListener('pointerup', onMappingPointerEnd)
  window.removeEventListener('pointercancel', onMappingPointerCancel)
  window.removeEventListener('blur', onMappingPointerCancel)

  const handle = activeDragHandle
  const pointerId = activeDragPointerId
  if (handle && pointerId !== null && handle.hasPointerCapture?.(pointerId)) {
    handle.releasePointerCapture(pointerId)
  }

  draggedMappingGroup.value = null
  draggedMappingName.value = null
  dragOrigin.value = null
  activeDragPointerId = null
  activeDragHandle = null
  activeDragList = null
  dragInitialNames = []

  if (!group || !changed) return
  if (commit) {
    void persistMappingOrder(group, finalNames)
  } else {
    clearMappingOrder(group)
  }
}

/** 将拖动顺序保存为连续的 `1..N` priority，并刷新供应商详情。 */
async function persistMappingOrder(group: AliasGroup, names: string[]) {
  const aliasesByName = new Map(group.aliases.map(alias => [alias.name, alias]))
  const groupAliases = new Set(group.aliases)
  const orderedAliases = names.flatMap((name, index) => {
    const alias = aliasesByName.get(name)
    return alias ? [{ ...alias, priority: index + 1 }] : []
  })
  const otherAliases = (group.model.provider_model_mappings ?? [])
    .filter(alias => !groupAliases.has(alias))
  const nextMappings = [...otherAliases, ...orderedAliases]

  reorderSavingGroupKey.value = getExactGroupKey(group)
  try {
    await updateModel(props.provider.id, group.model.id, {
      provider_model_mappings: nextMappings,
    })
    showSuccess('映射优先级已更新')
    emit('refresh')
  } catch (error: unknown) {
    clearMappingOrder(group)
    showError(parseApiError(error, '更新映射优先级失败'), '错误')
  } finally {
    reorderSavingGroupKey.value = null
  }
}

// 确认删除
async function confirmDelete() {
  if (!deletingGroup.value) return

  const { model, aliases, apiFormatsKey } = deletingGroup.value

  try {
    const currentAliases = model.provider_model_mappings || []
    const aliasNamesToRemove = new Set(aliases.map(a => a.name))
    const newAliases = currentAliases.filter((a: ProviderModelAlias) => {
      const currentKey = getApiFormatsKey(a.api_formats)
      return !(currentKey === apiFormatsKey
        && aliasNamesToRemove.has(a.name))
    })

    await updateModel(props.provider.id, model.id, {
      provider_model_mappings: newAliases.length > 0 ? newAliases : null
    })

    showSuccess('映射已删除')
    deleteConfirmOpen.value = false
    deletingGroup.value = null
    emit('refresh')
  } catch (err: unknown) {
    showError(parseApiError(err, '删除失败'), '错误')
  }
}

// 对话框保存后回调
async function onDialogSaved() {
  emit('refresh')
}

// 暴露给父组件
defineExpose({
  dialogOpen: computed(() => batchDialogOpen.value || deleteConfirmOpen.value),
  reload: refresh
})

onBeforeUnmount(() => finishMappingDrag(false))

watch(() => props.models, () => {
  if (!draggedMappingGroup.value) {
    localMappingOrders.value = {}
  }
})
</script>

<style scoped>
.mapping-priority-list {
  position: relative;
}

.mapping-priority-row {
  position: relative;
  transform-origin: center;
  transition:
    border-color 140ms ease,
    background-color 140ms ease,
    box-shadow 140ms ease;
}

.mapping-priority-move {
  transition: transform 190ms cubic-bezier(0.2, 0.75, 0.25, 1);
}

.mapping-priority-row--placeholder {
  border-color: color-mix(in srgb, var(--primary) 70%, transparent);
  border-style: dashed;
  background: color-mix(in srgb, var(--primary) 8%, var(--background));
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--primary) 12%, transparent);
}

.mapping-priority-row--placeholder > * {
  opacity: 0;
}

.mapping-drag-preview {
  position: fixed;
  z-index: 80;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  box-sizing: border-box;
  min-width: 0;
  padding: 0.5rem 0.625rem;
  pointer-events: none;
  color: var(--foreground);
  border: 1px solid color-mix(in srgb, var(--primary) 72%, white 10%);
  border-radius: 0.5rem;
  background: color-mix(in srgb, var(--background) 88%, var(--primary) 12%);
  box-shadow:
    0 22px 42px -18px color-mix(in srgb, var(--primary) 55%, black),
    0 12px 20px -14px rgb(0 0 0 / 55%),
    inset 0 1px 0 color-mix(in srgb, white 18%, transparent);
  transform-origin: center;
  will-change: transform;
}

.mapping-drag-preview__rank {
  background: color-mix(in srgb, var(--primary) 15%, transparent);
}

@media (prefers-reduced-motion: reduce) {
  .mapping-priority-row,
  .mapping-priority-move {
    transition: none;
  }
}
</style>
