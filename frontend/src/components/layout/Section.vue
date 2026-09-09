<template>
  <section :class="sectionClasses">
    <div
      v-if="title || description || $slots.header"
      class="mb-6"
    >
      <slot name="header">
        <div class="flex min-w-0 flex-wrap items-center justify-between gap-3">
          <div class="min-w-0 [overflow-wrap:anywhere]">
            <h2
              v-if="title"
              class="text-lg font-medium text-foreground"
            >
              {{ title }}
            </h2>
            <p
              v-if="description"
              class="mt-1 text-sm text-muted-foreground"
            >
              {{ description }}
            </p>
          </div>
          <div
            v-if="$slots.actions"
            class="flex max-w-full flex-wrap items-center gap-2"
          >
            <slot name="actions" />
          </div>
        </div>
      </slot>
    </div>

    <slot />
  </section>
</template>

<script setup lang="ts">
import { computed } from 'vue'

interface Props {
  title?: string
  description?: string
  spacing?: 'none' | 'sm' | 'md' | 'lg'
}

const props = withDefaults(defineProps<Props>(), {
  title: undefined,
  description: undefined,
  spacing: 'md',
})

const sectionClasses = computed(() => {
  const classes = []

  const spacingMap = {
    none: '',
    sm: 'mb-4',
    md: 'mb-6',
    lg: 'mb-8',
  }

  if (props.spacing !== 'none') {
    classes.push(spacingMap[props.spacing])
  }

  return classes.join(' ')
})
</script>
