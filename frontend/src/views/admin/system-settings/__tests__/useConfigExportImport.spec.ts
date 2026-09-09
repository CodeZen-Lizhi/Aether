import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ref } from 'vue'
import type { AggregateImportResponse, ConfigImportResponse } from '@/api/admin'

const {
  errorMock,
  successMock,
  importConfigMock,
  importAggregateMock,
  exportConfigMock,
  exportAggregateMock,
} = vi.hoisted(() => ({
  errorMock: vi.fn(),
  successMock: vi.fn(),
  importConfigMock: vi.fn(),
  importAggregateMock: vi.fn(),
  exportConfigMock: vi.fn(),
  exportAggregateMock: vi.fn(),
}))

vi.mock('@/composables/useToast', () => ({
  useToast: () => ({ error: errorMock, success: successMock }),
}))

vi.mock('@/utils/logger', () => ({ log: { error: vi.fn() } }))

vi.mock('@/api/admin', async (importOriginal) => ({
  ...await importOriginal<typeof import('@/api/admin')>(),
  adminApi: {
    importConfig: importConfigMock,
    importAggregateData: importAggregateMock,
    exportConfig: exportConfigMock,
    exportAggregateData: exportAggregateMock,
  },
}))

import { useConfigExportImport } from '../composables/useConfigExportImport'

function buildFileInputEvent(content: string, size = content.length): Event {
  const file = new File([content], 'backup.json', { type: 'application/json' })
  Object.defineProperty(file, 'size', { value: size })
  const input = document.createElement('input')
  input.type = 'file'
  Object.defineProperty(input, 'files', { value: [file] })
  const event = new Event('change')
  Object.defineProperty(event, 'target', { value: input })
  return event
}

function createState() {
  return useConfigExportImport(ref({ site_name: 'Aether' }))
}

type ImportState = ReturnType<typeof createState>

async function selectConfig(state: ImportState, data: unknown = configBackup) {
  state.handleConfigFileSelect(buildFileInputEvent(JSON.stringify(data)))
  await vi.waitFor(() => expect(state.importDialogOpen.value).toBe(true))
}

async function selectAggregate(state: ImportState, data: unknown = aggregateBackup) {
  state.handleAggregateFileSelect(buildFileInputEvent(JSON.stringify(data)))
  await vi.waitFor(() => expect(state.aggregateImportDialogOpen.value).toBe(true))
}

const configBackup = {
  exported_at: '2026-09-08T00:00:00.000Z',
  global_models: [],
  providers: [],
  proxy_nodes: [],
  system_configs: [],
}

const aggregateBackup = {
  exported_at: configBackup.exported_at,
  config_data: configBackup,
  user_data: {
    exported_at: configBackup.exported_at,
    provider_names: {},
    users: [{ id: 'admin-source', username: 'admin', role: 'admin', api_keys: [], preferences: null }],
    standalone_keys: [],
    usage_aggregates: { stats_daily: [], stats_user_daily: [], stats_daily_api_key: [] },
  },
}

function configImportResult(): ConfigImportResponse {
  return {
    message: 'ok',
    stats: {
      global_models: { created: 1, updated: 0, skipped: 0 },
      providers: { created: 1, updated: 0, skipped: 0 },
      endpoints: { created: 1, updated: 0, skipped: 0 },
      keys: { created: 1, updated: 0, skipped: 0 },
      models: { created: 1, updated: 0, skipped: 0 },
      errors: [],
    },
  }
}

function aggregateImportResult(): AggregateImportResponse {
  return {
    message: 'ok',
    config: configImportResult(),
    users: {
      message: 'ok',
      reauthentication_required: false,
      stats: {
        users: { created: 0, updated: 1, skipped: 0 },
        api_keys: { created: 1, updated: 0, skipped: 0 },
        standalone_keys: { created: 1, updated: 0, skipped: 0 },
        errors: [],
      },
    },
  }
}

