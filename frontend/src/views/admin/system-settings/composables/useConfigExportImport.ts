import { nextTick, ref } from 'vue'
import type { AxiosProgressEvent } from 'axios'
import { useToast } from '@/composables/useToast'
import {
  adminApi,
  type AggregateImportData,
  type AggregateImportResponse,
  type ConfigImportData,
  type ConfigImportResponse,
  type UsersImportData,
} from '@/api/admin'
import { parseApiError } from '@/utils/errorParser'
import { log } from '@/utils/logger'
import type { SystemConfig } from './useSystemConfig'

const BYTES_PER_MB = 1024 * 1024

type JsonObject = Record<string, unknown>

export interface ImportProgressState {
  percent: number
  message: string
}

function asJsonObject(value: unknown): JsonObject | null {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? value as JsonObject
    : null
}

function hasArrayField(value: JsonObject, key: string): boolean {
  return Array.isArray(value[key])
}

function looksLikeConfigExport(value: JsonObject): boolean {
  return hasArrayField(value, 'global_models')
    || hasArrayField(value, 'providers')
    || hasArrayField(value, 'proxy_nodes')
    || hasArrayField(value, 'system_configs')
}

function looksLikeUsersExport(value: JsonObject): boolean {
  return hasArrayField(value, 'users')
    || hasArrayField(value, 'standalone_keys')
}

function isConfigImportData(value: unknown): value is ConfigImportData {
  const data = asJsonObject(value)
  return data != null
    && ['global_models', 'proxy_nodes', 'system_configs'].every((key) => hasArrayField(data, key))
    && Array.isArray(data.providers)
    && data.providers.every((provider) => {
      const item = asJsonObject(provider)
      return item != null && ['endpoints', 'api_keys', 'models'].every((key) => hasArrayField(item, key))
    })
}

function isUsersImportData(value: unknown): value is UsersImportData {
  const data = asJsonObject(value)
  if (!data) return false
  const aggregates = asJsonObject(data.usage_aggregates)
  return Array.isArray(data.users)
    && data.users.every((user) => {
      const item = asJsonObject(user)
      return item != null
        && (item.username === undefined || typeof item.username === 'string')
        && hasArrayField(item, 'api_keys')
    })
    && hasArrayField(data, 'standalone_keys')
    && aggregates != null
    && ['stats_daily', 'stats_user_daily', 'stats_daily_api_key'].every((key) => hasArrayField(aggregates, key))
}

function looksLikeAggregateExport(value: JsonObject): boolean {
  return asJsonObject(value.config_data) != null
    && asJsonObject(value.user_data) != null
}

function formatBytes(bytes: number): string {
  if (bytes >= BYTES_PER_MB) {
    return `${(bytes / BYTES_PER_MB).toFixed(1)}MB`
  }
  if (bytes >= 1024) {
    return `${Math.round(bytes / 1024)}KB`
  }
  return `${bytes}B`
}

function setImportProgress(
  target: { value: ImportProgressState | null },
  percent: number,
  message: string,
) {
  target.value = {
    percent: Math.max(0, Math.min(100, Math.round(percent))),
    message,
  }
}

function buildUploadProgressHandler(
  target: { value: ImportProgressState | null },
  label: string,
) {
  return (event: AxiosProgressEvent) => {
    if (!event.total) {
      setImportProgress(target, 15, `${label}上传中：${formatBytes(event.loaded)}`)
      return
    }

    const uploadPercent = Math.min(85, 10 + (event.loaded / event.total) * 75)
    const loaded = formatBytes(event.loaded)
    const total = formatBytes(event.total)
    const message = event.loaded >= event.total
      ? `${label}已上传，服务端正在校验并写入数据`
      : `${label}上传中：${loaded} / ${total}`
    setImportProgress(target, uploadPercent, message)
  }
}

function downloadJson(data: unknown, filename: string) {
  const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}

