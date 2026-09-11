<script setup lang="ts">
import { computed } from 'vue'
import {
  AlertCircle, CheckCircle2, ChevronDown, CircleDashed, LoaderCircle, Play, RefreshCw, RotateCw, Square,
} from 'lucide-vue-next'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { useDesktopGateway } from './useDesktopGateway'
import type { GatewayPhase } from './bridge'

const gateway = useDesktopGateway()
const { phase, status, loading, pendingAction, connectionError, canStart, canStop, errors } = gateway

const labels: Record<GatewayPhase, string> = {
  setup: '等待启动', starting: '正在启动', running: '运行中', stopping: '正在停止', stopped: '已停止', failed: '运行异常',
}
const label = computed(() => {
  if (phase.value) return labels[phase.value]
  if (connectionError.value) return '连接失败'
  return loading.value ? '正在连接' : '状态不可用'
})
const actionPending = computed(() => !!pendingAction.value)
const visualState = computed(() => {
  if (phase.value === 'failed' || connectionError.value) return 'failed'
  if (actionPending.value || loading.value || phase.value === 'starting' || phase.value === 'stopping') return 'progress'
  if (phase.value === 'running') return 'running'
  return 'stopped'
})
const stateClasses = computed(() => ({
  running: 'border-emerald-500/35 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300',
  progress: 'border-sky-500/35 bg-sky-500/10 text-sky-700 dark:text-sky-300',
  stopped: 'border-border/80 bg-muted/70 text-muted-foreground',
  failed: 'border-destructive/40 bg-destructive/10 text-destructive',
}[visualState.value]))
const showRecoveryActions = computed(() => !status.value || phase.value === 'starting' || phase.value === 'stopping')
const showRestart = computed(() => showRecoveryActions.value || phase.value === 'running' || phase.value === 'failed')
const showStop = computed(() => showRecoveryActions.value || canStop.value || status.value?.pid != null)
const stopDisabled = computed(() => !!pendingAction.value
  && pendingAction.value !== 'start' && pendingAction.value !== 'restart')

function refreshStatus() { void gateway.refreshStatus() }
function start() { void gateway.start() }
function stop() { void gateway.stop() }
function restart() { void gateway.restart() }
</script>

<template>
  <DropdownMenu>
    <DropdownMenuTrigger as-child>
      <button
        type="button"
        class="group flex min-h-9 items-center gap-2 rounded-lg border px-2.5 text-xs font-medium shadow-sm transition-colors hover:brightness-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary dark:hover:brightness-110"
        :class="stateClasses"
        :data-gateway-state="visualState"
        :aria-busy="loading || actionPending"
        aria-label="网关状态与操作"
        title="网关状态与操作"
      >
        <span
          class="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-black/5 dark:bg-white/10"
          aria-hidden="true"
        >
          <LoaderCircle
            v-if="actionPending"
            class="h-3.5 w-3.5 animate-spin motion-reduce:animate-none"
          />
          <AlertCircle
            v-else-if="visualState === 'failed'"
            class="h-3.5 w-3.5"
          />
          <CheckCircle2
            v-else-if="visualState === 'running'"
            class="h-3.5 w-3.5"
          />
          <CircleDashed
            v-else-if="visualState === 'progress'"
            class="h-3.5 w-3.5"
          />
          <Square
            v-else
            class="h-3 w-3"
          />
        </span>
        <span class="hidden sm:inline">网关</span>
        <span class="whitespace-nowrap">{{ label }}</span>
        <ChevronDown
          class="h-3.5 w-3.5 opacity-70 transition-transform group-data-[state=open]:rotate-180"
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
          {{ status?.gateway_url || (connectionError ? '状态读取失败' : '正在读取状态…') }}
        </p>
      </div>
      <div class="my-1 h-px bg-border" />
      <DropdownMenuItem
        v-if="!status || connectionError"
        :disabled="loading || actionPending"
        @select="refreshStatus"
      >
        <RefreshCw
          class="mr-2 h-3.5 w-3.5"
          aria-hidden="true"
        />
        重新读取状态
      </DropdownMenuItem>
      <DropdownMenuItem
        v-if="canStart"
        :disabled="actionPending"
        @select="start"
      >
        <Play
          class="mr-2 h-3.5 w-3.5"
          aria-hidden="true"
        />
        启动网关
      </DropdownMenuItem>
      <DropdownMenuItem
        v-if="showRestart"
        :disabled="actionPending"
        @select="restart"
      >
        <RotateCw
          class="mr-2 h-3.5 w-3.5"
          aria-hidden="true"
        />
        重启网关
      </DropdownMenuItem>
      <DropdownMenuItem
        v-if="showStop"
        class="text-destructive data-[highlighted]:bg-destructive/10 data-[highlighted]:text-destructive"
        :disabled="stopDisabled"
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
