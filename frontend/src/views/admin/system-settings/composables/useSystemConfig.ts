import { ref, computed } from 'vue'
import { useToast } from '@/composables/useToast'
import { adminApi } from '@/api/admin'
import { log } from '@/utils/logger'
import { desktopApi } from '@/desktop/bridge'
import { hasDesktopSession } from '@/desktop/session'

export interface SystemConfig {
  // 网络代理
  system_proxy_node_id: string | null
  // 基础配置
  rate_limit_per_minute: number
  // 独立余额 Key 过期管理
  auto_delete_expired_keys: boolean
  // 格式转换
  enable_format_conversion: boolean
  // 同步生图心跳
  enable_openai_image_sync_heartbeat: boolean
  // 标准文本非流式心跳
  enable_standard_text_sync_heartbeat: boolean
  // Cyber Policy 错误继续故障转移
  cyber_continue_failover: boolean
  // 请求记录
  request_record_level: string
  sensitive_headers: string[]
  // 请求记录清理
  enable_auto_cleanup: boolean
  detail_log_retention_days: number
  compressed_log_retention_days: number
  header_retention_days: number
  log_retention_days: number
  cleanup_batch_size: number
  audit_log_retention_days: number
  request_candidates_retention_days: number
  request_candidates_cleanup_batch_size: number
  proxy_node_metrics_1m_retention_days: number
  proxy_node_metrics_1h_retention_days: number
  proxy_node_metrics_cleanup_batch_size: number
}

const CONFIG_KEYS = [
  // 网络代理
  'system_proxy_node_id',
  // 基础配置
  'rate_limit_per_minute',
  // 独立余额 Key 过期管理
  'auto_delete_expired_keys',
  // 格式转换
  'enable_format_conversion',
  // 同步生图心跳
  'enable_openai_image_sync_heartbeat',
  // 标准文本非流式心跳
  'enable_standard_text_sync_heartbeat',
  // Cyber Policy 错误继续故障转移
  'cyber_continue_failover',
  // 请求记录
  'request_record_level',
  'sensitive_headers',
  // 请求记录清理
  'enable_auto_cleanup',
  'detail_log_retention_days',
  'compressed_log_retention_days',
  'header_retention_days',
  'log_retention_days',
  'cleanup_batch_size',
  'audit_log_retention_days',
  'request_candidates_retention_days',
  'request_candidates_cleanup_batch_size',
  'proxy_node_metrics_1m_retention_days',
  'proxy_node_metrics_1h_retention_days',
  'proxy_node_metrics_cleanup_batch_size',
]

function createDefaultConfig(): SystemConfig {
  return {
    // 网络代理
    system_proxy_node_id: null,
    // 基础配置
    rate_limit_per_minute: 0,
    // 独立余额 Key 过期管理
    auto_delete_expired_keys: false,
    // 格式转换
    enable_format_conversion: false,
    // 同步生图心跳
    enable_openai_image_sync_heartbeat: false,
    // 标准文本非流式心跳
    enable_standard_text_sync_heartbeat: false,
    // Cyber Policy 错误继续故障转移
    cyber_continue_failover: false,
    // 请求记录
    request_record_level: 'full',
    sensitive_headers: ['authorization', 'x-api-key', 'api-key', 'cookie', 'set-cookie'],
    // 请求记录清理
    enable_auto_cleanup: true,
    detail_log_retention_days: 7,
    compressed_log_retention_days: 30,
    header_retention_days: 90,
    log_retention_days: 365,
    cleanup_batch_size: 1000,
    audit_log_retention_days: 30,
    request_candidates_retention_days: 30,
    request_candidates_cleanup_batch_size: 5000,
    proxy_node_metrics_1m_retention_days: 30,
    proxy_node_metrics_1h_retention_days: 180,
    proxy_node_metrics_cleanup_batch_size: 5000,
  }
}

type ConfigKey = keyof SystemConfig
type SaveGroup = 'proxy' | 'basic' | 'log' | 'cleanup'

