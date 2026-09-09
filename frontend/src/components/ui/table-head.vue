<script setup lang="ts">
import { cn } from '@/lib/utils'
import { computed } from 'vue'

interface Props {
  class?: string
}

const props = defineProps<Props>()

const headClass = computed(() =>
  cn(
    'h-11 px-2.5 py-2 text-left text-xs align-middle font-medium text-muted-foreground [&:has([role=checkbox])]:pr-0',
    props.class
  )
)
</script>

<template>
  <th :class="headClass">
    <slot />
  </th>
</template>
