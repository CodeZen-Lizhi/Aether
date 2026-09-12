<template>
  <section class="settings-group">
    <h3 class="settings-heading">
      完整备份
    </h3>
    <p class="settings-description mt-4">
      配置、管理员资料与偏好、API Keys 及用量统计的一体化备份
    </p>
    <div class="settings-actions settings-backup-actions">
      <Button
        :disabled="aggregateExportLoading"
        size="sm"
        @click="$emit('export', 'aggregate')"
      >
        <Download class="h-4 w-4" />
        {{ aggregateExportLoading ? '导出中...' : '创建备份' }}
      </Button>
      <Button
        variant="outline"
        size="sm"
        :disabled="aggregateImportLoading"
        @click="aggregateFileInput?.click()"
      >
        <Upload class="h-4 w-4" />
        {{ aggregateImportLoading ? '导入中...' : '恢复备份' }}
      </Button>
    </div>
    <details class="settings-disclosure">
      <summary>配置迁移<ChevronDown class="settings-chevron" /></summary>
      <div class="settings-disclosure-content">
        <p class="settings-description">
          提供商、端点、渠道 Key、模型、代理节点、调度策略与系统配置
        </p>
        <div class="settings-actions mt-4">
          <Button
            variant="outline"
            size="sm"
            :disabled="configExportLoading"
            @click="$emit('export', 'config')"
          >
            <Download class="h-4 w-4" />
            {{ configExportLoading ? '导出中...' : '导出配置' }}
          </Button>
          <Button
            variant="outline"
            size="sm"
            :disabled="configImportLoading"
            @click="configFileInput?.click()"
          >
            <Upload class="h-4 w-4" />
            {{ configImportLoading ? '导入中...' : '导入配置' }}
          </Button>
        </div>
      </div>
    </details>
    <input
      ref="configFileInput"
      type="file"
      accept=".json"
      class="hidden"
      @change="$emit('fileSelect', 'config', $event)"
    >
    <input
      ref="aggregateFileInput"
      type="file"
      accept=".json"
      class="hidden"
      @change="$emit('fileSelect', 'aggregate', $event)"
    >
  </section>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { ChevronDown, Download, Upload } from 'lucide-vue-next'
import Button from '@/components/ui/button.vue'

defineProps<{
  configExportLoading: boolean
  configImportLoading: boolean
  aggregateExportLoading: boolean
  aggregateImportLoading: boolean
}>()
defineEmits<{
  export: [key: 'config' | 'aggregate']
  fileSelect: [key: 'config' | 'aggregate', event: Event]
}>()

const configFileInput = ref<HTMLInputElement | null>(null)
const aggregateFileInput = ref<HTMLInputElement | null>(null)
</script>
