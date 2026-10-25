<script setup lang="ts">
import { PopoverAnchor } from 'radix-vue'

/** 候选浮层锚点：允许输入框独立定位，不要求同时存在点击触发器。 */
withDefaults(defineProps<{
  /** 是否把锚点能力合并到子元素，避免额外节点影响表格布局。 */
  asChild?: boolean
}>(), {
  asChild: false,
})
</script>

<template>
  <PopoverAnchor :as-child="asChild">
    <slot />
  </PopoverAnchor>
</template>
