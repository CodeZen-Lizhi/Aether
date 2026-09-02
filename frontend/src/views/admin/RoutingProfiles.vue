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
              v-if="mode === 'cost_based'"
              class="mt-2 text-xs text-muted-foreground"
            >
              成本优先按供应商 Key 的倍率排序，请在供应商管理中设置倍率。
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
                draggable="true"
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
                  <div class="flex min-w-0 flex-1 items-center gap-2 text-left">
                    <span class="truncate text-sm font-medium">{{ provider.name }}</span>
                    <Badge
                      v-if="!provider.is_active"
                      variant="secondary"
                      class="shrink-0"
                    >
                      已停用
                    </Badge>
                  </div>
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

import { Badge, Button, TableCard } from '@/components/ui'
import { PageContainer } from '@/components/layout'
import {
  listRoutingGroups,
  createRoutingGroup,
  updateRoutingGroup,
  publishRoutingGroup,
  type RoutingGroupRecord,
} from '@/api/routing-profiles'
import { listAdminProviders, type AdminProviderListItem } from '@/api/endpoints/providers'
import type { RoutingGroupConfig } from '@/features/routing/utils/routingPolicy'
import {
  buildSchedulingStrategyConfig,
  findSystemDefaultRoutingGroup,
  parseSchedulingStrategy,
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
const orderedProviderIds = ref<string[]>([])
// 保留已存在的 Key 优先级覆盖，页面不再提供重复编辑入口。
const keyPriorities = ref<Record<string, number>>({})
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

const dirty = computed(() => currentSnapshot() !== savedSnapshot.value)

function currentSnapshot(): string {
  return JSON.stringify({
    mode: mode.value,
    order: orderedProviderIds.value,
    keyPriorities: keyPriorities.value,
  })
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
}

async function loadStrategy() {
  loading.value = true
  loadError.value = null
  try {
    const [groupsResponse, providerList] = await Promise.all([
      listRoutingGroups(),
      listAdminProviders(),
    ])
    providers.value = providerList

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
