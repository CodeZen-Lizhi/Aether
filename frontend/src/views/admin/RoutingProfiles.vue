<template>
  <PageContainer>
    <section class="mt-6 space-y-6">
      <!-- 头部：标题 + 保存 -->
      <TableCard class="overflow-hidden">
        <template #header>
          <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <h2 class="text-sm font-semibold">
                调度策略
              </h2>
              <p class="mt-1 text-xs text-muted-foreground">
                全局唯一一份 —— 排序即调度顺序，也是故障转移顺序
              </p>
            </div>
            <div class="flex items-center gap-2">
              <span
                v-if="dirty"
                class="text-xs text-amber-600"
              >
                有未保存的修改
              </span>
              <Button
                size="sm"
                :disabled="loading || saving || !dirty"
                @click="saveStrategy"
              >
                {{ saving ? '保存中…' : '保存策略' }}
              </Button>
            </div>
          </div>
        </template>

        <div
          v-if="loading"
          class="py-10 text-center text-sm text-muted-foreground"
        >
          正在加载调度策略…
        </div>
        <div
          v-else-if="loadError"
          class="py-10 text-center text-sm text-destructive"
        >
          {{ loadError }}
        </div>

        <template v-else>
          <!-- 调度模式：三选一 -->
          <div class="px-4 pb-2 pt-4">
            <h3 class="text-xs font-medium text-muted-foreground">
              调度模式
            </h3>
            <div class="mt-3 grid gap-3 sm:grid-cols-3">
              <button
                v-for="option in schedulingModeOptions"
                :key="option.value"
                type="button"
                class="rounded-lg border p-3 text-left transition-colors"
                :class="mode === option.value
                  ? 'border-primary bg-primary/5 ring-1 ring-primary'
                  : 'border-border hover:bg-muted/40'"
                :aria-pressed="mode === option.value"
                @click="setMode(option.value)"
              >
                <div class="flex items-center justify-between">
                  <span class="text-sm font-medium">{{ option.label }}</span>
                  <span
                    v-if="option.recommended"
                    class="rounded bg-primary/10 px-1.5 py-0.5 text-[10px] text-primary"
                  >
                    推荐
                  </span>
                </div>
                <p class="mt-1 text-xs text-muted-foreground">
                  {{ option.description }}
                </p>
              </button>
            </div>
            <p
              v-if="mode === 'cost_based' && !anyKeyHasMultiplier"
              class="mt-2 text-xs text-amber-600"
            >
              成本优先按 Key 倍率排序——当前还没有 Key 配置倍率，展开下方供应商先设置倍率。
            </p>
          </div>

          <!-- 供应商优先级：拖拽排序 -->
          <div class="border-t px-4 py-4">
            <div class="flex items-center justify-between">
              <h3 class="text-xs font-medium text-muted-foreground">
                供应商优先级（拖拽或用箭头调整，① 最优先）
              </h3>
              <span class="text-xs text-muted-foreground">
                共 {{ orderedProviders.length }} 个
              </span>
            </div>

            <p
              v-if="orderedProviders.length === 0"
              class="mt-3 py-8 text-center text-sm text-muted-foreground"
            >
              还没有供应商——先到「供应商管理」添加供应商和 Key
            </p>

            <ul
              v-else
              class="mt-3 space-y-2"
            >
              <li
                v-for="(provider, index) in orderedProviders"
                :key="provider.id"
                class="rounded-lg border border-border bg-background"
                :draggable="!expandedProviderId"
                @dragstart="onDragStart($event, index)"
                @dragover.prevent
                @drop="onDrop($event, index)"
              >
                <div class="flex items-center gap-2 px-3 py-2.5">
                  <span
                    class="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-primary/10 text-xs font-semibold text-primary"
                  >
                    {{ index + 1 }}
                  </span>
                  <GripVertical class="h-4 w-4 shrink-0 cursor-grab text-muted-foreground" />
                  <button
                    type="button"
                    class="flex min-w-0 flex-1 items-center gap-2 text-left"
                    @click="toggleExpand(provider.id)"
                  >
                    <span class="truncate text-sm font-medium">{{ provider.name }}</span>
                    <Badge
                      v-if="!provider.is_active"
                      variant="secondary"
                      class="shrink-0"
                    >
                      已停用
                    </Badge>
                    <span class="shrink-0 text-xs text-muted-foreground">
                      {{ providerKeyCount(provider.id) }} 个 Key
                    </span>
                  </button>
                  <div class="flex shrink-0 items-center gap-1">
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7"
                      :disabled="index === 0"
                      aria-label="上移"
                      @click="moveProvider(index, -1)"
                    >
                      <ChevronUp class="h-4 w-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7"
                      :disabled="index === orderedProviders.length - 1"
                      aria-label="下移"
                      @click="moveProvider(index, 1)"
                    >
                      <ChevronDown class="h-4 w-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7"
                      :aria-label="expandedProviderId === provider.id ? '收起 Key' : '展开 Key'"
                      @click="toggleExpand(provider.id)"
                    >
                      <ChevronDown
                        class="h-4 w-4 transition-transform"
                        :class="{ 'rotate-180': expandedProviderId === provider.id }"
                      />
                    </Button>
                  </div>
                </div>

                <!-- Key 行内展开：优先级 + 倍率 -->
                <div
                  v-if="expandedProviderId === provider.id"
                  class="border-t bg-muted/20 px-3 py-3"
                >
                  <p
                    v-if="providerKeys(provider.id).length === 0"
                    class="py-2 text-xs text-muted-foreground"
                  >
                    该供应商还没有 Key
                  </p>
                  <div
                    v-else
                    class="space-y-2"
                  >
                    <div
                      v-for="key in providerKeys(provider.id)"
                      :key="key.id"
                      class="rounded-md border border-border/60 bg-background px-3 py-2"
                    >
                      <div class="flex flex-wrap items-center gap-x-3 gap-y-2">
                        <span class="min-w-0 flex-1 truncate text-xs font-medium">
                          {{ key.name || key.api_key_masked || key.id }}
                        </span>
                        <label class="flex items-center gap-1.5 text-xs text-muted-foreground">
                          优先级
                          <Input
                            v-model.number="keyPriorities[key.id]"
                            type="number"
                            min="1"
                            class="h-7 w-16 text-xs"
                            :disabled="!key.is_active"
                            @change="touchKeyPriority(key.id)"
                          />
                        </label>
                        <label
                          v-for="format in key.api_formats"
                          :key="format"
                          class="flex items-center gap-1.5 text-xs text-muted-foreground"
                        >
                          {{ formatLabel(format) }} 倍率
                          <Input
                            type="number"
                            step="0.1"
                            min="0"
                            class="h-7 w-16 text-xs"
                            :value="keyMultiplier(key, format)"
                            :disabled="!key.is_active"
                            @change="saveKeyMultiplier(key, format, $event)"
                          />
                        </label>
                      </div>
                    </div>
                    <p class="text-[11px] leading-relaxed text-muted-foreground">
                      优先级只在同一供应商内平级决胜；倍率供成本优先排序（留空按 1.0）。
                    </p>
                  </div>
                </div>
              </li>
            </ul>
          </div>
        </template>
      </TableCard>
    </section>
  </PageContainer>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ChevronDown, ChevronUp, GripVertical } from 'lucide-vue-next'

