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

            <TransitionGroup
              v-else
              tag="ul"
              name="provider-row"
              move-class="provider-row-move"
              class="provider-priority-list mt-3 space-y-2"
            >
              <li
                v-for="(provider, index) in orderedProviders"
                :key="provider.id"
                :data-provider-id="provider.id"
                class="provider-row rounded-lg border border-border bg-background"
                :class="draggedProviderId === provider.id ? 'provider-row--placeholder' : ''"
              >
                <div class="flex items-center gap-2 px-3 py-2.5">
                  <span
                    class="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-primary/10 text-xs font-semibold text-primary"
                  >
                    {{ index + 1 }}
                  </span>
                  <button
                    type="button"
                    class="flex h-7 w-7 shrink-0 touch-none select-none items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    :class="draggedProviderId === provider.id ? 'cursor-grabbing text-primary' : 'cursor-grab'"
                    :aria-label="`拖动 ${provider.name} 调整优先级`"
                    :title="`拖动 ${provider.name} 调整优先级`"
                    @pointerdown="onPointerDragStart($event, provider.id)"
                    @pointermove="onPointerDragMove"
                    @pointerup="onPointerDragEnd"
                    @pointercancel="onPointerDragEnd"
                    @lostpointercapture="onPointerDragEnd"
                  >
                    <GripVertical
                      aria-hidden="true"
                      class="h-4 w-4 pointer-events-none"
                    />
                  </button>
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
            </TransitionGroup>

            <Teleport to="body">
              <div
                v-if="draggedProvider && dragPreview"
                class="provider-drag-preview"
                :style="dragPreviewStyle"
                aria-hidden="true"
              >
                <span class="provider-drag-preview__rank flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-xs font-semibold text-primary">
                  {{ dragPreview.index + 1 }}
                </span>
                <GripVertical
                  class="h-4 w-4 shrink-0 text-primary"
                  aria-hidden="true"
                />
                <span class="min-w-0 flex-1 truncate text-sm font-semibold">
                  {{ dragPreview.provider.name }}
                </span>
                <Badge
                  v-if="!dragPreview.provider.is_active"
                  variant="secondary"
                  class="shrink-0"
                >
                  已停用
                </Badge>
              </div>
            </Teleport>
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
const systemDefaultGroup = ref<RoutingGroupRecord | null>(null)
const savedSnapshot = ref<string | null>(null)
const dragOrigin = ref<{ left: number, top: number, width: number, startX: number, startY: number } | null>(null)
const dragPointer = ref({ x: 0, y: 0 })

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

const draggedProviderId = ref<string | null>(null)

const draggedProvider = computed(() => (
  draggedProviderId.value
    ? providers.value.find(provider => provider.id === draggedProviderId.value) ?? null
    : null
))

const dragPreview = computed(() => {
  const provider = draggedProvider.value
  if (!provider) return null
  return {
    provider,
    index: orderedProviders.value.findIndex(item => item.id === provider.id),
  }
})

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

function onPointerDragStart(event: PointerEvent, providerId: string) {
  if (event.button !== 0) return
  event.preventDefault()
  draggedProviderId.value = providerId
  dragPointer.value = { x: event.clientX, y: event.clientY }

  const row = (event.currentTarget as HTMLElement).closest<HTMLElement>('[data-provider-id]')
  const rect = row?.getBoundingClientRect()
  if (rect) {
    dragOrigin.value = {
      left: rect.left,
      top: rect.top,
      width: rect.width,
      startX: event.clientX,
      startY: event.clientY,
    }
  }

  const handle = event.currentTarget as HTMLElement
  handle.setPointerCapture?.(event.pointerId)
}

function onPointerDragMove(event: PointerEvent) {
  const providerId = draggedProviderId.value
  if (!providerId) return
  event.preventDefault()
  dragPointer.value = { x: event.clientX, y: event.clientY }

  const target = document.elementFromPoint(event.clientX, event.clientY)
    ?.closest<HTMLElement>('[data-provider-id]')
  if (!target?.parentElement?.classList.contains('provider-priority-list')) return
  const targetProviderId = target?.dataset.providerId
  if (!targetProviderId || targetProviderId === providerId) return

  const fromIndex = orderedProviderIds.value.indexOf(providerId)
  const targetIndex = orderedProviderIds.value.indexOf(targetProviderId)
  if (fromIndex < 0 || targetIndex < 0 || fromIndex === targetIndex) return

  const targetRect = target.getBoundingClientRect()
  const movingDown = fromIndex < targetIndex
  const targetMidpoint = targetRect.top + targetRect.height / 2
  if ((movingDown && event.clientY < targetMidpoint) || (!movingDown && event.clientY > targetMidpoint)) return

  const next = [...orderedProviderIds.value]
  const [moved] = next.splice(fromIndex, 1)
  next.splice(targetIndex, 0, moved)
  orderedProviderIds.value = next
}

function onPointerDragEnd(event: PointerEvent) {
  draggedProviderId.value = null
  dragOrigin.value = null

  const handle = event.currentTarget as HTMLElement
  if (handle.hasPointerCapture?.(event.pointerId)) {
    handle.releasePointerCapture(event.pointerId)
  }
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
  const snapshot = currentSnapshot()
  const config: RoutingGroupConfig = buildSchedulingStrategyConfig(
    mode.value,
    orderedProviderIds.value,
    group?.config_json,
  )
  saving.value = true
  try {
    if (group) {
      await updateRoutingGroup(group.id, { config_json: config })
      systemDefaultGroup.value = await publishRoutingGroup(group.id)
    } else {
      // 单份策略：首次保存自动创建系统默认组
      const created = await createRoutingGroup({
        name: '默认调度策略',
        description: '全局唯一调度策略（单页形态自动创建）',
        enabled: true,
        is_system_default: true,
        config_json: config,
      })
      systemDefaultGroup.value = await publishRoutingGroup(created.id)
    }
    savedSnapshot.value = snapshot
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

<style scoped>
.provider-row {
  transform-origin: center;
  transition:
    border-color 140ms ease,
    background-color 140ms ease,
    box-shadow 140ms ease;
}

.provider-row-move {
  transition: transform 190ms cubic-bezier(0.2, 0.75, 0.25, 1);
}

.provider-row--placeholder {
  border-color: color-mix(in srgb, var(--primary) 70%, transparent);
  border-style: dashed;
  background: color-mix(in srgb, var(--primary) 8%, var(--background));
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--primary) 12%, transparent);
}

.provider-row--placeholder > div {
  opacity: 0;
}

.provider-drag-preview {
  position: fixed;
  z-index: 80;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  box-sizing: border-box;
  min-width: 0;
  padding: 0.625rem 0.75rem;
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

.provider-drag-preview__rank {
  background: color-mix(in srgb, var(--primary) 15%, transparent);
}

@media (prefers-reduced-motion: reduce) {
  .provider-row,
  .provider-row-move {
    transition: none;
  }
}
</style>