export function useConfigExportImport(systemConfig: { value: Pick<SystemConfig, 'site_name'> }) {
  const { success, error } = useToast()

  // 配置导出/导入相关
  const exportLoading = ref(false)
  const importLoading = ref(false)
  const importDialogOpen = ref(false)
  const importResultDialogOpen = ref(false)
  const configFileInput = ref<HTMLInputElement | null>(null)
  const importPreview = ref<ConfigImportData | null>(null)
  const importResult = ref<ConfigImportResponse | null>(null)
  const mergeMode = ref<'skip' | 'overwrite' | 'error'>('skip')
  const mergeModeSelectOpen = ref(false)
  const importProgress = ref<ImportProgressState | null>(null)

  // 完整备份导出/导入相关
  const exportAggregateLoading = ref(false)
  const importAggregateLoading = ref(false)
  const aggregateImportDialogOpen = ref(false)
  const aggregateImportResultDialogOpen = ref(false)
  const aggregateImportPreview = ref<AggregateImportData | null>(null)
  const aggregateImportResult = ref<AggregateImportResponse | null>(null)
  const aggregateMergeMode = ref<'skip' | 'overwrite'>('skip')
  const aggregateMergeModeSelectOpen = ref(false)
  const importAggregateProgress = ref<ImportProgressState | null>(null)

  // 导出配置
  async function handleExportConfig() {
    exportLoading.value = true
    try {
      const data = await adminApi.exportConfig()
      downloadJson(
        data,
        `${systemConfig.value.site_name.toLowerCase()}-config-${new Date().toISOString().slice(0, 10)}.json`,
      )
      success('配置已导出')
    } catch (err) {
      error(parseApiError(err, '导出配置失败'))
      log.error('导出配置失败:', parseApiError(err))
    } finally {
      exportLoading.value = false
    }
  }

  // 触发文件选择
  function triggerConfigFileSelect() {
    configFileInput.value?.click()
  }

  // 处理文件选择
  function handleConfigFileSelect(event: Event) {
    const input = event.target as HTMLInputElement
    const file = input.files?.[0]
    if (!file) return
    importPreview.value = null
    importDialogOpen.value = false
    importResult.value = null
    importResultDialogOpen.value = false

    const reader = new FileReader()
    reader.onload = (e) => {
      try {
        const content = e.target?.result
        if (typeof content !== 'string') {
          error('读取文件失败，请重新选择文件')
          return
        }
        const root = asJsonObject(JSON.parse(content))
        if (!root) {
          error('无效的配置文件：JSON 顶层必须是对象')
          return
        }

        if (looksLikeUsersExport(root) && !looksLikeConfigExport(root)) {
          error('请使用完整备份导入用户资料和 API Keys')
          return
        }

        if (!isConfigImportData(root)) {
          error('无效的配置文件：未找到配置导出内容')
          return
        }

        importPreview.value = root
        mergeMode.value = 'skip'
        importDialogOpen.value = true
      } catch {
        error('解析配置文件失败，请确保是有效的 JSON 文件')
        log.error('解析配置文件失败')
      }
    }
    reader.onerror = () => error('读取文件失败，请重新选择文件')
    reader.readAsText(file)

    input.value = ''
  }

  // 确认导入
  async function confirmImport() {
    if (!importPreview.value || importLoading.value) return

    importLoading.value = true
    importResult.value = null
    importResultDialogOpen.value = false
    setImportProgress(importProgress, 5, '准备提交配置数据')
    await nextTick()
    try {
      const result = await adminApi.importConfig({
        ...importPreview.value,
        merge_mode: mergeMode.value,
      }, {
        onUploadProgress: buildUploadProgressHandler(importProgress, '配置数据'),
      })
      if (result.stats.errors?.length) {
        throw new Error('导入失败：服务端返回了错误，请检查目标系统中的数据后重试')
      }
      setImportProgress(importProgress, 100, '配置数据导入完成')
      importResult.value = result
      importDialogOpen.value = false
      mergeModeSelectOpen.value = false
      importResultDialogOpen.value = true
      success('配置导入成功')
    } catch (err: unknown) {
      error(parseApiError(err, '导入配置失败'))
      log.error('导入配置失败:', parseApiError(err))
    } finally {
      importLoading.value = false
      importProgress.value = null
    }
  }

  // 导出完整备份
  async function handleExportAggregate() {
    exportAggregateLoading.value = true
    try {
      const data = await adminApi.exportAggregateData()
      downloadJson(
        data,
        `${systemConfig.value.site_name.toLowerCase()}-data-${new Date().toISOString().slice(0, 10)}.json`,
      )
      success('完整备份已导出')
    } catch (err) {
      error(parseApiError(err, '导出完整备份失败'))
      log.error('导出完整备份失败:', parseApiError(err))
    } finally {
      exportAggregateLoading.value = false
    }
  }

  // 处理完整备份文件选择
  function handleAggregateFileSelect(event: Event) {
    const input = event.target as HTMLInputElement
    const file = input.files?.[0]
    if (!file) return
    aggregateImportPreview.value = null
    aggregateImportDialogOpen.value = false
    aggregateImportResult.value = null
    aggregateImportResultDialogOpen.value = false

    const reader = new FileReader()
    reader.onload = (e) => {
      try {
        const content = e.target?.result
        if (typeof content !== 'string') {
          error('读取文件失败，请重新选择文件')
          return
        }
        const root = asJsonObject(JSON.parse(content))
        if (!root) {
          error('无效的完整备份文件：JSON 顶层必须是对象')
          return
        }

        if (!looksLikeAggregateExport(root)) {
          if (looksLikeConfigExport(root)) {
            error('这是配置导出文件，请使用“导入配置数据”')
          } else if (looksLikeUsersExport(root)) {
            error('请使用完整备份导入用户资料和 API Keys')
          } else {
            error('无效的完整备份文件：未找到配置数据和用户数据')
          }
          return
        }

        const configData = root.config_data
        const userData = root.user_data
        if (!isConfigImportData(configData)) {
          error('无效的完整备份文件：config_data 格式不正确')
          return
        }
        if (!isUsersImportData(userData)) {
          error('无效的完整备份文件：user_data 格式不正确')
          return
        }

        aggregateImportPreview.value = { ...root, config_data: configData, user_data: userData }
        aggregateMergeMode.value = 'skip'
        aggregateImportDialogOpen.value = true
      } catch {
        error('解析完整备份文件失败，请确保是有效的 JSON 文件')
        log.error('解析完整备份文件失败')
      }
    }
    reader.onerror = () => error('读取文件失败，请重新选择文件')
    reader.readAsText(file)

    input.value = ''
  }

  // 确认导入完整备份
  async function confirmImportAggregate() {
    if (!aggregateImportPreview.value || importAggregateLoading.value) return

    importAggregateLoading.value = true
    aggregateImportResult.value = null
    aggregateImportResultDialogOpen.value = false
    setImportProgress(importAggregateProgress, 5, '准备提交完整备份')
    await nextTick()
    try {
      const result = await adminApi.importAggregateData({
        ...aggregateImportPreview.value,
        merge_mode: aggregateMergeMode.value,
      }, {
        onUploadProgress: buildUploadProgressHandler(importAggregateProgress, '完整备份'),
      })
      if (result.config.stats.errors?.length || result.users.stats.errors?.length) {
        throw new Error('导入失败：服务端返回了错误，请检查目标系统中的数据后重试')
      }
      setImportProgress(importAggregateProgress, 100, '完整备份导入完成')
      aggregateImportResult.value = result
      aggregateImportDialogOpen.value = false
      aggregateMergeModeSelectOpen.value = false
      aggregateImportResultDialogOpen.value = true
      success('完整备份导入成功')
    } catch (err: unknown) {
      error(parseApiError(err, '导入完整备份失败'))
      log.error('导入完整备份失败:', parseApiError(err))
    } finally {
      importAggregateLoading.value = false
      importAggregateProgress.value = null
    }
  }

  return {
    // 配置导出/导入
    exportLoading,
    importLoading,
    importDialogOpen,
    importResultDialogOpen,
    configFileInput,
    importPreview,
    importResult,
    mergeMode,
    mergeModeSelectOpen,
    importProgress,
    handleExportConfig,
    triggerConfigFileSelect,
    handleConfigFileSelect,
    confirmImport,
    // 完整备份导出/导入
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
  }
}