import { Badge, Button, Input, TableCard } from '@/components/ui'
import { PageContainer } from '@/components/layout'
import {
  listRoutingGroups,
  createRoutingGroup,
  updateRoutingGroup,
  publishRoutingGroup,
  type RoutingGroupRecord,
} from '@/api/routing-profiles'
import { listAdminProviders, type AdminProviderListItem } from '@/api/endpoints/providers'
import {
  getEndpointKeysGroupedByFormat,
  updateProviderKey,
  type GroupedEndpointKey,
} from '@/api/endpoints/keys'
import type { RoutingGroupConfig } from '@/features/routing/utils/routingPolicy'
import {
  buildSchedulingStrategyConfig,
  findSystemDefaultRoutingGroup,
  parseSchedulingStrategy,
  prioritiesFromOrder,
  type SchedulingStrategyMode,
} from '@/features/routing/utils/schedulingStrategy'
import { useToast } from '@/composables/useToast'
import { parseApiError } from '@/utils/errorParser'
import { log } from '@/utils/logger'

const { success, error: showError } = useToast()

const schedulingModeOptions: Array<{
  value: SchedulingStrategyMode
  label: string
  description: string
  recommended?: boolean
}> = [
  {
    value: 'cache_affinity',
    label: '缓存亲和',
    description: '会话粘住首选供应商，prompt cache 命中率最高',
    recommended: true,
  },
  {
    value: 'fixed_order',
    label: '固定顺序',
    description: '永远严格按优先级主备，无会话亲和',
  },
  {
    value: 'cost_based',
    label: '成本优先',
    description: '同模型下倍率低的 Key 优先，失败换次便宜',
  },
]

