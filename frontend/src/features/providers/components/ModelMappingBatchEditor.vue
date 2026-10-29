<template>
  <!-- 批量映射行内编辑器；候选浮层通过 Portal 避免被可滚动表格裁切。 -->
  <div class="min-w-0">
    <div
      v-if="names.length > 0 || !editing"
      class="space-y-1.5"
    >
      <TransitionGroup
        v-if="names.length > 0"
        tag="div"
        name="mapping-order"
        class="space-y-1.5"
      >
        <div
          v-for="(name, index) in names"
          :key="name"
          class="group flex min-w-0 items-center gap-2 rounded-lg border bg-background px-2 py-1.5 transition-colors"
          :class="[
            draggedIndex === index
              ? 'border-primary/60 bg-primary/5 opacity-70'
              : dragOverIndex === index
                ? 'border-primary/40 bg-primary/5'
                : 'border-border/60 hover:border-border hover:bg-muted/20',
          ]"
          :draggable="!disabled"
          data-batch-mapping-order-row
          :data-batch-mapping-order-name="name"
          @dragstart="handleDragStart(index, $event)"
          @dragend="handleDragEnd"
          @dragover.prevent="handleDragOver(index)"
          @dragleave="handleDragLeave(index)"
          @drop.prevent="handleDrop(index)"
        >
          <span
            class="flex h-6 w-5 shrink-0 cursor-grab items-center justify-center text-muted-foreground/60 transition-colors group-hover:text-muted-foreground active:cursor-grabbing"
            title="拖动调整顺序"
            aria-hidden="true"
          >
            <GripVertical class="h-3.5 w-3.5" />
          </span>
          <span
            class="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-primary/10 text-[10px] font-semibold tabular-nums text-primary"
          >
            {{ index + 1 }}
          </span>
          <span
            class="min-w-0 flex-1 truncate font-mono text-[11px] text-foreground"
            :title="name"
          >
            {{ name }}
          </span>
          <span class="flex shrink-0 items-center gap-0.5">
            <button
              type="button"
              class="rounded p-0.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:pointer-events-none disabled:opacity-30"
              :disabled="disabled || index === 0"
              :aria-label="`上移 ${name}`"
              :title="`上移 ${name}`"
              draggable="false"
              @click="$emit('reorder', index, index - 1)"
            >
              <ArrowUp class="h-3 w-3" />
            </button>
            <button
              type="button"
              class="rounded p-0.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:pointer-events-none disabled:opacity-30"
              :disabled="disabled || index === names.length - 1"
              :aria-label="`下移 ${name}`"
              :title="`下移 ${name}`"
              draggable="false"
              @click="$emit('reorder', index, index + 1)"
            >
              <ArrowDown class="h-3 w-3" />
            </button>
            <button
              type="button"
              class="rounded p-0.5 text-primary/60 transition-colors hover:bg-primary/10 hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:pointer-events-none disabled:opacity-50"
              :aria-label="`移除 ${name}`"
              :title="`移除 ${name}`"
              draggable="false"
              :disabled="disabled"
              @click="$emit('remove', name)"
            >
              <X class="h-3 w-3" />
            </button>
          </span>
        </div>
      </TransitionGroup>
      <button
        v-if="!editing"
        type="button"
        class="inline-flex shrink-0 items-center gap-1 rounded-lg border border-dashed border-border bg-muted/20 px-2 py-1 text-[11px] font-medium text-muted-foreground transition-colors hover:border-primary/50 hover:bg-primary/5 hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:pointer-events-none disabled:opacity-50"
        :aria-label="`为 ${modelName} 添加提供商模型`"
        :disabled="disabled"
        @click="$emit('start')"
      >
        <Plus class="h-3 w-3" />
        添加
      </button>
    </div>

    <button
      v-if="!editing && names.length === 0"
      type="button"
      class="inline-flex w-full items-center justify-center gap-1 rounded-lg border border-dashed border-border bg-muted/20 px-2 py-2 text-xs font-medium text-muted-foreground transition-colors hover:border-primary/50 hover:bg-primary/5 hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:pointer-events-none disabled:opacity-50"
      :aria-label="`为 ${modelName} 添加提供商模型`"
      :disabled="disabled"
      @click="$emit('start')"
    >
      <Plus class="h-3 w-3" />
      添加提供商模型
    </button>

    <Popover
      v-if="editing"
      :open="true"
    >
      <PopoverAnchor as-child>
        <div class="py-1.5">
          <div class="flex items-center gap-2 rounded-lg border border-primary bg-background px-2.5 py-1.5 shadow-[0_0_0_3px_rgba(37,99,235,0.08)]">
            <Search class="h-3 w-3 shrink-0 text-muted-foreground" />
            <input
              :id="inputId"
              data-mapping-add-input
              :value="search"
              class="min-w-0 flex-1 border-0 bg-transparent p-0 font-mono text-[11px] text-foreground outline-none placeholder:font-sans placeholder:text-muted-foreground"
              placeholder="搜索或输入提供商模型"
              role="combobox"
              aria-autocomplete="list"
              :aria-expanded="true"
              :aria-controls="listboxId"
              :aria-label="`${modelName} 的提供商模型`"
              :disabled="disabled"
              @input="$emit('update-search', ($event.target as HTMLInputElement).value)"
              @keydown="handleKeydown"
            >
            <button
              type="button"
              class="shrink-0 rounded p-0.5 text-muted-foreground transition-colors hover:text-foreground"
              title="取消添加"
              :disabled="disabled"
              @click="$emit('cancel')"
            >
              <X class="h-3 w-3" />
            </button>
          </div>
        </div>
      </PopoverAnchor>

      <PopoverContent
        class="z-[80] w-[var(--radix-popover-trigger-width)] p-1"
        side="bottom"
        align="start"
        :collision-padding="12"
        :disable-outside-pointer-events="false"
        force-mount
      >
        <div
          :id="listboxId"
          role="listbox"
          class="max-h-52 overflow-y-auto"
        >
          <button
            v-for="option in suggestions"
            :key="option.id"
            type="button"
            role="option"
            :aria-selected="false"
            class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left font-mono text-[11px] text-foreground transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
            @click="$emit('add', option.id)"
          >
            <Plus class="h-3 w-3 shrink-0 text-muted-foreground" />
            <span class="truncate">{{ option.id }}</span>
          </button>
          <button
            v-if="canCreateCustom"
            type="button"
            role="option"
            :aria-selected="false"
            class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left font-mono text-[11px] text-primary transition-colors hover:bg-primary/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
            @click="$emit('add', normalizedSearch)"
          >
            <Plus class="h-3 w-3 shrink-0" />
            <span class="truncate">创建 “{{ normalizedSearch }}”</span>
          </button>
          <p
            v-if="suggestions.length === 0 && !canCreateCustom"
            class="px-2 py-2 text-[11px] text-muted-foreground"
          >
            没有可添加的提供商模型
          </p>
        </div>
      </PopoverContent>
    </Popover>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { ArrowDown, ArrowUp, GripVertical, Plus, Search, X } from 'lucide-vue-next'
