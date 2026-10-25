<template>
  <!-- 批量映射行内编辑器；候选浮层通过 Portal 避免被可滚动表格裁切。 -->
  <div class="min-w-0">
    <div
      v-if="names.length > 0 || !editing"
      class="flex min-w-0 flex-wrap items-center gap-1.5"
    >
      <span
        v-for="name in names"
        :key="name"
        class="inline-flex max-w-full min-w-0 items-center gap-1 rounded-lg border border-[#bfdbfe] bg-[#f5f9ff] py-1 pl-2 pr-1 font-mono text-[11px] text-[#1d4ed8] dark:border-[#1e40af]/40 dark:bg-[#1e3a8a]/25 dark:text-[#93c5fd]"
      >
        <span
          class="min-w-0 truncate"
          :title="name"
        >
          {{ name }}
        </span>
        <button
          type="button"
          class="shrink-0 rounded p-0.5 text-primary/60 transition-colors hover:bg-primary/10 hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:pointer-events-none disabled:opacity-50"
          :aria-label="`移除 ${name}`"
          :title="`移除 ${name}`"
          :disabled="disabled"
          @click="$emit('remove', name)"
        >
          <X class="h-3 w-3" />
        </button>
      </span>
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
import { computed } from 'vue'
import { Plus, Search, X } from 'lucide-vue-next'
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
}>()

const inputId = computed(() => `batch-mapping-add-${props.surface}-${props.modelId}`)
const listboxId = computed(() => `batch-mapping-options-${props.surface}-${props.modelId}`)
const normalizedSearch = computed(() => props.search.trim())
const selectedNames = computed(() => new Set(props.names))
const selectedIds = computed(() => new Set(props.options.map(option => option.id)))

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
</script>