const loading = ref(true)
const saving = ref(false)
const loadError = ref<string | null>(null)
const mode = ref<SchedulingStrategyMode>('cache_affinity')
const providers = ref<AdminProviderListItem[]>([])
const keys = ref<GroupedEndpointKey[]>([])
const orderedProviderIds = ref<string[]>([])
const keyPriorities = ref<Record<string, number>>({})
const expandedProviderId = ref<string | null>(null)
const systemDefaultGroup = ref<RoutingGroupRecord | null>(null)
const savedSnapshot = ref<string | null>(null)

const orderedProviders = computed(() => {
  const byId = new Map(providers.value.map(provider => [provider.id, provider]))
  const ordered: AdminProviderListItem[] = []
  for (const providerId of orderedProviderIds.value) {
    const provider = byId.get(providerId)
    if (provider) ordered.push(provider)
  }
  // 保险：数据库里新出现的供应商（还没进排序表）追加到尾部
  for (const provider of providers.value) {
    if (!orderedProviderIds.value.includes(provider.id)) {
      ordered.push(provider)
    }
  }
  return ordered
})

const anyKeyHasMultiplier = computed(() =>
  keys.value.some(
    key => key.rate_multipliers
      && Object.values(key.rate_multipliers).some(value => Number.isFinite(value) && value !== 1),
  ),
)

const dirty = computed(() => currentSnapshot() !== savedSnapshot.value)

function currentSnapshot(): string {
  return JSON.stringify({
    mode: mode.value,
    order: orderedProviderIds.value,
    keyPriorities: keyPriorities.value,
  })
}

function providerKeys(providerId: string): GroupedEndpointKey[] {
  return keys.value.filter(key => key.provider_id === providerId)
}

function providerKeyCount(providerId: string): number {
  return providerKeys(providerId).length
}

function keyMultiplier(key: GroupedEndpointKey, format: string): number | null {
  const value = key.rate_multipliers?.[format]
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

function formatLabel(format: string): string {
  return format.split(':')[0] || format
}

function toggleExpand(providerId: string) {
  expandedProviderId.value = expandedProviderId.value === providerId ? null : providerId
}

function setMode(next: SchedulingStrategyMode) {
  mode.value = next
}

function moveProvider(index: number, direction: -1 | 1) {
  const target = index + direction
  if (target < 0 || target >= orderedProviderIds.value.length) return
  const next = [...orderedProviderIds.value]
  ;[next[index], next[target]] = [next[target], next[index]]
  orderedProviderIds.value = next
  keyPriorities.value = {
    ...keyPriorities.value,
    ...prioritiesFromOrder(orderedProviderIds.value),
  }
}

let dragFromIndexValue: number | null = null
function onDragStart(event: DragEvent, index: number) {
  dragFromIndexValue = index
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move'
  }
}

