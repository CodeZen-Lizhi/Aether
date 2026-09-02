import { ref, computed } from 'vue'
import { useToast } from '@/composables/useToast'
import { adminApi } from '@/api/admin'
import { log } from '@/utils/logger'
import { useSiteInfo } from '@/composables/useSiteInfo'

export interface SystemConfig {
  // 站点信息
  site_name: string
  site_subtitle: string
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
  // 定时任务
  enable_provider_checkin: boolean
  provider_checkin_time: string
  enable_oauth_token_refresh: boolean
}

const CONFIG_KEYS = [
  // 站点信息
  'site_name',
  'site_subtitle',
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
  // 定时任务
  'enable_provider_checkin',
  'provider_checkin_time',
  'enable_oauth_token_refresh',
]

function createDefaultConfig(): SystemConfig {
  return {
    // 站点信息
    site_name: 'Aether',
    site_subtitle: 'AI Gateway',
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
    // 定时任务
    enable_provider_checkin: true,
    provider_checkin_time: '01:05',
    enable_oauth_token_refresh: true,
  }
}

export function useSystemConfig() {
  const { success, error } = useToast()
  const { refreshSiteInfo } = useSiteInfo()

  const systemConfig = ref<SystemConfig>(createDefaultConfig())
  const originalConfig = ref<SystemConfig | null>(null)
  const systemVersion = ref<string>('')
  const systemConfigLoading = ref(true)

  // 各模块 loading 状态
  const siteInfoLoading = ref(false)
  const proxyConfigLoading = ref(false)
  const basicConfigLoading = ref(false)
  const logConfigLoading = ref(false)
  const cleanupConfigLoading = ref(false)

  // 变动检测
  const hasSiteInfoChanges = computed(() => {
    if (systemConfigLoading.value) return false
    if (!originalConfig.value) return false
    return (
      systemConfig.value.site_name !== originalConfig.value.site_name ||
      systemConfig.value.site_subtitle !== originalConfig.value.site_subtitle
    )
  })

  const hasProxyConfigChanges = computed(() => {
    if (systemConfigLoading.value) return false
    if (!originalConfig.value) return false
    return systemConfig.value.system_proxy_node_id !== originalConfig.value.system_proxy_node_id
  })

  const hasBasicConfigChanges = computed(() => {
    if (systemConfigLoading.value) return false
    if (!originalConfig.value) return false
    return (
      systemConfig.value.rate_limit_per_minute !== originalConfig.value.rate_limit_per_minute ||
      systemConfig.value.auto_delete_expired_keys !== originalConfig.value.auto_delete_expired_keys ||
      systemConfig.value.enable_format_conversion !== originalConfig.value.enable_format_conversion ||
      systemConfig.value.enable_openai_image_sync_heartbeat !==
      originalConfig.value.enable_openai_image_sync_heartbeat ||
      systemConfig.value.enable_standard_text_sync_heartbeat !==
      originalConfig.value.enable_standard_text_sync_heartbeat ||
      systemConfig.value.cyber_continue_failover !==
      originalConfig.value.cyber_continue_failover
    )
  })

  const hasLogConfigChanges = computed(() => {
    if (systemConfigLoading.value) return false
    if (!originalConfig.value) return false
    return (
      systemConfig.value.request_record_level !== originalConfig.value.request_record_level ||
      JSON.stringify(systemConfig.value.sensitive_headers) !==
        JSON.stringify(originalConfig.value.sensitive_headers)
    )
  })

  const hasCleanupConfigChanges = computed(() => {
    if (systemConfigLoading.value) return false
    if (!originalConfig.value) return false
    return (
      systemConfig.value.detail_log_retention_days !==
      originalConfig.value.detail_log_retention_days ||
      systemConfig.value.compressed_log_retention_days !==
      originalConfig.value.compressed_log_retention_days ||
      systemConfig.value.header_retention_days !== originalConfig.value.header_retention_days ||
      systemConfig.value.log_retention_days !== originalConfig.value.log_retention_days ||
      systemConfig.value.cleanup_batch_size !== originalConfig.value.cleanup_batch_size ||
      systemConfig.value.audit_log_retention_days !==
      originalConfig.value.audit_log_retention_days ||
      systemConfig.value.request_candidates_retention_days !==
      originalConfig.value.request_candidates_retention_days ||
      systemConfig.value.request_candidates_cleanup_batch_size !==
      originalConfig.value.request_candidates_cleanup_batch_size ||
      systemConfig.value.proxy_node_metrics_1m_retention_days !==
      originalConfig.value.proxy_node_metrics_1m_retention_days ||
      systemConfig.value.proxy_node_metrics_1h_retention_days !==
      originalConfig.value.proxy_node_metrics_1h_retention_days ||
      systemConfig.value.proxy_node_metrics_cleanup_batch_size !==
      originalConfig.value.proxy_node_metrics_cleanup_batch_size
    )
  })

  // 敏感请求头数组和字符串之间的转换
  const sensitiveHeadersStr = computed({
    get: () => systemConfig.value.sensitive_headers.join(', '),
    set: (val: string) => {
      systemConfig.value.sensitive_headers = val
        .split(',')
        .map((s) => s.trim().toLowerCase())
        .filter((s) => s.length > 0)
    },
  })

  // 加载配置
  async function loadSystemConfig() {
    systemConfigLoading.value = true
    try {
      const configs = await adminApi.getAllSystemConfigs({ cacheTtlMs: 30_000 })
      const configsByKey = new Map(configs.map((config) => [config.key, config]))

      const nextConfig = createDefaultConfig()
      for (const key of CONFIG_KEYS) {
        const response = configsByKey.get(key)
        if (!response) continue
        try {
          if (response.value !== null && response.value !== undefined) {
            ; (nextConfig as unknown as Record<string, unknown>)[key] = response.value
          }
        } catch {
          // 单个配置项加载失败时忽略，使用默认值
        }
      }
      systemConfig.value = nextConfig
      originalConfig.value = JSON.parse(JSON.stringify(nextConfig))
    } catch (err) {
      error('加载系统配置失败')
      log.error('加载系统配置失败:', err)
    } finally {
      systemConfigLoading.value = false
    }
  }

  async function loadSystemVersion() {
    try {
      const data = await adminApi.getSystemVersion()
      systemVersion.value = data.version
    } catch (err) {
      log.error('加载系统版本失败:', err)
    }
  }

  // 保存函数
  async function saveSiteInfo() {
    siteInfoLoading.value = true
    try {
      const configItems = [
        { key: 'site_name', value: systemConfig.value.site_name, description: '站点名称' },
        {
          key: 'site_subtitle',
          value: systemConfig.value.site_subtitle,
          description: '站点副标题',
        },
      ]
      await Promise.all(
        configItems.map((item) =>
          adminApi.updateSystemConfig(item.key, item.value, item.description)
        )
      )
      if (originalConfig.value) {
        originalConfig.value.site_name = systemConfig.value.site_name
        originalConfig.value.site_subtitle = systemConfig.value.site_subtitle
      }
      await refreshSiteInfo()
      success('站点信息已保存')
    } catch (err) {
      error('保存站点信息失败')
      log.error('保存站点信息失败:', err)
    } finally {
      siteInfoLoading.value = false
    }
  }

  async function saveProxyConfig() {
    proxyConfigLoading.value = true
    try {
      await adminApi.updateSystemConfig(
        'system_proxy_node_id',
        systemConfig.value.system_proxy_node_id || null,
        '系统默认代理节点 ID'
      )
      if (originalConfig.value) {
        originalConfig.value.system_proxy_node_id = systemConfig.value.system_proxy_node_id
      }
      success('网络代理配置已保存')
    } catch (err) {
      error('保存代理配置失败')
      log.error('保存代理配置失败:', err)
    } finally {
      proxyConfigLoading.value = false
    }
  }

  async function saveBasicConfig() {
    basicConfigLoading.value = true
    try {
      const configItems = [
        {
          key: 'rate_limit_per_minute',
          value: systemConfig.value.rate_limit_per_minute,
          description: '每分钟请求限制',
        },
        {
          key: 'auto_delete_expired_keys',
          value: systemConfig.value.auto_delete_expired_keys,
          description: '是否自动删除过期的API Key',
        },
        {
          key: 'enable_format_conversion',
          value: systemConfig.value.enable_format_conversion,
          description: '全局格式转换开关：开启时强制允许所有提供商的格式转换',
        },
        {
          key: 'enable_openai_image_sync_heartbeat',
          value: systemConfig.value.enable_openai_image_sync_heartbeat,
          description: '同步生图心跳开关：开启后外层 HTTP 状态固定为 200，上游失败写入响应体',
        },
        {
          key: 'enable_standard_text_sync_heartbeat',
          value: systemConfig.value.enable_standard_text_sync_heartbeat,
          description: '标准文本非流式心跳开关：开启后外层 HTTP 状态固定为 200，上游失败写入响应体',
        },
        {
          key: 'cyber_continue_failover',
          value: systemConfig.value.cyber_continue_failover,
          description: 'Cyber继续转移开关：开启后在响应内容开始前将Cyber Policy错误按普通错误继续故障转移，可能增加首字等待时间',
        },
      ]

      await Promise.all(
        configItems.map((item) =>
          adminApi.updateSystemConfig(item.key, item.value, item.description)
        )
      )
      if (originalConfig.value) {
        originalConfig.value.rate_limit_per_minute = systemConfig.value.rate_limit_per_minute
        originalConfig.value.auto_delete_expired_keys =
          systemConfig.value.auto_delete_expired_keys
        originalConfig.value.enable_format_conversion =
          systemConfig.value.enable_format_conversion
        originalConfig.value.enable_openai_image_sync_heartbeat =
          systemConfig.value.enable_openai_image_sync_heartbeat
        originalConfig.value.enable_standard_text_sync_heartbeat =
          systemConfig.value.enable_standard_text_sync_heartbeat
        originalConfig.value.cyber_continue_failover =
          systemConfig.value.cyber_continue_failover
      }
      success('基础配置已保存')
    } catch (err) {
      error('保存配置失败')
      log.error('保存基础配置失败:', err)
    } finally {
      basicConfigLoading.value = false
    }
  }

  async function saveLogConfig() {
    logConfigLoading.value = true
    try {
      const configItems = [
        {
          key: 'request_record_level',
          value: systemConfig.value.request_record_level,
          description: '请求记录级别',
        },
        {
          key: 'sensitive_headers',
          value: systemConfig.value.sensitive_headers,
          description: '敏感请求头列表',
        },
      ]

      await Promise.all(
        configItems.map((item) =>
          adminApi.updateSystemConfig(item.key, item.value, item.description)
        )
      )
      if (originalConfig.value) {
        originalConfig.value.request_record_level = systemConfig.value.request_record_level
        originalConfig.value.sensitive_headers = [...systemConfig.value.sensitive_headers]
      }
      success('请求记录配置已保存')
    } catch (err) {
      error('保存配置失败')
      log.error('保存请求记录配置失败:', err)
    } finally {
      logConfigLoading.value = false
    }
  }

  async function saveCleanupConfig() {
    cleanupConfigLoading.value = true
    try {
      const configItems = [
        {
          key: 'detail_log_retention_days',
          value: systemConfig.value.detail_log_retention_days,
          description: '详细记录保留天数',
        },
        {
          key: 'compressed_log_retention_days',
          value: systemConfig.value.compressed_log_retention_days,
          description: '压缩记录保留天数',
        },
        {
          key: 'header_retention_days',
          value: systemConfig.value.header_retention_days,
          description: '请求头保留天数',
        },
        {
          key: 'log_retention_days',
          value: systemConfig.value.log_retention_days,
          description: '完整记录保留天数',
        },
        {
          key: 'cleanup_batch_size',
          value: systemConfig.value.cleanup_batch_size,
          description: '每批次清理的记录数',
        },
        {
          key: 'audit_log_retention_days',
          value: systemConfig.value.audit_log_retention_days,
          description: '审计日志保留天数',
        },
        {
          key: 'request_candidates_retention_days',
          value: systemConfig.value.request_candidates_retention_days,
          description: '请求候选记录保留天数',
        },
        {
          key: 'request_candidates_cleanup_batch_size',
          value: systemConfig.value.request_candidates_cleanup_batch_size,
          description: '请求候选记录每批次清理条数',
        },
        {
          key: 'proxy_node_metrics_1m_retention_days',
          value: systemConfig.value.proxy_node_metrics_1m_retention_days,
          description: '代理节点 1m 指标保留天数',
        },
        {
          key: 'proxy_node_metrics_1h_retention_days',
          value: systemConfig.value.proxy_node_metrics_1h_retention_days,
          description: '代理节点 1h 指标保留天数',
        },
        {
          key: 'proxy_node_metrics_cleanup_batch_size',
          value: systemConfig.value.proxy_node_metrics_cleanup_batch_size,
          description: '代理节点指标每批次清理条数',
        },
      ]

      await Promise.all(
        configItems.map((item) =>
          adminApi.updateSystemConfig(item.key, item.value, item.description)
        )
      )
      if (originalConfig.value) {
        originalConfig.value.detail_log_retention_days =
          systemConfig.value.detail_log_retention_days
        originalConfig.value.compressed_log_retention_days =
          systemConfig.value.compressed_log_retention_days
        originalConfig.value.header_retention_days = systemConfig.value.header_retention_days
        originalConfig.value.log_retention_days = systemConfig.value.log_retention_days
        originalConfig.value.cleanup_batch_size = systemConfig.value.cleanup_batch_size
        originalConfig.value.audit_log_retention_days =
          systemConfig.value.audit_log_retention_days
        originalConfig.value.request_candidates_retention_days =
          systemConfig.value.request_candidates_retention_days
        originalConfig.value.request_candidates_cleanup_batch_size =
          systemConfig.value.request_candidates_cleanup_batch_size
        originalConfig.value.proxy_node_metrics_1m_retention_days =
          systemConfig.value.proxy_node_metrics_1m_retention_days
        originalConfig.value.proxy_node_metrics_1h_retention_days =
          systemConfig.value.proxy_node_metrics_1h_retention_days
        originalConfig.value.proxy_node_metrics_cleanup_batch_size =
          systemConfig.value.proxy_node_metrics_cleanup_batch_size
      }
      success('请求记录清理配置已保存')
    } catch (err) {
      error('保存配置失败')
      log.error('保存请求记录清理配置失败:', err)
    } finally {
      cleanupConfigLoading.value = false
    }
  }

  async function handleAutoCleanupToggle(enabled: boolean) {
    const previousValue = systemConfig.value.enable_auto_cleanup
    systemConfig.value.enable_auto_cleanup = enabled
    try {
      await adminApi.updateSystemConfig(
        'enable_auto_cleanup',
        enabled,
        '是否启用自动清理任务'
      )
      success(enabled ? '已启用自动清理' : '已禁用自动清理')
    } catch (err) {
      error('保存配置失败')
      log.error('保存自动清理配置失败:', err)
      systemConfig.value.enable_auto_cleanup = previousValue
    }
  }

  return {
    systemConfig,
    originalConfig,
    systemVersion,
    systemConfigLoading,
    // loading 状态
    siteInfoLoading,
    proxyConfigLoading,
    basicConfigLoading,
    logConfigLoading,
    cleanupConfigLoading,
    // 变动检测
    hasSiteInfoChanges,
    hasProxyConfigChanges,
    hasBasicConfigChanges,
    hasLogConfigChanges,
    hasCleanupConfigChanges,
    // 计算属性
    sensitiveHeadersStr,
    // 加载函数
    loadSystemConfig,
    loadSystemVersion,
    // 保存函数
    saveSiteInfo,
    saveProxyConfig,
    saveBasicConfig,
    saveLogConfig,
    saveCleanupConfig,
    handleAutoCleanupToggle,
  }
}
