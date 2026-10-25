<template>
  <div class="flex min-w-0 flex-wrap items-center gap-1.5">
    <Badge
      v-if="hidden"
      variant="secondary"
      class="text-[11px]"
    >
      已隐藏
    </Badge>
    <Badge
      v-else-if="mappedCount === 0"
      variant="secondary"
      class="text-[11px]"
    >
      未映射
    </Badge>
    <Badge
      v-else
      :variant="changed ? 'warning' : 'default'"
      class="text-[11px]"
    >
      <span
        v-if="changed"
        class="mr-1 inline-block h-1.5 w-1.5 rounded-full bg-current"
        aria-hidden="true"
      />
      已映射 {{ mappedCount }}
    </Badge>
  </div>
</template>

<script setup lang="ts">
import { Badge } from '@/components/ui'

/** 批量映射草稿的映射状态、修改状态与工作台隐藏状态。 */
defineProps<{
  /** 当前草稿中提供商模型名称数量。 */
  mappedCount: number
  /** 草稿是否不同于已保存状态。 */
  changed: boolean
  /** 是否仅在当前工作台隐藏。 */
  hidden: boolean
}>()
</script>
