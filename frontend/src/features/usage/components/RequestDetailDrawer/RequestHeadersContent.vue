<template>
  <div>
    <!-- 对比模式 - 并排 Diff -->
    <div v-show="viewMode === 'compare'">
      <div
        v-if="!resolvedClientHeaders || !resolvedProviderHeaders"
        class="text-sm text-muted-foreground"
      >
        {{ emptyMessage }}
      </div>
      <Card
        v-else
        class="bg-muted/30 overflow-hidden"
      >
        <!-- Diff 头部 -->
        <div class="flex border-b bg-muted/50">
          <div class="flex-1 px-3 py-2 text-xs text-muted-foreground border-r flex items-center justify-between">
            <span class="font-medium">{{ clientLabel }}</span>
            <span class="text-destructive">-{{ headerStats.removed + headerStats.modified }}</span>
          </div>
          <div class="flex-1 px-3 py-2 text-xs text-muted-foreground flex items-center justify-between">
            <span class="font-medium">{{ providerLabel }}</span>
            <span class="text-green-600 dark:text-green-400">+{{ headerStats.added + headerStats.modified }}</span>
          </div>
        </div>

        <!-- Each header pair shares a grid row so wrapped values stay aligned. -->
        <div class="max-h-[500px] overflow-y-auto font-mono text-xs">
          <div
            v-for="entry in sortedEntries"
            :key="entry.key"
            class="header-diff-row grid grid-cols-2 [overflow-wrap:anywhere]"
          >
            <div
              class="min-w-0 border-r px-3 py-0.5"
              :class="{
                'bg-destructive/10 text-destructive': entry.status === 'removed',
                'bg-amber-500/10 text-amber-600 dark:text-amber-400': entry.status === 'modified',
                'bg-muted/30 text-muted-foreground/30 italic': entry.status === 'added',
                'text-muted-foreground hover:bg-muted/50': entry.status === 'unchanged',
              }"
            >
              <span v-if="entry.status === 'added'">（无）</span>
              <span v-else>"{{ entry.key }}": "{{ entry.clientValue }}"</span>
            </div>
            <div
              class="min-w-0 px-3 py-0.5"
              :class="{
                'bg-muted/30 text-muted-foreground/50 line-through': entry.status === 'removed',
                'bg-amber-500/10 text-amber-600 dark:text-amber-400': entry.status === 'modified',
                'bg-green-500/10 text-green-600 dark:text-green-400': entry.status === 'added',
                'text-muted-foreground hover:bg-muted/50': entry.status === 'unchanged',
              }"
            >
              <span>"{{ entry.key }}": "{{ entry.status === 'removed' ? entry.clientValue : entry.providerValue }}"</span>
            </div>
          </div>
        </div>
      </Card>
    </div>

    <!-- 格式化模式 - 直接使用 JsonContent -->
    <div v-show="viewMode === 'formatted'">
      <JsonContent
        :data="currentHeaderData"
        :view-mode="viewMode"
        :expand-depth="currentExpandDepth"
        :is-dark="isDark"
        empty-message="无请求头信息"
      />
    </div>

    <!-- 原始模式 -->
    <div v-show="viewMode === 'raw'">
      <div
        v-if="!currentHeaderData || Object.keys(currentHeaderData).length === 0"
        class="text-sm text-muted-foreground"
      >
        无请求头信息
      </div>
      <Card
        v-else
        class="bg-muted/30"
      >
        <div class="p-4">
          <pre class="text-xs font-mono whitespace-pre-wrap [overflow-wrap:anywhere]">{{ JSON.stringify(currentHeaderData, null, 2) }}</pre>
        </div>
      </Card>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import Card from '@/components/ui/card.vue'
import JsonContent from './JsonContent.vue'
import type { RequestDetail } from '@/api/dashboard'

const props = withDefaults(defineProps<{
  detail: RequestDetail
  viewMode: 'compare' | 'formatted' | 'raw'
  dataSource: 'client' | 'provider'
  currentHeaderData: Record<string, unknown> | null
  currentExpandDepth: number
  hasProviderHeaders: boolean
  headerStats: { added: number; modified: number; removed: number; unchanged: number }
  isDark: boolean
  // 泛化 props：允许传入任意 header 对和标签，用于复用为响应头对比
  clientHeaders?: Record<string, unknown>
  providerHeaders?: Record<string, unknown>
  clientLabel?: string
  providerLabel?: string
  emptyMessage?: string
}>(), {
  clientHeaders: undefined,
  providerHeaders: undefined,
  clientLabel: '客户端请求头',
  providerLabel: '提供商请求头',
  emptyMessage: '无请求头信息',
})

// 解析实际使用的 header 数据
const resolvedClientHeaders = computed(() =>
  props.clientHeaders ?? props.detail.request_headers ?? {}
)
const resolvedProviderHeaders = computed(() =>
  props.providerHeaders ?? props.detail.provider_request_headers ?? {}
)

// 合并并排序的条目（用于并排显示）
const sortedEntries = computed(() => {
  const clientHeaders = resolvedClientHeaders.value
  const providerHeaders = resolvedProviderHeaders.value

  const clientKeys = new Set(Object.keys(clientHeaders))
  const providerKeys = new Set(Object.keys(providerHeaders))
  const allKeys = Array.from(new Set([...clientKeys, ...providerKeys])).sort()

  return allKeys.map(key => {
    const inClient = clientKeys.has(key)
    const inProvider = providerKeys.has(key)
    const clientValue = clientHeaders[key]
    const providerValue = providerHeaders[key]

    let status: 'added' | 'removed' | 'modified' | 'unchanged'
    if (inClient && inProvider) {
      status = clientValue === providerValue ? 'unchanged' : 'modified'
    } else if (inClient) {
      status = 'removed'
    } else {
      status = 'added'
    }

    return {
      key,
      clientValue,
      providerValue,
      status
    }
  })
})
</script>