const groupItems: Record<SaveGroup, Array<{ key: ConfigKey; description: string }>> = {
  proxy: [{ key: 'system_proxy_node_id', description: '系统默认代理节点 ID' }],
  basic: [
    { key: 'rate_limit_per_minute', description: '每分钟请求限制' },
    { key: 'auto_delete_expired_keys', description: '是否自动删除过期的API Key' },
    { key: 'enable_format_conversion', description: '全局格式转换开关：开启时强制允许所有提供商的格式转换' },
    { key: 'enable_openai_image_sync_heartbeat', description: '同步生图心跳开关：开启后外层 HTTP 状态固定为 200，上游失败写入响应体' },
    { key: 'enable_standard_text_sync_heartbeat', description: '标准文本非流式心跳开关：开启后外层 HTTP 状态固定为 200，上游失败写入响应体' },
    { key: 'cyber_continue_failover', description: 'Cyber继续转移开关：开启后在响应内容开始前将Cyber Policy错误按普通错误继续故障转移，可能增加首字等待时间' },
  ],
  log: [
    { key: 'request_record_level', description: '请求记录级别' },
    { key: 'sensitive_headers', description: '敏感请求头列表' },
  ],
  cleanup: [
    { key: 'detail_log_retention_days', description: '详细记录保留天数' },
    { key: 'compressed_log_retention_days', description: '压缩记录保留天数' },
    { key: 'header_retention_days', description: '请求头保留天数' },
    { key: 'log_retention_days', description: '完整记录保留天数' },
    { key: 'audit_log_retention_days', description: '审计日志保留天数' },
    { key: 'request_candidates_retention_days', description: '请求候选记录保留天数' },
    { key: 'proxy_node_metrics_1m_retention_days', description: '代理节点 1m 指标保留天数' },
    { key: 'proxy_node_metrics_1h_retention_days', description: '代理节点 1h 指标保留天数' },
  ],
}

