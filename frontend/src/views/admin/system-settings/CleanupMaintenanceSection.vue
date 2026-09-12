<template>
  <section>
    <div class="settings-row">
      <div>
        <h3 class="settings-label">
          按策略清理
        </h3>
        <p class="settings-description">
          使用已保存的保留策略清理过期数据。
        </p>
      </div>
      <div class="settings-actions">
        <Button
          variant="outline"
          size="sm"
          :disabled="manualCleanupRunning"
          @click="openManualCleanupDialog"
        >
          <Trash2 class="h-4 w-4" />
          {{ manualCleanupRunning ? '清理中…' : '立即清理' }}
        </Button>
      </div>
    </div>
    <ManualCleanupConfirmDialog
      :open="manualCleanupDialogOpen"
      @update:open="manualCleanupDialogOpen = $event"
      @running-change="manualCleanupRunning = $event"
      @completed="handleManualCleanupCompleted"
    />
    <p
      v-if="manualCleanupResult"
      class="settings-description"
      role="status"
    >
      {{ manualCleanupResult.title }} {{ manualCleanupResult.description }}
    </p>
    <details class="settings-disclosure">
      <summary>最近清理记录<ChevronDown class="settings-chevron" /></summary>
      <div class="settings-toolbar">
        <p class="settings-description">
          自动清理、手动系统清理和请求体后台任务的执行结果
        </p>
        <Button
          variant="ghost"
          size="icon"
          class="settings-icon-button"
          aria-label="刷新清理记录"
          title="刷新清理记录"
          :disabled="cleanupRunsLoading"
          @click="loadCleanupRuns"
        >
          <RefreshCw
            class="h-4 w-4"
            :class="{ 'animate-spin': cleanupRunsLoading }"
          />
        </Button>
      </div>
      <p
        v-if="cleanupRunsError"
        class="settings-error"
        role="alert"
      >
        {{ cleanupRunsError }}
      </p>
      <p
        v-else-if="!cleanupRuns.length"
        class="settings-empty"
        role="status"
      >
        {{ cleanupRunsLoading ? '加载中...' : '暂无清理记录' }}
      </p>
      <ol
        v-else
        class="divide-y divide-border"
      >
        <li
          v-for="run in cleanupRuns"
          :key="run.id"
          class="py-3 text-xs [overflow-wrap:anywhere]"
        >
          <div class="flex flex-wrap items-center gap-x-3 gap-y-1">
            <span class="font-medium">{{ cleanupKindLabel(run.kind) }}</span>
            <span :class="cleanupStatusClass(run.status)">{{ cleanupStatusLabel(run.status) }}</span>
            <span class="text-muted-foreground">{{ run.trigger === 'manual' ? '手动' : '自动' }}</span>
            <span class="text-muted-foreground">{{ formatRunTime(run.started_at_unix_secs) }}</span>
            <span class="text-muted-foreground">{{ formatDuration(run.duration_ms) }}</span>
          </div>
          <p class="mt-2">
            {{ run.error || run.message }}
          </p>
          <p class="settings-description">
            {{ cleanupSummaryText(run.summary) }}
          </p>
        </li>
      </ol>
    </details>
  </section>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import { ChevronDown, RefreshCw, Trash2 } from 'lucide-vue-next'
import { adminApi, type CleanupRunRecord } from '@/api/admin'
import Button from '@/components/ui/button.vue'
import ManualCleanupConfirmDialog from './ManualCleanupConfirmDialog.vue'
import { useToast } from '@/composables/useToast'

const props = defineProps<{ active: boolean }>()
const cleanupRuns = ref<CleanupRunRecord[]>([])
const cleanupRunsLoading = ref(false)
const cleanupRunsError = ref('')
let cleanupRunsTimer: number | null = null

const manualCleanupDialogOpen = ref(false)
const manualCleanupRunning = ref(false)
const manualCleanupResult = ref<{ title: string; description?: string } | null>(null)
const toast = useToast()

function openManualCleanupDialog() {
  if (manualCleanupRunning.value) return
  manualCleanupDialogOpen.value = true
}

function handleManualCleanupCompleted(task: CleanupRunRecord) {
  manualCleanupRunning.value = false
  manualCleanupResult.value = {
    title: task.message,
    description: cleanupSummaryText(task.summary),
  }
  if (task.status === 'failed') {
    toast.error(task.error || task.message)
  } else {
    toast.success(task.message)
  }
  void loadCleanupRuns()
}

async function loadCleanupRuns() {
  if (cleanupRunsLoading.value) return
  cleanupRunsLoading.value = true
  cleanupRunsError.value = ''
  try {
    const response = await adminApi.getCleanupRuns()
    cleanupRuns.value = response.items.slice(0, 10)
  } catch {
    cleanupRunsError.value = '加载清理记录失败'
  } finally {
    cleanupRunsLoading.value = false
  }
}

function cleanupKindLabel(kind: string): string {
  const labels: Record<string, string> = {
    usage_cleanup: '请求记录',
    audit_cleanup: '审计日志',
    request_candidate_cleanup: '候选记录',
    request_bodies: '请求体',
    config_purge: '配置清空',
    users_purge: '用户清空',
    usage_purge: '使用记录清空',
    audit_logs_purge: '审计日志清空',
    stats_purge: '统计聚合清空',
    system_cleanup: '系统清理',
  }
  return labels[kind] || kind
}

function cleanupStatusLabel(status: string): string {
  if (status === 'processing') return '执行中'
  if (status === 'failed') return '失败'
  return '完成'
}

function cleanupStatusClass(status: string): string {
  if (status === 'processing') return 'text-amber-500'
  if (status === 'failed') return 'text-destructive'
  return 'text-emerald-500'
}

function formatRunTime(value: number): string {
  if (!value) return '-'
  return new Date(value * 1000).toLocaleString()
}

function formatDuration(value: number | null): string {
  if (value === null || value === undefined) return '-'
  if (value < 1000) return `${value}ms`
  return `${(value / 1000).toFixed(1)}s`
}

function cleanupSummaryText(summary: Record<string, unknown>): string {
  const total = typeof summary.total === 'number' ? summary.total : null
  if (total !== null) return `影响 ${total} 行`

  const entries = Object.entries(summary)
    .filter(([, value]) => typeof value === 'number' && value > 0)
    .map(([key, value]) => `${summaryLabel(key)} ${value}`)
  return entries.length > 0 ? entries.join(' / ') : '无数据变更'
}

function summaryLabel(key: string): string {
  const labels: Record<string, string> = {
    body_externalized: '详细体',
    legacy_body_refs_migrated: '迁移',
    body_cleaned: '清体',
    header_cleaned: '清头',
    keys_cleaned: 'Key',
    records_deleted: '删记录',
    audit_logs_deleted: '删日志',
    request_candidates_deleted: '删候选',
  }
  return labels[key] || key
}

watch(() => props.active, active => {
  if (cleanupRunsTimer) window.clearInterval(cleanupRunsTimer)
  cleanupRunsTimer = null
  if (active) {
    void loadCleanupRuns()
    cleanupRunsTimer = window.setInterval(() => { void loadCleanupRuns() }, 15_000)
  }
}, { immediate: true })

onBeforeUnmount(() => {
  if (cleanupRunsTimer) {
    window.clearInterval(cleanupRunsTimer)
    cleanupRunsTimer = null
  }
})
</script>
