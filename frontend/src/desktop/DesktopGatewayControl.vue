<script setup lang="ts">
import { computed } from 'vue'
import { AlertCircle, CheckCircle2, ChevronDown, LoaderCircle, Play, RotateCw, Square } from 'lucide-vue-next'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { useDesktopGateway } from './useDesktopGateway'
import type { GatewayPhase } from './bridge'

const gateway = useDesktopGateway()
const { phase, status, busy, canStart, canStop, errors } = gateway

const labels: Record<GatewayPhase, string> = {
  setup: '等待启动', starting: '正在启动', running: '运行中', stopping: '正在停止', stopped: '已停止', failed: '运行异常',
}
const label = computed(() => phase.value ? labels[phase.value] : '连接中')
const tone = computed(() => phase.value === 'running' ? 'text-emerald-600 dark:text-emerald-400'
  : phase.value === 'failed' ? 'text-destructive' : 'text-muted-foreground')

function start() { void gateway.start() }
function stop() { void gateway.stop() }
function restart() { void gateway.restart() }
</script>

<template>
  <DropdownMenu>
    <DropdownMenuTrigger as-child>
      <button
        type="button"
        class="group flex min-h-9 items-center gap-2 rounded-lg border border-border/70 bg-background/80 px-2.5 text-xs font-medium text-muted-foreground shadow-sm transition-colors hover:border-primary/40 hover:bg-muted/60 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
        :class="tone"
        :disabled="!status"
        aria-label="网关状态与操作"
        title="网关状态与操作"
      >
        <LoaderCircle
          v-if="busy || !status"
          class="h-3.5 w-3.5 animate-spin"
          aria-hidden="true"
        />
        <AlertCircle
          v-else-if="phase === 'failed'"
          class="h-3.5 w-3.5"
          aria-hidden="true"
        />
        <CheckCircle2
          v-else-if="phase === 'running'"
          class="h-3.5 w-3.5"
          aria-hidden="true"
        />
        <span class="hidden sm:inline">网关</span>
        <span>{{ label }}</span>
        <ChevronDown
          class="h-3.5 w-3.5 text-muted-foreground transition-transform group-data-[state=open]:rotate-180"
          aria-hidden="true"
        />
      </button>
    </DropdownMenuTrigger>
    <DropdownMenuContent
      align="end"
      class="w-52"
    >
      <div class="px-3 py-2">
        <p class="text-xs font-medium text-foreground">
          本机网关
        </p>
        <p class="mt-1 truncate text-[11px] text-muted-foreground">
          {{ status?.gateway_url || '正在读取状态…' }}
        </p>
      </div>
      <div class="my-1 h-px bg-border" />
      <DropdownMenuItem
        v-if="canStart"
        :disabled="busy"
        @select="start"
      >
        <Play
          class="mr-2 h-3.5 w-3.5"
          aria-hidden="true"
        />
        启动网关
      </DropdownMenuItem>
      <DropdownMenuItem
        v-if="phase === 'running'"
        :disabled="busy"
        @select="restart"
      >
        <RotateCw
          class="mr-2 h-3.5 w-3.5"
          aria-hidden="true"
        />
        重启网关
      </DropdownMenuItem>
      <DropdownMenuItem
        v-if="canStop"
        class="text-destructive data-[highlighted]:bg-destructive/10 data-[highlighted]:text-destructive"
        :disabled="busy"
        @select="stop"
      >
        <Square
          class="mr-2 h-3.5 w-3.5"
          aria-hidden="true"
        />
        停止网关
      </DropdownMenuItem>
      <p
        v-if="errors.length"
        class="mx-3 mb-2 mt-1 line-clamp-2 text-[11px] text-destructive"
        role="status"
      >
        {{ errors[0] }}
      </p>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