import { Popover, PopoverAnchor, PopoverContent } from '@/components/ui'
import type { UpstreamModel } from '@/api/endpoints'

const props = defineProps<{
  /** 当前响应式布局标识，用于生成唯一输入 ID。 */
  surface: 'table' | 'cards'
  /** 当前客户端模型标识。 */
  modelId: string
  /** 当前客户端模型展示名，用于无障碍名称。 */
  modelName: string
  /** 当前已选提供商模型名称。 */
  names: string[]
  /** 上游可用模型及当前草稿中的历史名称。 */
  options: UpstreamModel[]
  /** 是否正在当前行编辑提供商模型。 */
  editing: boolean
  /** 行内添加搜索框当前值。 */
  search: string
  /** 保存中或模型已隐藏时禁止编辑。 */
  disabled: boolean
}>()

const emit = defineEmits<{
  /** 进入当前行添加状态。 */
  start: []
  /** 退出添加状态并清空搜索。 */
  cancel: []
  /** 更新行内搜索词。 */
  'update-search': [value: string]
  /** 将指定提供商模型加入当前客户端模型。 */
  add: [name: string]
  /** 从当前客户端模型移除指定提供商模型。 */
  remove: [name: string]
  /** 将映射从原下标移动到目标下标，数组顺序即保存优先级。 */
  reorder: [fromIndex: number, toIndex: number]
}>()

const inputId = computed(() => `batch-mapping-add-${props.surface}-${props.modelId}`)
const listboxId = computed(() => `batch-mapping-options-${props.surface}-${props.modelId}`)
const normalizedSearch = computed(() => props.search.trim())
const selectedNames = computed(() => new Set(props.names))
const selectedIds = computed(() => new Set(props.options.map(option => option.id)))

// 当前拖动项下标；只用于批量行内排序的临时视觉状态。
const draggedIndex = ref<number | null>(null)

// 当前拖动经过的目标下标；用于提示可放置位置。
const dragOverIndex = ref<number | null>(null)

/** 搜索限制在尚未映射的名称内，并最多展示五个候选项。 */
const suggestions = computed(() => {
  const query = normalizedSearch.value.toLowerCase()
  return props.options
    .filter(option => !selectedNames.value.has(option.id))
    .filter(option => !query || option.id.toLowerCase().includes(query))
    .slice(0, 5)
})

/** 搜索词既不在上游选项中、也不在当前映射中时，允许作为自定义映射创建。 */
const canCreateCustom = computed(() =>
  Boolean(normalizedSearch.value)
  && !selectedIds.value.has(normalizedSearch.value)
  && !selectedNames.value.has(normalizedSearch.value))

/** 处理行内组合框的取消与首项确认，保持键盘操作不离开当前批量工作台。 */
function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    emit('cancel')
    return
  }
  if (event.key !== 'Enter') return
  const first = suggestions.value[0]
  if (first) {
    emit('add', first.id)
  } else if (canCreateCustom.value) {
    emit('add', normalizedSearch.value)
  }
}

/** 记录拖拽起点，并声明本次操作为移动而非复制。 */
function handleDragStart(index: number, event: DragEvent) {
  if (props.disabled) {
    event.preventDefault()
    return
  }
  draggedIndex.value = index
  dragOverIndex.value = null
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move'
    event.dataTransfer.setData('text/plain', props.names[index] ?? '')
  }
}

/** 高亮当前拖动经过的目标位置。 */
function handleDragOver(index: number) {
  if (draggedIndex.value === null || draggedIndex.value === index) return
  dragOverIndex.value = index
}

/** 离开当前目标位置后清除高亮。 */
function handleDragLeave(index: number) {
  if (dragOverIndex.value === index) {
    dragOverIndex.value = null
  }
}

/** 将拖拽项提交给父组件重排，并清理临时状态。 */
function handleDrop(targetIndex: number) {
  const sourceIndex = draggedIndex.value
  if (sourceIndex === null) return
  if (sourceIndex !== targetIndex) {
    emit('reorder', sourceIndex, targetIndex)
  }
  handleDragEnd()
}

/** 结束或取消拖拽时清理高亮和拖动状态。 */
function handleDragEnd() {
  draggedIndex.value = null
  dragOverIndex.value = null
}
</script>
