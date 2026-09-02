<template>
  <Card :class="cardClasses">
    <Collapsible
      v-if="collapsible"
      v-model:open="isOpen"
    >
      <div
        v-if="title || description || $slots.header"
        :class="headerClasses"
      >
        <div
          v-if="$slots.header"
          class="flex items-center justify-between gap-4"
        >
          <div class="min-w-0 flex-1">
            <slot name="header" />
          </div>
          <CollapsibleTrigger as-child>
            <button
              type="button"
              class="shrink-0 rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              :aria-label="`${isOpen ? '收起' : '展开'}${title || '此设置'}`"
              :title="`${isOpen ? '收起' : '展开'}${title || '此设置'}`"
            >
              <ChevronDown
                class="h-5 w-5 transition-transform duration-200"
                :class="isOpen ? 'rotate-180' : ''"
              />
            </button>
          </CollapsibleTrigger>
        </div>
        <div
          v-else
          class="flex items-center justify-between gap-4"
        >
          <CollapsibleTrigger as-child>
            <button
              type="button"
              class="group flex min-w-0 flex-1 items-start gap-3 rounded-md text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              :aria-label="`${isOpen ? '收起' : '展开'}${title || '此设置'}`"
            >
              <div class="min-w-0 flex-1">
                <h3
                  v-if="title"
                  class="text-lg font-medium leading-6 text-foreground"
                >
                  {{ title }}
                </h3>
                <p
                  v-if="description"
                  class="mt-1 text-sm text-muted-foreground"
                >
                  {{ description }}
                </p>
              </div>
              <ChevronDown
                class="mt-0.5 h-5 w-5 shrink-0 text-muted-foreground transition-transform duration-200 group-hover:text-foreground"
                :class="isOpen ? 'rotate-180' : ''"
              />
            </button>
          </CollapsibleTrigger>
          <div
            v-if="$slots.actions"
            class="shrink-0"
          >
            <slot name="actions" />
          </div>
        </div>
      </div>

      <CollapsibleContent>
        <div :class="contentClasses">
          <slot />
        </div>
      </CollapsibleContent>

      <div
        v-if="$slots.footer"
        :class="footerClasses"
      >
        <slot name="footer" />
      </div>
    </Collapsible>

    <template v-else>
      <div
        v-if="title || description || $slots.header"
        :class="headerClasses"
      >
        <slot name="header">
          <div class="flex items-center justify-between">
            <div>
              <h3
                v-if="title"
                class="text-lg font-medium leading-6 text-foreground"
              >
                {{ title }}
              </h3>
              <p
                v-if="description"
                class="mt-1 text-sm text-muted-foreground"
              >
                {{ description }}
              </p>
            </div>
            <div v-if="$slots.actions">
              <slot name="actions" />
            </div>
          </div>
        </slot>
      </div>

      <div :class="contentClasses">
        <slot />
      </div>

      <div
        v-if="$slots.footer"
        :class="footerClasses"
      >
        <slot name="footer" />
      </div>
    </template>
  </Card>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import Card from '@/components/ui/card.vue'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui'
import { ChevronDown } from 'lucide-vue-next'

interface Props {
  title?: string
  description?: string
  variant?: 'default' | 'elevated' | 'glass'
  padding?: 'none' | 'sm' | 'md' | 'lg'
  collapsible?: boolean
  defaultOpen?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  title: undefined,
  description: undefined,
  variant: 'default',
  padding: 'md',
  collapsible: false,
  defaultOpen: true,
})

const isOpen = ref(props.defaultOpen)

const cardClasses = computed(() => {
  const classes = []

  if (props.variant === 'elevated') {
    classes.push('shadow-md')
  } else if (props.variant === 'glass') {
    classes.push('surface-glass')
  }

  return classes.join(' ')
})

const headerClasses = computed(() => {
  const paddingMap = {
    none: '',
    sm: 'px-3 py-3',
    md: 'px-4 py-5 sm:p-6',
    lg: 'px-6 py-6 sm:p-8',
  }

  const classes = [paddingMap[props.padding]]

  if (props.padding !== 'none') {
    classes.push('border-b border-border')
  }

  return classes.join(' ')
})

const contentClasses = computed(() => {
  const paddingMap = {
    none: '',
    sm: 'px-3 py-3',
    md: 'px-4 py-5 sm:p-6',
    lg: 'px-6 py-6 sm:p-8',
  }

  return paddingMap[props.padding]
})

const footerClasses = computed(() => {
  const paddingMap = {
    none: '',
    sm: 'px-3 py-3',
    md: 'px-4 py-5 sm:p-6',
    lg: 'px-6 py-6 sm:p-8',
  }

  const classes = [paddingMap[props.padding]]

  if (props.padding !== 'none') {
    classes.push('border-t border-border')
  }

  return classes.join(' ')
})
</script>
