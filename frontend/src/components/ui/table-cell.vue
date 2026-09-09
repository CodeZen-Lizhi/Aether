<script setup lang="ts">
import { cn } from '@/lib/utils'
import { computed } from 'vue'

interface Props {
  class?: string
}

const props = defineProps<Props>()

const cellClass = computed(() =>
  cn('px-2.5 py-3 align-middle [&:has([role=checkbox])]:pr-0', props.class)
)
</script>

<template>
  <td :class="cellClass">
    <slot />
  </td>
</template>
