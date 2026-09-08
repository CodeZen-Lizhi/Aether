import { beforeEach, describe, expect, it, vi } from 'vitest'
import { ref } from 'vue'

const { errorMock, successMock } = vi.hoisted(() => ({
  errorMock: vi.fn(),
  successMock: vi.fn(),
}))

vi.mock('@/composables/useToast', () => ({
  useToast: () => ({
    error: errorMock,
    success: successMock,
  }),
}))

vi.mock('@/api/admin', async (importOriginal) => ({
  ...await importOriginal<typeof import('@/api/admin')>(),
  adminApi: {},
}))

import { CONFIG_EXPORT_VERSION, USERS_EXPORT_VERSION, AGGREGATE_EXPORT_VERSION } from '@/api/admin'
import { useConfigExportImport } from '../composables/useConfigExportImport'
import type { SystemConfig } from '../composables/useSystemConfig'

function buildFileInputEvent(file: File): Event {
  return {
    target: {
      files: [file],
      value: 'selected.json',
    },
  } as unknown as Event
}

function makeSizedFile(content: string, size: number): File {
  const file = new File([content], 'config.json', { type: 'application/json' })
  Object.defineProperty(file, 'size', { value: size })
  return file
}

const configBackup = {
  version: CONFIG_EXPORT_VERSION,
  exported_at: '2026-09-08T00:00:00.000Z',
  global_models: [],
  providers: [],
  proxy_nodes: [],
  system_configs: [],
}

const aggregateBackup = {
  version: AGGREGATE_EXPORT_VERSION,
  exported_at: configBackup.exported_at,
  config_data: configBackup,
  user_data: {
    version: USERS_EXPORT_VERSION,
    exported_at: configBackup.exported_at,
    provider_names: {},
    users: [{ id: 'admin-source', username: 'admin', role: 'admin', api_keys: [], preferences: null }],
    standalone_keys: [],
    usage_aggregates: { stats_daily: [], stats_user_daily: [], stats_daily_api_key: [] },
  },
}

describe('useConfigExportImport file selection', () => {
  beforeEach(() => {
    errorMock.mockReset()
    successMock.mockReset()
  })

  it('accepts current config exports larger than the old file size limits', async () => {
    const state = useConfigExportImport(ref({ site_name: 'Aether' }) as unknown as { value: SystemConfig })
    const file = makeSizedFile(JSON.stringify(configBackup), 501 * 1024 * 1024)

    state.handleConfigFileSelect(buildFileInputEvent(file))
    await vi.waitFor(() => expect(state.importDialogOpen.value).toBe(true))

    expect(errorMock).not.toHaveBeenCalled()
    expect(state.importPreview.value).toEqual(configBackup)
  })

  it('rejects old config exports and clears a previously selected valid file', async () => {
    const state = useConfigExportImport(ref({ site_name: 'Aether' }) as unknown as { value: SystemConfig })
    state.handleConfigFileSelect(buildFileInputEvent(makeSizedFile(JSON.stringify(configBackup), 100)))
    await vi.waitFor(() => expect(state.importDialogOpen.value).toBe(true))

    state.handleConfigFileSelect(buildFileInputEvent(makeSizedFile(
      JSON.stringify({ ...configBackup, version: '2.3' }), 100,
    )))
    await vi.waitFor(() => expect(errorMock).toHaveBeenCalledWith('仅支持当前版本导出的配置文件，请重新导出'))

    expect(state.importPreview.value).toBeNull()
    expect(state.importDialogOpen.value).toBe(false)
  })

  it('accepts current full backups and rejects an old nested user format', async () => {
    const state = useConfigExportImport(ref({ site_name: 'Aether' }) as unknown as { value: SystemConfig })
    state.handleAggregateFileSelect(buildFileInputEvent(makeSizedFile(
      JSON.stringify(aggregateBackup), 501 * 1024 * 1024,
    )))
    await vi.waitFor(() => expect(state.aggregateImportDialogOpen.value).toBe(true))
    expect(state.aggregateImportPreview.value).toEqual(aggregateBackup)
    expect(errorMock).not.toHaveBeenCalled()

    state.handleAggregateFileSelect(buildFileInputEvent(makeSizedFile(JSON.stringify({
      ...aggregateBackup,
      user_data: { ...aggregateBackup.user_data, version: '1.5' },
    }), 100)))
    await vi.waitFor(() => expect(errorMock).toHaveBeenCalledWith('无效的完整备份文件：user_data 格式不正确'))
    expect(state.aggregateImportPreview.value).toBeNull()
    expect(state.aggregateImportDialogOpen.value).toBe(false)
  })
})
