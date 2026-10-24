<template>
  <details
    class="group min-w-0 overflow-hidden rounded-xl border border-border/60 bg-card/60"
    data-request-detail-disclosure
  >
    <summary class="flex min-h-11 cursor-pointer list-none items-center justify-between gap-3 px-3 py-2.5 text-xs text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring sm:px-4 [&::-webkit-details-marker]:hidden">
      <span class="flex min-w-0 flex-1 flex-wrap items-baseline gap-x-2 gap-y-0.5">
        <span class="font-medium text-foreground">{{ title }}</span>
        <span
          v-if="description"
          class="min-w-0 text-muted-foreground/80"
        >{{ description }}</span>
      </span>
      <ChevronDown
        class="h-4 w-4 shrink-0 transition-transform duration-150 group-open:rotate-180 motion-reduce:transition-none"
        aria-hidden="true"
      />
    </summary>
    <div class="min-w-0 border-t border-border/60 px-3 py-3 sm:px-4">
      <slot />
    </div>
  </details>
</template>

<script setup lang="ts">
import { ChevronDown } from 'lucide-vue-next'

// 请求详情中的统一折叠容器，只负责标题、展开状态和承载区块内容。
interface Props {
  /** 折叠区主标题。 */
  title: string
  /** 补充说明该折叠区包含的可展开内容；省略时只显示模块标题。 */
  description?: string
}

defineProps<Props>()
</script>
