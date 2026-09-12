<template>
  <PageContainer>
    <div class="system-settings">
      <PageHeader
        :title="pageTitle"
        class="settings-page-header"
      >
        <template #icon>
          <RouterLink
            v-if="activeTab !== 'general'"
            :to="{ query: { ...route.query, tab: 'general' }, hash: '' }"
            class="settings-back settings-icon-button"
            aria-label="返回系统设置"
            title="返回系统设置"
          >
            <ArrowLeft
              class="h-4 w-4"
              aria-hidden="true"
            />
          </RouterLink>
        </template>
        <template #actions>
          <RouterLink
            v-if="activeTab === 'general'"
            :to="{ query: { ...route.query, tab: 'advanced' }, hash: '' }"
            class="settings-page-link"
          >
            高级设置 <ChevronRight
              class="h-4 w-4"
              aria-hidden="true"
            />
          </RouterLink>
        </template>
      </PageHeader>
      <div class="settings-layout">
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
              v-if="visited.has('general')"
              v-show="activeTab === 'general'"
              class="settings-panel settings-gateway"
              aria-labelledby="connection-title"
            >
              <h2
                id="connection-title"
                class="settings-heading"
              >
                网关
              </h2>
              <ProxyConfigSection
                id="section-proxy"
                view="selector"
                :proxy-node-id="systemConfig.system_proxy_node_id"
                :loading="proxyConfigLoading"
                :has-changes="hasProxyConfigChanges"
                :error="saveErrors.proxy"
                @save="saveProxyConfig"
                @cancel="cancelChanges('proxy')"
                @proxy-cleared="confirmProxyCleared"
                @update:proxy-node-id="systemConfig.system_proxy_node_id = $event"
                @manage="selectTab('proxies')"
              />
              <DesktopGatewaySection
                v-if="desktopGateway"
                id="section-desktop-gateway"
                :gateway="desktopGateway"
                view="connection"
              />
            </section>

            <section
              v-if="visited.has('proxies')"
              v-show="activeTab === 'proxies'"
              class="settings-panel"
            >
              <ProxyConfigSection
                view="management"
                :proxy-node-id="systemConfig.system_proxy_node_id"
                :loading="proxyConfigLoading"
                :has-changes="hasProxyConfigChanges"
                @proxy-cleared="confirmProxyCleared"
                @update:proxy-node-id="systemConfig.system_proxy_node_id = $event"
              />
            </section>

            <section
              v-if="visited.has('advanced')"
              v-show="activeTab === 'advanced'"
              class="settings-panel"
            >
              <details
                id="section-records"
                class="settings-disclosure settings-advanced-group"
              >
                <summary>
                  记录与清理<ChevronDown
                    class="settings-chevron"
                    aria-hidden="true"
                  />
                </summary>
                <div class="settings-disclosure-content">
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
                </div>
              </details>
            </section>

            <section
              v-if="visited.has('general')"
              v-show="activeTab === 'general'"
              class="settings-panel settings-backup"
            >
              <DataManagementSection
                id="section-data-mgmt"
                view="backup"
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
            >
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
              <details class="settings-disclosure settings-advanced-group">
                <summary>
                  配置迁移<ChevronDown
                    class="settings-chevron"
                    aria-hidden="true"
                  />
                </summary>
                <div class="settings-disclosure-content">
                  <DataManagementSection
                    view="migration"
                    :config-export-loading="exportLoading"
                    :config-import-loading="importLoading"
                    :aggregate-export-loading="exportAggregateLoading"
                    :aggregate-import-loading="importAggregateLoading"
                    @export="handleDataExport"
                    @file-select="handleDataFileSelect"
                  />
                </div>
              </details>
              <details
                id="section-diagnostics"
                class="settings-disclosure settings-advanced-group"
              >
                <summary>
                  诊断<ChevronDown
                    class="settings-chevron"
                    aria-hidden="true"
                  />
                </summary>
                <div class="settings-disclosure-content">
                  <SystemInfoSection
                    id="section-sysinfo"
                    :system-version="displayVersion"
                  />
                  <DesktopGatewaySection
                    v-if="desktopGateway"
                    :gateway="desktopGateway"
                    view="diagnostics"
                  />
                </div>
              </details>
              <DataMaintenanceSection
                class="settings-advanced-group"
                :active="activeTab === 'advanced'"
              />
            </section>
          </div>
        </div>
      </div>
      <footer
        v-if="displayVersion"
        class="settings-version"
      >
        Aether <samp>{{ displayVersion }}</samp>
      </footer>
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
import { ArrowLeft, ChevronDown, ChevronRight } from 'lucide-vue-next'
import { PageHeader, PageContainer } from '@/components/layout'
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
const tabs = ['general', 'proxies', 'advanced']
const legacyTabs: Record<string, string> = {
  '#section-desktop-gateway': 'general', '#section-proxy': 'general',
  '#section-request-log': 'advanced', '#section-cleanup': 'advanced',
  '#section-data-mgmt': 'general', '#section-basic': 'advanced', '#section-sysinfo': 'advanced',
}
const legacyQueries = new Map([
  ['connection', 'general'], ['records', 'advanced'], ['backup', 'general'],
])
const activeTab = computed(() => legacyTabs[route.hash]
  || legacyQueries.get(String(route.query.tab))
  || (tabs.includes(String(route.query.tab)) ? String(route.query.tab) : 'general'))
const pageTitle = computed(() => activeTab.value === 'proxies' ? '代理管理'
  : activeTab.value === 'advanced' ? '高级设置' : '系统设置')
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
const displayVersion = computed(() => desktopGateway?.status.value?.version || systemVersion.value)

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
  const sectionId = legacyTabs[route.hash] ? route.hash.slice(1)
    : route.query.tab === 'records' ? 'section-records'
      : route.query.tab === 'backup' ? 'section-data-mgmt' : ''
  if (!sectionId) return
  const section = document.getElementById(sectionId)
  if (section instanceof window.HTMLDetailsElement) section.open = true
  let parent = section?.parentElement
  while (parent && !parent.classList.contains('system-settings')) {
    if (parent instanceof window.HTMLDetailsElement) parent.open = true
    parent = parent.parentElement
  }
  if (sectionId === 'section-cleanup' || sectionId === 'section-basic') {
    section?.querySelectorAll('details').forEach(details => { details.open = true })
  }
  section?.scrollIntoView?.({ block: 'start' })
}

onMounted(async () => {
  await Promise.all([loadSystemConfig(), loadSystemVersion()])
  await nextTick()
  scrollToLegacySection()
})
</script>
