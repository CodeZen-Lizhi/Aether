<script setup lang="ts">
import { cn } from '@/lib/utils'
import { computed } from 'vue'

interface Props {
  class?: string
}

const props = defineProps<Props>()

const tableClass = computed(() =>
  cn('w-full table-fixed caption-bottom text-sm [overflow-wrap:anywhere]', props.class)
)
</script>

<template>
  <div class="relative w-full min-w-0">
    <table :class="tableClass">
      <slot />
    </table>
  </div>
</template>
