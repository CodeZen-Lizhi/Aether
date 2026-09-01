<script setup lang="ts">
import { cva } from 'class-variance-authority'
import { cn } from '@/lib/utils'
import { computed } from 'vue'

const props = withDefaults(defineProps<Props>(), {
  variant: 'default',
  class: undefined,
})

// 软底描边药丸：由原状态徽章油猴脚本（Aether Providers Light V3）融入。
// primary/success=蓝（活跃/正常/启用）、secondary/outline=灰（停用/未配置）、
// warning=琥珀（待处理/限速）、destructive=红（异常/失败/过期）。
const badgeVariants = cva(
  'inline-flex items-center justify-center whitespace-nowrap rounded-full border px-2.5 py-0.5 text-xs font-semibold leading-none h-6 transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 shadow-none',
  {
    variants: {
      variant: {
        default:
          'border-[#bfdbfe] bg-[#f5f9ff] text-[#1d4ed8] dark:border-[#1e40af]/40 dark:bg-[#1e3a8a]/25 dark:text-[#93c5fd]',
        secondary:
          'border-[#e2e8f0] bg-[#f8fafc] text-[#475569] dark:border-[#334155]/60 dark:bg-[#1e293b]/70 dark:text-[#cbd5e1]',
        destructive:
          'border-[#fecaca] bg-[#fef2f2] text-[#dc2626] dark:border-[#7f1d1d]/50 dark:bg-[#7f1d1d]/25 dark:text-[#fca5a5]',
        outline: 'text-foreground border-border bg-card/50',
        'outline-transparent': 'text-foreground border-border bg-transparent',
        success:
          'border-[#bfdbfe] bg-[#f5f9ff] text-[#1d4ed8] dark:border-[#1e40af]/40 dark:bg-[#1e3a8a]/25 dark:text-[#93c5fd]',
        warning:
          'border-[#fde68a] bg-[#fffbeb] text-[#b45309] dark:border-[#78350f]/50 dark:bg-[#78350f]/25 dark:text-[#fcd34d]',
        dark: 'border-transparent bg-foreground text-background hover:bg-foreground/80',
      },
    },
    defaultVariants: {
      variant: 'default',
    },
  }
)

interface Props {
  variant?: 'default' | 'secondary' | 'destructive' | 'outline' | 'outline-transparent' | 'success' | 'warning' | 'dark'
  class?: string
}

const badgeClass = computed(() =>
  cn(badgeVariants({ variant: props.variant }), props.class)
)
</script>

<template>
  <div :class="badgeClass">
    <slot />
  </div>
</template>
