<template>
  <PageContainer>
    <div class="system-settings">
      <PageHeader title="系统设置" />
      <div class="settings-layout">
        <nav
          class="settings-nav"
          aria-label="设置分类"
        >
          <RouterLink
            v-for="item in tabs"
            :key="item.id"
            :to="{ query: { ...route.query, tab: item.id }, hash: '' }"
            :aria-current="activeTab === item.id ? 'page' : undefined"
          >
            <component
              :is="item.icon"
              class="h-4 w-4 shrink-0"
              aria-hidden="true"
            />
            {{ item.label }}
          </RouterLink>
          <p
            v-if="systemVersion"
            class="settings-nav-version"
          >
            Aether <samp>{{ systemVersion }}</samp>
          </p>
        </nav>
        <div class="settings-mobile-nav">
          <Select
            :model-value="activeTab"
            @update:model-value="selectTab"
          >
            <SelectTrigger aria-label="设置分类">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="item in tabs"
                :key="item.id"
                :value="item.id"
              >
                {{ item.label }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="settings-content">
          <div
            v-if="systemConfigLoading"
            class="settings-empty"
            role="status"
          >
            系统配置加载中...
          </div>
          <div
            v-else-if="systemConfigError"
            class="settings-toolbar"
            role="alert"
          >
            <p class="settings-error">
              {{ systemConfigError }}
            </p>
            <Button
              variant="outline"
              size="sm"
              @click="loadSystemConfig"
            >
              重试
            </Button>
          </div>
          <div
            :inert="systemConfigLoading || !!systemConfigError"
            :aria-busy="systemConfigLoading"
          >
            <section
              v-if="visited.has('connection')"
              v-show="activeTab === 'connection'"
              class="settings-panel"
              aria-labelledby="connection-title"
            >
              <h2
                id="connection-title"
                class="settings-panel-title"
              >
                {{ desktopMode ? '连接与启动' : '网络连接' }}
              </h2>
              <ProxyConfigSection
                id="section-proxy"
                :proxy-node-id="systemConfig.system_proxy_node_id"
                :loading="proxyConfigLoading"
                :has-changes="hasProxyConfigChanges"
                :error="saveErrors.proxy"
                @save="saveProxyConfig"
                @cancel="cancelChanges('proxy')"
                @proxy-cleared="confirmProxyCleared"
                @update:proxy-node-id="systemConfig.system_proxy_node_id = $event"
              />
              <DesktopGatewaySection
                v-if="desktopGateway"
                id="section-desktop-gateway"
                :gateway="desktopGateway"
                view="connection"
                :port-expanded="route.hash === '#section-desktop-gateway'"
              />
            </section>

            <section
              v-if="visited.has('records')"
              v-show="activeTab === 'records'"
              class="settings-panel"
              aria-labelledby="records-title"
            >
              <h2
                id="records-title"
                class="settings-panel-title"
              >
                记录与存储
              </h2>
              <RequestLogSection
                id="section-request-log"
                :request-record-level="systemConfig.request_record_level"
                :sensitive-headers-str="sensitiveHeadersStr"
                :loading="logConfigLoading"
                :has-changes="hasLogConfigChanges"
                :error="saveErrors.log"
                @save="saveLogConfig"
                @cancel="cancelChanges('log')"
                @update:request-record-level="systemConfig.request_record_level = $event"
                @update:sensitive-headers-str="sensitiveHeadersStr = $event"
              />
              <CleanupPolicySection
                id="section-cleanup"
                :enable-auto-cleanup="systemConfig.enable_auto_cleanup"
                :auto-cleanup-loading="autoCleanupLoading"
                :detail-log-retention-days="systemConfig.detail_log_retention_days"
                :compressed-log-retention-days="systemConfig.compressed_log_retention_days"
                :header-retention-days="systemConfig.header_retention_days"
                :log-retention-days="systemConfig.log_retention_days"
                :audit-log-retention-days="systemConfig.audit_log_retention_days"
                :request-candidates-retention-days="systemConfig.request_candidates_retention_days"
                :proxy-node-metrics-1m-retention-days="systemConfig.proxy_node_metrics_1m_retention_days"
                :proxy-node-metrics-1h-retention-days="systemConfig.proxy_node_metrics_1h_retention_days"
                :loading="cleanupConfigLoading"
                :has-changes="hasCleanupConfigChanges"
                :error="saveErrors.cleanup"
                @save="saveCleanupConfig"
                @cancel="cancelChanges('cleanup')"
                @toggle-auto-cleanup="handleAutoCleanupToggle"
                @update:detail-log-retention-days="systemConfig.detail_log_retention_days = $event"
                @update:compressed-log-retention-days="systemConfig.compressed_log_retention_days = $event"
                @update:header-retention-days="systemConfig.header_retention_days = $event"
                @update:log-retention-days="systemConfig.log_retention_days = $event"
                @update:audit-log-retention-days="systemConfig.audit_log_retention_days = $event"
                @update:request-candidates-retention-days="systemConfig.request_candidates_retention_days = $event"
                @update:proxy-node-metrics-1m-retention-days="systemConfig.proxy_node_metrics_1m_retention_days = $event"
                @update:proxy-node-metrics-1h-retention-days="systemConfig.proxy_node_metrics_1h_retention_days = $event"
              />
            </section>

            <section
              v-if="visited.has('backup')"
              v-show="activeTab === 'backup'"
              class="settings-panel"
              aria-labelledby="backup-title"
            >
              <h2
                id="backup-title"
                class="settings-panel-title"
              >
                备份与恢复
              </h2>
              <DataManagementSection
                id="section-data-mgmt"
                :config-export-loading="exportLoading"
                :config-import-loading="importLoading"
                :aggregate-export-loading="exportAggregateLoading"
                :aggregate-import-loading="importAggregateLoading"
                @export="handleDataExport"
                @file-select="handleDataFileSelect"
              />
            </section>

            <section
              v-if="visited.has('advanced')"
              v-show="activeTab === 'advanced'"
              class="settings-panel"
              aria-labelledby="advanced-title"
            >
              <h2
                id="advanced-title"
                class="settings-panel-title"
              >
                高级与诊断
              </h2>
              <BasicConfigSection
                id="section-basic"
                :rate-limit-per-minute="systemConfig.rate_limit_per_minute"
                :auto-delete-expired-keys="systemConfig.auto_delete_expired_keys"
                :enable-format-conversion="systemConfig.enable_format_conversion"
                :enable-openai-image-sync-heartbeat="systemConfig.enable_openai_image_sync_heartbeat"
                :enable-standard-text-sync-heartbeat="systemConfig.enable_standard_text_sync_heartbeat"
                :cyber-continue-failover="systemConfig.cyber_continue_failover"
                :loading="basicConfigLoading"
                :has-changes="hasBasicConfigChanges"
                :error="saveErrors.basic"
                @save="saveBasicConfig"
                @cancel="cancelChanges('basic')"
                @update:rate-limit-per-minute="systemConfig.rate_limit_per_minute = $event"
                @update:auto-delete-expired-keys="systemConfig.auto_delete_expired_keys = $event"
                @update:enable-format-conversion="systemConfig.enable_format_conversion = $event"
                @update:enable-openai-image-sync-heartbeat="systemConfig.enable_openai_image_sync_heartbeat = $event"
                @update:enable-standard-text-sync-heartbeat="systemConfig.enable_standard_text_sync_heartbeat = $event"
                @update:cyber-continue-failover="systemConfig.cyber_continue_failover = $event"
              />
              <SystemInfoSection
                id="section-sysinfo"
                :system-version="systemVersion"
              />
              <DesktopGatewaySection
                v-if="desktopGateway"
                :gateway="desktopGateway"
                view="diagnostics"
              />
              <DataMaintenanceSection :active="activeTab === 'advanced'" />
            </section>
          </div>
        </div>
      </div>
    </div>

    <!-- 导入配置对话框 -->
    <ConfigImportDialog
      :import-dialog-open="importDialogOpen"
      :import-result-dialog-open="importResultDialogOpen"
      :import-preview="importPreview"
      :import-result="importResult"
      :merge-mode="mergeMode"
      :merge-mode-select-open="mergeModeSelectOpen"
      :import-loading="importLoading"
      :import-progress="importProgress"
      @confirm="confirmImport"
      @update:import-dialog-open="importDialogOpen = $event"
      @update:import-result-dialog-open="importResultDialogOpen = $event"
      @update:merge-mode="mergeMode = $event"
      @update:merge-mode-select-open="mergeModeSelectOpen = $event"
    />

    <!-- 完整备份导入对话框 -->
    <AggregateImportDialog
      :aggregate-import-dialog-open="aggregateImportDialogOpen"
      :aggregate-import-result-dialog-open="aggregateImportResultDialogOpen"
      :aggregate-import-preview="aggregateImportPreview"
      :aggregate-import-result="aggregateImportResult"
      :aggregate-merge-mode="aggregateMergeMode"
      :aggregate-merge-mode-select-open="aggregateMergeModeSelectOpen"
      :import-aggregate-loading="importAggregateLoading"
      :import-aggregate-progress="importAggregateProgress"
      @confirm="confirmImportAggregate"
      @update:aggregate-import-dialog-open="aggregateImportDialogOpen = $event"
      @update:aggregate-import-result-dialog-open="aggregateImportResultDialogOpen = $event"
      @update:aggregate-merge-mode="aggregateMergeMode = $event"
      @update:aggregate-merge-mode-select-open="aggregateMergeModeSelectOpen = $event"
    />
  </PageContainer>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, nextTick, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { Archive, Cable, FileClock, SlidersHorizontal } from 'lucide-vue-next'
import { PageHeader, PageContainer } from '@/components/layout'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui'
import Button from '@/components/ui/button.vue'
import { hasDesktopSession } from '@/desktop/session'
import { useDesktopGateway } from '@/desktop/useDesktopGateway'
import { useSystemConfig } from './system-settings/composables/useSystemConfig'
import { useConfigExportImport } from './system-settings/composables/useConfigExportImport'
import DataManagementSection from './system-settings/DataManagementSection.vue'
import DataMaintenanceSection from './system-settings/DataMaintenanceSection.vue'
import ProxyConfigSection from './system-settings/ProxyConfigSection.vue'
import BasicConfigSection from './system-settings/BasicConfigSection.vue'
import RequestLogSection from './system-settings/RequestLogSection.vue'
import CleanupPolicySection from './system-settings/CleanupPolicySection.vue'
import SystemInfoSection from './system-settings/SystemInfoSection.vue'
import DesktopGatewaySection from '@/desktop/DesktopGatewaySection.vue'
import ConfigImportDialog from './system-settings/ConfigImportDialog.vue'
import AggregateImportDialog from './system-settings/AggregateImportDialog.vue'
import './system-settings/settings.css'

const desktopMode = hasDesktopSession()
const desktopGateway = desktopMode ? useDesktopGateway() : null
const route = useRoute()
const router = useRouter()
const tabs = [
  { id: 'connection', label: desktopMode ? '连接与启动' : '网络连接', icon: Cable },
  { id: 'records', label: '记录与存储', icon: FileClock },
  { id: 'backup', label: '备份与恢复', icon: Archive },
  { id: 'advanced', label: '高级与诊断', icon: SlidersHorizontal },
]
const legacyTabs: Record<string, string> = {
  '#section-desktop-gateway': 'connection', '#section-proxy': 'connection',
  '#section-request-log': 'records', '#section-cleanup': 'records',
  '#section-data-mgmt': 'backup', '#section-basic': 'advanced', '#section-sysinfo': 'advanced',
}
const activeTab = computed(() => legacyTabs[route.hash]
  || (tabs.some(tab => tab.id === route.query.tab) ? String(route.query.tab) : 'connection'))
const visited = ref(new Set<string>())
watch(activeTab, tab => visited.value.add(tab), { immediate: true })

function selectTab(tab: string) {
  void router.push({ query: { ...route.query, tab }, hash: '' })
}

// System config composable
const {
  systemConfig,
  systemVersion,
  systemConfigLoading,
  systemConfigError,
  saveErrors,
  cancelChanges,
  confirmProxyCleared,
  autoCleanupLoading,
  proxyConfigLoading,
  basicConfigLoading,
  logConfigLoading,
  cleanupConfigLoading,
  hasProxyConfigChanges,
  hasBasicConfigChanges,
  hasLogConfigChanges,
  hasCleanupConfigChanges,
  sensitiveHeadersStr,
  loadSystemConfig,
  loadSystemVersion,
  saveProxyConfig,
  saveBasicConfig,
  saveLogConfig,
  saveCleanupConfig,
  handleAutoCleanupToggle,
} = useSystemConfig()

// 数据导出/导入 composable
const {
  exportLoading,
  importLoading,
  importDialogOpen,
  importResultDialogOpen,
  importPreview,
  importResult,
  mergeMode,
  mergeModeSelectOpen,
  importProgress,
  handleExportConfig,
  handleConfigFileSelect,
  confirmImport,
  exportAggregateLoading,
  importAggregateLoading,
  aggregateImportDialogOpen,
  aggregateImportResultDialogOpen,
  aggregateImportPreview,
  aggregateImportResult,
  aggregateMergeMode,
  aggregateMergeModeSelectOpen,
  importAggregateProgress,
  handleExportAggregate,
  handleAggregateFileSelect,
  confirmImportAggregate,
} = useConfigExportImport(async () => {
  await loadSystemConfig()
})

type DataManagementKind = 'config' | 'aggregate'

function handleDataExport(kind: DataManagementKind) {
  if (kind === 'config') {
    handleExportConfig()
  } else {
    handleExportAggregate()
  }
}

function handleDataFileSelect(kind: DataManagementKind, event: Event) {
  if (kind === 'config') {
    handleConfigFileSelect(event)
  } else {
    handleAggregateFileSelect(event)
  }
}


watch(() => route.fullPath, async () => {
  await nextTick()
  scrollToLegacySection()
})

function scrollToLegacySection() {
  if (!legacyTabs[route.hash]) return
  const section = document.getElementById(route.hash.slice(1))
  if (route.hash === '#section-cleanup') {
    const details = section?.querySelector('details')
    if (details) details.open = true
  }
  section?.scrollIntoView?.({ block: 'start' })
}

onMounted(async () => {
  await Promise.all([loadSystemConfig(), loadSystemVersion()])
  await nextTick()
  scrollToLegacySection()
})
</script>