export function useSystemConfig() {
  const { success, error } = useToast()
  const systemConfig = ref<SystemConfig>(createDefaultConfig())
  const originalConfig = ref<SystemConfig | null>(null)
  const systemVersion = ref('')
  const systemConfigLoading = ref(true)
  const systemConfigError = ref('')
  const proxyConfigLoading = ref(false)
  const basicConfigLoading = ref(false)
  const logConfigLoading = ref(false)
  const cleanupConfigLoading = ref(false)
  const autoCleanupLoading = ref(false)
  const saveErrors = ref<Record<SaveGroup, string>>({ proxy: '', basic: '', log: '', cleanup: '' })
  const loadingByGroup = { proxy: proxyConfigLoading, basic: basicConfigLoading, log: logConfigLoading, cleanup: cleanupConfigLoading }

  function changed(group: SaveGroup) {
    const baseline = originalConfig.value
    return !systemConfigLoading.value && !!baseline && groupItems[group].some(({ key }) =>
      JSON.stringify(systemConfig.value[key]) !== JSON.stringify(baseline[key])
    )
  }

  const hasProxyConfigChanges = computed(() => changed('proxy'))
  const hasBasicConfigChanges = computed(() => changed('basic'))
  const hasLogConfigChanges = computed(() => changed('log'))
  const hasCleanupConfigChanges = computed(() => changed('cleanup'))

  const sensitiveHeadersStr = computed({
    get: () => systemConfig.value.sensitive_headers.join(', '),
    set: (value: string) => {
      systemConfig.value.sensitive_headers = value.split(',').map(header => header.trim().toLowerCase()).filter(Boolean)
    },
  })

  function copyValue<K extends ConfigKey>(target: SystemConfig, source: SystemConfig, key: K) {
    const value = source[key]
    target[key] = (Array.isArray(value) ? [...value] : value) as SystemConfig[K]
  }

  function cancelChanges(group: SaveGroup) {
    if (!originalConfig.value || loadingByGroup[group].value) return
    for (const { key } of groupItems[group]) copyValue(systemConfig.value, originalConfig.value, key)
    saveErrors.value[group] = ''
  }

  function confirmProxyCleared() {
    if (originalConfig.value) originalConfig.value.system_proxy_node_id = null
  }

  async function loadSystemConfig() {
    systemConfigLoading.value = true
    systemConfigError.value = ''
    try {
      const configs = await adminApi.getAllSystemConfigs({ cacheTtlMs: 30_000 })
      const nextConfig = createDefaultConfig()
      for (const config of configs) {
        if (CONFIG_KEYS.includes(config.key) && config.value !== null && config.value !== undefined) {
          (nextConfig as unknown as Record<string, unknown>)[config.key] = config.value
        }
      }
      systemConfig.value = nextConfig
      originalConfig.value = JSON.parse(JSON.stringify(nextConfig))
      saveErrors.value = { proxy: '', basic: '', log: '', cleanup: '' }
    } catch (err) {
      systemConfigError.value = '加载系统配置失败'
      error(systemConfigError.value)
      log.error('加载系统配置失败:', err)
    } finally {
      systemConfigLoading.value = false
    }
  }

  async function loadSystemVersion() {
    try {
      if (hasDesktopSession()) {
        try {
          systemVersion.value = (await desktopApi.status()).version
          return
        } catch (err) {
          log.error('加载桌面应用版本失败，回退到网关版本:', err)
        }
      }
      systemVersion.value = (await adminApi.getSystemVersion()).version
    } catch (err) {
      log.error('加载系统版本失败:', err)
    }
  }

  // Each API call commits independently. Only acknowledge the captured values that succeeded.
  async function saveGroup(group: SaveGroup, message: string) {
    if (loadingByGroup[group].value || !originalConfig.value || !changed(group)) return
    const baseline = originalConfig.value
    const snapshot: SystemConfig = JSON.parse(JSON.stringify(systemConfig.value))
    const items = groupItems[group].filter(({ key }) =>
      JSON.stringify(snapshot[key]) !== JSON.stringify(baseline[key])
    )
    loadingByGroup[group].value = true
    saveErrors.value[group] = ''
    try {
      const results = await Promise.allSettled(items.map(item =>
        adminApi.updateSystemConfig(item.key, snapshot[item.key], item.description)
      ))
      let failed = 0
      results.forEach((result, index) => {
        if (result.status === 'fulfilled') {
          copyValue(baseline, snapshot, items[index].key)
        } else {
          failed += 1
          log.error('保存配置失败:', { key: items[index].key, error: result.reason })
        }
      })
      if (failed) {
        saveErrors.value[group] = failed === items.length
          ? '保存失败，修改已保留，请重试。'
          : '部分配置已保存，其余修改仍待保存，请重试。'
        error(saveErrors.value[group])
      } else {
        success(message)
      }
    } finally {
      loadingByGroup[group].value = false
    }
  }

  async function handleAutoCleanupToggle(enabled: boolean) {
    if (autoCleanupLoading.value || !originalConfig.value) return
    const previousValue = systemConfig.value.enable_auto_cleanup
    systemConfig.value.enable_auto_cleanup = enabled
    autoCleanupLoading.value = true
    try {
      await adminApi.updateSystemConfig('enable_auto_cleanup', enabled, '是否启用自动清理任务')
      originalConfig.value.enable_auto_cleanup = enabled
      success(enabled ? '已启用自动清理' : '已禁用自动清理')
    } catch (err) {
      error('保存配置失败')
      log.error('保存自动清理配置失败:', err)
      systemConfig.value.enable_auto_cleanup = previousValue
    } finally {
      autoCleanupLoading.value = false
    }
  }

  return {
    systemConfig, originalConfig, systemVersion, systemConfigLoading, systemConfigError,
    proxyConfigLoading, basicConfigLoading, logConfigLoading, cleanupConfigLoading, autoCleanupLoading,
    hasProxyConfigChanges, hasBasicConfigChanges, hasLogConfigChanges, hasCleanupConfigChanges,
    sensitiveHeadersStr, saveErrors, loadSystemConfig, loadSystemVersion, cancelChanges, confirmProxyCleared,
    saveProxyConfig: () => saveGroup('proxy', '网络代理配置已保存'),
    saveBasicConfig: () => saveGroup('basic', '基础配置已保存'),
    saveLogConfig: () => saveGroup('log', '请求记录配置已保存'),
    saveCleanupConfig: () => saveGroup('cleanup', '请求记录清理配置已保存'),
    handleAutoCleanupToggle,
  }
}
