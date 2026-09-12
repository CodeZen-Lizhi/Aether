<template>
  <section class="settings-group">
    <template v-if="view === 'backup'">
      <h2 class="settings-heading">
        数据备份
      </h2>
      <div class="settings-row">
        <div>
          <p class="settings-label">
            备份与恢复
          </p>
          <p class="settings-description">
            配置、管理员资料、API Keys 与用量统计。
          </p>
        </div>
        <div class="settings-actions settings-backup-actions">
          <Button
            :disabled="aggregateExportLoading"
            variant="outline"
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
      </div>
    </template>
    <div
      v-else
      class="settings-row"
    >
      <div>
        <p class="settings-label">
          导入与导出配置
        </p>
        <p class="settings-description">
          提供商、端点、渠道 Key、模型、代理节点、调度策略与系统配置
        </p>
      </div>
      <div class="settings-actions settings-backup-actions">
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
    <input
      v-if="view === 'migration'"
      ref="configFileInput"
      type="file"
      accept=".json"
      class="hidden"
      @change="$emit('fileSelect', 'config', $event)"
    >
    <input
      v-if="view === 'backup'"
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
import { Download, Upload } from 'lucide-vue-next'
import Button from '@/components/ui/button.vue'

defineProps<{
  view: 'backup' | 'migration'
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