function onDrop(event: DragEvent, targetIndex: number) {
  event.preventDefault()
  const fromIndex = dragFromIndexValue
  dragFromIndexValue = null
  if (fromIndex === null || fromIndex === targetIndex) return
  const next = [...orderedProviderIds.value]
  const [moved] = next.splice(fromIndex, 1)
  next.splice(targetIndex, 0, moved)
  orderedProviderIds.value = next
  keyPriorities.value = {
    ...keyPriorities.value,
    ...prioritiesFromOrder(orderedProviderIds.value),
  }
}

function touchKeyPriority(keyId: string) {
  const value = keyPriorities.value[keyId]
  if (!Number.isFinite(value) || (value ?? 0) <= 0) {
    delete keyPriorities.value[keyId]
  }
  keyPriorities.value = { ...keyPriorities.value }
}

async function saveKeyMultiplier(key: GroupedEndpointKey, format: string, event: Event) {
  const raw = (event.target as HTMLInputElement).value
  const parsed = Number.parseFloat(raw)
  const merged: Record<string, number> = { ...(key.rate_multipliers ?? {}) }
  if (Number.isFinite(parsed) && parsed > 0) {
    merged[format] = parsed
  } else {
    delete merged[format]
  }
  try {
    await updateProviderKey(key.id, { rate_multipliers: merged })
    key.rate_multipliers = Object.keys(merged).length > 0 ? merged : null
    success('倍率已保存')
  } catch (err) {
    log.error('failed to save key rate multiplier', err)
    showError(parseApiError(err, '倍率保存失败'))
  }
}

async function loadStrategy() {
  loading.value = true
  loadError.value = null
  try {
    const [groupsResponse, providerList, keyList] = await Promise.all([
      listRoutingGroups(),
      listAdminProviders(),
      getEndpointKeysGroupedByFormat().catch(err => {
        log.warn('grouped keys unavailable', err)
        return [] as GroupedEndpointKey[]
      }),
    ])
    providers.value = providerList
    keys.value = keyList

    const group = findSystemDefaultRoutingGroup(groupsResponse.items)
    systemDefaultGroup.value = group
    const state = parseSchedulingStrategy(group?.config_json ?? null)
    mode.value = state.mode
    keyPriorities.value = { ...state.keyPriorities }

    // 供应商顺序：overlay 优先级 → 顺排；未配置的按名称缀在尾部
    const sorted = [...providerList].sort((left, right) => {
      const leftPriority = state.providerPriorities[left.id] ?? Number.MAX_SAFE_INTEGER
      const rightPriority = state.providerPriorities[right.id] ?? Number.MAX_SAFE_INTEGER
      return leftPriority - rightPriority || left.name.localeCompare(right.name)
    })
    orderedProviderIds.value = sorted.map(provider => provider.id)

    savedSnapshot.value = currentSnapshot()
  } catch (err) {
    log.error('failed to load scheduling strategy', err)
    loadError.value = parseApiError(err, '调度策略加载失败')
  } finally {
    loading.value = false
  }
}

async function saveStrategy() {
  const group = systemDefaultGroup.value
  const config: RoutingGroupConfig = buildSchedulingStrategyConfig(
    mode.value,
    orderedProviderIds.value,
    keyPriorities.value,
  )
  saving.value = true
  try {
    if (group) {
      await updateRoutingGroup(group.id, { config_json: config })
      await publishRoutingGroup(group.id)
    } else {
      // 单份策略：首次保存自动创建系统默认组
      const created = await createRoutingGroup({
        name: '默认调度策略',
        description: '全局唯一调度策略（单页形态自动创建）',
        enabled: true,
        is_system_default: true,
        config_json: config,
      })
      await publishRoutingGroup(created.id)
      systemDefaultGroup.value = created
    }
    savedSnapshot.value = currentSnapshot()
    success('调度策略已保存并发布')
  } catch (err) {
    log.error('failed to save scheduling strategy', err)
    showError(parseApiError(err, '调度策略保存失败'))
  } finally {
    saving.value = false
  }
}

onMounted(() => {
  void loadStrategy()
})
</script>
