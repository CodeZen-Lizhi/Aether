<template>
  <details
    class="settings-disclosure settings-group"
    @toggle="toggleMaintenance"
  >
    <summary>数据维护<ChevronDown class="settings-chevron" /></summary>
    <div class="settings-disclosure-content">
      <CleanupMaintenanceSection
        v-if="visited"
        :active="active && expanded"
      />
      <h3 class="settings-heading mt-6">
        清空数据
      </h3>
      <div
        v-for="item in purgeItems"
        :key="item.key"
        class="settings-row"
      >
        <div>
          <h4 class="settings-label">
            {{ item.title }}
          </h4>
          <p class="settings-description">
            {{ item.description }}
          </p>
        </div>
        <div class="settings-actions">
          <Button
            variant="outline"
            size="sm"
            class="text-destructive"
            :disabled="loadingKey !== null"
            @click="handlePurge(item)"
          >
            <Trash2 class="h-4 w-4" />
            {{ loadingKey === item.key ? '清空中...' : item.buttonText }}
          </Button>
        </div>
      </div>
    </div>
  </details>
</template>

<script setup lang="ts">
import { ref, markRaw, type Component } from 'vue'
import { ChevronDown, Settings, Trash2, BarChart3, Shield, FileText, PieChart } from 'lucide-vue-next'
import Button from '@/components/ui/button.vue'
import { adminApi } from '@/api/admin'
import { useToast } from '@/composables/useToast'
import { useConfirm } from '@/composables/useConfirm'
import { parseApiError } from '@/utils/errorParser'
import CleanupMaintenanceSection from './CleanupMaintenanceSection.vue'

defineProps<{ active: boolean }>()
const expanded = ref(false)
const visited = ref(false)
const { success, error } = useToast()
const { confirmDanger } = useConfirm()
const loadingKey = ref<string | null>(null)

interface PurgeItem {
  key: string
  title: string
  description: string
  buttonText: string
  icon: Component
  confirmMessage: string
  action: () => Promise<{ message: string }>
}

function toggleMaintenance(event: Event) {
  expanded.value = (event.currentTarget as HTMLElement).hasAttribute('open')
  if (expanded.value) visited.value = true
}

const purgeItems: PurgeItem[] = [
  {
    key: 'config',
    title: '清空配置',
    description: '后台删除所有提供商、端点、API Key 和模型配置',
    buttonText: '清空配置',
    icon: markRaw(Settings),
    confirmMessage: '确定要后台清空所有提供商配置吗？这将删除所有提供商、端点、API Key 和模型配置，操作不可逆。',
    action: () => adminApi.purgeConfig(),
  },
  {
    key: 'usage',
    title: '清空使用记录',
    description: '后台清空全部使用记录和请求候选记录',
    buttonText: '清空记录',
    icon: markRaw(BarChart3),
    confirmMessage: '确定要后台清空全部使用记录吗？所有请求统计数据将被永久删除，操作不可逆。',
    action: () => adminApi.purgeUsage(),
  },
  {
    key: 'audit-logs',
    title: '清空审计日志',
    description: '后台删除全部审计日志记录',
    buttonText: '清空日志',
    icon: markRaw(Shield),
    confirmMessage: '确定要后台清空全部审计日志吗？所有安全事件记录将被永久删除，操作不可逆。',
    action: () => adminApi.purgeAuditLogs(),
  },
  {
    key: 'request-bodies',
    title: '清空请求体',
    description: '后台分批清空所有请求/响应体数据，保留统计信息',
    buttonText: '清空请求体',
    icon: markRaw(FileText),
    confirmMessage: '确定要后台清空全部请求体吗？请求/响应内容将被分批清除，但 token 和成本等统计信息会保留，操作不可逆。',
    action: () => adminApi.purgeRequestBodiesAsync(),
  },
  {
    key: 'stats',
    title: '清空统计聚合',
    description: '删除统计聚合后仅能从剩余使用记录重建，已清理的历史统计可能丢失',
    buttonText: '清空统计聚合',
    icon: markRaw(PieChart),
    confirmMessage: '确定要后台清空全部统计聚合数据吗？原始使用记录会保留，但仅能从剩余记录重建，已清理记录对应的历史统计可能永久丢失。',
    action: () => adminApi.purgeStats(),
  },
]

async function handlePurge(item: PurgeItem) {
  if (loadingKey.value) return
  const confirmed = await confirmDanger(item.confirmMessage, item.title)
  if (!confirmed) return

  loadingKey.value = item.key
  try {
    const result = await item.action()
    success(result.message || '操作成功')
  } catch (e) {
    error(parseApiError(e, '清空失败'))
  } finally {
    loadingKey.value = null
  }
}
</script>