beforeEach(() => {
  vi.resetAllMocks()
  importConfigMock.mockResolvedValue(configImportResult())
  importAggregateMock.mockResolvedValue(aggregateImportResult())
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('useConfigExportImport file selection', () => {
  it.each([
    ['without version metadata', {}],
    ['with old version metadata', { version: '2.3' }],
    ['with arbitrary version metadata', { version: { source: 'another-instance' } }],
  ])('accepts config backups %s and submits all their contents', async (_label, metadata) => {
    const state = createState()
    const backup = { ...configBackup, ...metadata }
    await selectConfig(state, backup)
    state.mergeMode.value = 'overwrite'

    await state.confirmImport()

    expect(errorMock).not.toHaveBeenCalled()
    expect(importConfigMock).toHaveBeenCalledWith(
      { ...backup, merge_mode: 'overwrite' },
      { onUploadProgress: expect.any(Function) },
    )
    expect(state.importDialogOpen.value).toBe(false)
    expect(state.importResultDialogOpen.value).toBe(true)
    expect(successMock).toHaveBeenCalledWith('配置导入成功')
  })

  it.each([
    ['without version metadata', {}],
    ['with old version metadata', { version: '1.0' }],
    ['with arbitrary version metadata', { version: 123 }],
  ])('accepts full backups %s at every level', async (_label, metadata) => {
    const state = createState()
    const backup = {
      ...aggregateBackup,
      ...metadata,
      config_data: { ...configBackup, ...metadata },
      user_data: { ...aggregateBackup.user_data, ...metadata },
    }
    await selectAggregate(state, backup)

    await state.confirmImportAggregate()

    expect(errorMock).not.toHaveBeenCalled()
    expect(importAggregateMock).toHaveBeenCalledWith(
      { ...backup, merge_mode: 'skip' },
      { onUploadProgress: expect.any(Function) },
    )
    expect(state.aggregateImportDialogOpen.value).toBe(false)
    expect(state.aggregateImportResultDialogOpen.value).toBe(true)
    expect(successMock).toHaveBeenCalledWith('完整备份导入成功')
  })

  it('previews and forwards an older backup without provider ids, provider_names or admin profiles', async () => {
    const state = createState()
    const backup = {
      version: '1.0',
      exported_at: configBackup.exported_at,
      config_data: {
        ...configBackup,
        version: '2.3',
        providers: [{
          name: 'Source provider',
          endpoints: [{ api_format: 'openai:chat', base_url: 'https://example.test/v1' }],
          api_keys: [{ name: 'Source channel', api_key: 'synthetic-channel-key' }],
          models: [],
        }],
      },
      user_data: {
        version: '1.5',
        exported_at: configBackup.exported_at,
        users: [],
        standalone_keys: [{ id: 'source-key', key: 'synthetic-user-key' }],
        usage_aggregates: { stats_daily: [], stats_user_daily: [], stats_daily_api_key: [] },
      },
    }

    await selectAggregate(state, backup)
    expect(state.aggregateImportPreview.value).toEqual(backup)
    await state.confirmImportAggregate()

    expect(importAggregateMock).toHaveBeenCalledWith(
      { ...backup, merge_mode: 'skip' },
      { onUploadProgress: expect.any(Function) },
    )
    expect(errorMock).not.toHaveBeenCalled()
  })

  it('continues accepting backup files larger than the old file size limits', async () => {
    const state = createState()
    state.handleConfigFileSelect(buildFileInputEvent(JSON.stringify(configBackup), 501 * 1024 * 1024))
    await vi.waitFor(() => expect(state.importDialogOpen.value).toBe(true))

    expect(state.importPreview.value).toEqual(configBackup)
    expect(errorMock).not.toHaveBeenCalled()
  })

  it.each([
    ['{', '解析配置文件失败，请确保是有效的 JSON 文件'],
    ['[]', '无效的配置文件：JSON 顶层必须是对象'],
    [JSON.stringify({ ...configBackup, providers: [null] }), '无效的配置文件：未找到配置导出内容'],
    [JSON.stringify({ ...configBackup, providers: [{ endpoints: {}, api_keys: [], models: [] }] }), '无效的配置文件：未找到配置导出内容'],
  ])('rejects malformed config contents and clears the previous selection (%s)', async (content, message) => {
    const state = createState()
    await selectConfig(state)

    state.handleConfigFileSelect(buildFileInputEvent(content))
    await vi.waitFor(() => expect(errorMock).toHaveBeenCalledWith(message))

    expect(state.importPreview.value).toBeNull()
    expect(state.importDialogOpen.value).toBe(false)
    expect(importConfigMock).not.toHaveBeenCalled()
  })

  it.each([
    ['null', '无效的完整备份文件：JSON 顶层必须是对象'],
    [JSON.stringify(configBackup), '这是配置导出文件，请使用“导入配置数据”'],
    [JSON.stringify({ ...aggregateBackup, config_data: {} }), '无效的完整备份文件：config_data 格式不正确'],
    [JSON.stringify({ ...aggregateBackup, user_data: { ...aggregateBackup.user_data, users: [null] } }), '无效的完整备份文件：user_data 格式不正确'],
    [JSON.stringify({ ...aggregateBackup, user_data: { ...aggregateBackup.user_data, usage_aggregates: [] } }), '无效的完整备份文件：user_data 格式不正确'],
  ])('rejects malformed full backups and clears the previous selection (%s)', async (content, message) => {
    const state = createState()
    await selectAggregate(state)

    state.handleAggregateFileSelect(buildFileInputEvent(content))
    await vi.waitFor(() => expect(errorMock).toHaveBeenCalledWith(message))

    expect(state.aggregateImportPreview.value).toBeNull()
    expect(state.aggregateImportDialogOpen.value).toBe(false)
    expect(importAggregateMock).not.toHaveBeenCalled()
  })
})

describe('useConfigExportImport import results', () => {
  it('keeps a failed config import available for retry without showing a success result', async () => {
    const state = createState()
    await selectConfig(state)
    importConfigMock.mockRejectedValueOnce(new Error('Configuration import failed'))

    await state.confirmImport()

    expect(errorMock).toHaveBeenCalledWith('Configuration import failed')
    expect(successMock).not.toHaveBeenCalled()
    expect(state.importPreview.value).toEqual(configBackup)
    expect(state.importDialogOpen.value).toBe(true)
    expect(state.importResultDialogOpen.value).toBe(false)
    expect(state.importResult.value).toBeNull()
    expect(state.importLoading.value).toBe(false)
    expect(state.importProgress.value).toBeNull()

    await state.confirmImport()
    expect(importConfigMock).toHaveBeenCalledTimes(2)
    expect(state.importDialogOpen.value).toBe(false)
    expect(successMock).toHaveBeenCalledWith('配置导入成功')
  })

  it('keeps a failed full import available for retry without showing a success result', async () => {
    const state = createState()
    await selectAggregate(state)
    importAggregateMock.mockRejectedValueOnce(new Error('Full import failed'))

    await state.confirmImportAggregate()

    expect(errorMock).toHaveBeenCalledWith('Full import failed')
    expect(successMock).not.toHaveBeenCalled()
    expect(state.aggregateImportPreview.value).toEqual(aggregateBackup)
    expect(state.aggregateImportDialogOpen.value).toBe(true)
    expect(state.aggregateImportResultDialogOpen.value).toBe(false)
    expect(state.aggregateImportResult.value).toBeNull()
    expect(state.importAggregateLoading.value).toBe(false)
    expect(state.importAggregateProgress.value).toBeNull()

    await state.confirmImportAggregate()
    expect(importAggregateMock).toHaveBeenCalledTimes(2)
    expect(state.aggregateImportDialogOpen.value).toBe(false)
    expect(successMock).toHaveBeenCalledWith('完整备份导入成功')
  })

  it('does not present legacy config responses with errors as a completed import', async () => {
    const state = createState()
    await selectConfig(state)
    const result = configImportResult()
    result.stats.errors = ['A provider could not be imported']
    importConfigMock.mockResolvedValueOnce(result)

    await state.confirmImport()

    expect(errorMock).toHaveBeenCalledWith('导入失败：服务端返回了错误，请检查目标系统中的数据后重试')
    expect(successMock).not.toHaveBeenCalled()
    expect(state.importDialogOpen.value).toBe(true)
    expect(state.importResultDialogOpen.value).toBe(false)
    expect(state.importResult.value).toBeNull()
  })

  it.each(['config', 'users'] as const)('does not present legacy full responses with %s errors as a completed import', async (part) => {
    const state = createState()
    await selectAggregate(state)
    const result = aggregateImportResult()
    result[part].stats.errors = ['An item could not be imported']
    importAggregateMock.mockResolvedValueOnce(result)

    await state.confirmImportAggregate()

    expect(errorMock).toHaveBeenCalledWith('导入失败：服务端返回了错误，请检查目标系统中的数据后重试')
    expect(successMock).not.toHaveBeenCalled()
    expect(state.aggregateImportDialogOpen.value).toBe(true)
    expect(state.aggregateImportResultDialogOpen.value).toBe(false)
    expect(state.aggregateImportResult.value).toBeNull()
  })
})

describe('useConfigExportImport exports', () => {
  const createObjectUrlMock = vi.fn()
  const revokeObjectUrlMock = vi.fn()

  beforeEach(() => {
    createObjectUrlMock.mockReturnValue('blob:backup')
    vi.stubGlobal('URL', class extends URL {
      static createObjectURL = createObjectUrlMock
      static revokeObjectURL = revokeObjectUrlMock
    })
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {})
  })

  it.each(['config', 'aggregate'] as const)('does not download a %s backup when the request fails', async (scope) => {
    const state = createState()
    if (scope === 'config') {
      exportConfigMock.mockRejectedValueOnce(new Error('Export failed'))
      await state.handleExportConfig()
    } else {
      exportAggregateMock.mockRejectedValueOnce(new Error('Export failed'))
      await state.handleExportAggregate()
    }

    expect(errorMock).toHaveBeenCalledWith('Export failed')
    expect(successMock).not.toHaveBeenCalled()
    expect(createObjectUrlMock).not.toHaveBeenCalled()
    expect(HTMLAnchorElement.prototype.click).not.toHaveBeenCalled()
    expect(state.exportLoading.value).toBe(false)
    expect(state.exportAggregateLoading.value).toBe(false)
  })

  it('downloads the complete response without adding format version metadata', async () => {
    const state = createState()
    exportAggregateMock.mockResolvedValueOnce(aggregateBackup)

    await state.handleExportAggregate()

    expect(createObjectUrlMock).toHaveBeenCalledTimes(1)
    const blob: Blob = createObjectUrlMock.mock.calls[0][0]
    const content = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = () => typeof reader.result === 'string'
        ? resolve(reader.result)
        : reject(new Error('Expected JSON text'))
      reader.onerror = () => reject(reader.error)
      reader.readAsText(blob)
    })
    expect(JSON.parse(content)).toEqual(aggregateBackup)
    expect(HTMLAnchorElement.prototype.click).toHaveBeenCalledTimes(1)
    expect(revokeObjectUrlMock).toHaveBeenCalledWith('blob:backup')
    expect(successMock).toHaveBeenCalledWith('完整备份已导出')
  })
})
