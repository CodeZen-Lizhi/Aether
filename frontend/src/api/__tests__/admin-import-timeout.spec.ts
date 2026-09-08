import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { AggregateImportRequest, ConfigImportRequest } from '@/api/admin'

const { postMock } = vi.hoisted(() => ({
  postMock: vi.fn(),
}))

vi.mock('@/api/client', () => ({
  default: {
    post: postMock,
  },
}))

import { adminApi, CONFIG_EXPORT_VERSION, USERS_EXPORT_VERSION, AGGREGATE_EXPORT_VERSION } from '@/api/admin'

const SYSTEM_DATA_IMPORT_TIMEOUT_MS = 10 * 60 * 1000

describe('adminApi system data import timeouts', () => {
  beforeEach(() => {
    postMock.mockReset()
    postMock.mockResolvedValue({ data: {} })
  })

  it('uses a long timeout for config imports', async () => {
    const payload = {
      version: CONFIG_EXPORT_VERSION,
      exported_at: '2026-01-01T00:00:00.000Z',
      global_models: [],
      providers: [],
      proxy_nodes: [],
      system_configs: [],
      merge_mode: 'skip',
    } satisfies ConfigImportRequest

    await adminApi.importConfig(payload)

    expect(postMock).toHaveBeenCalledWith(
      '/api/admin/system/config/import',
      payload,
      { timeout: SYSTEM_DATA_IMPORT_TIMEOUT_MS }
    )
  })

  it('uses a long timeout for aggregate imports', async () => {
    const payload = {
      version: AGGREGATE_EXPORT_VERSION,
      exported_at: '2026-01-01T00:00:00.000Z',
      config_data: {
        version: CONFIG_EXPORT_VERSION,
        exported_at: '2026-01-01T00:00:00.000Z',
        global_models: [],
        providers: [],
        proxy_nodes: [],
        system_configs: [],
      },
      user_data: {
        version: USERS_EXPORT_VERSION,
        exported_at: '2026-01-01T00:00:00.000Z',
        users: [],
        provider_names: {},
        standalone_keys: [],
        usage_aggregates: { stats_daily: [], stats_user_daily: [], stats_daily_api_key: [] },
      },
      merge_mode: 'skip',
    } satisfies AggregateImportRequest

    await adminApi.importAggregateData(payload)

    expect(postMock).toHaveBeenCalledWith(
      '/api/admin/system/data/import',
      payload,
      { timeout: SYSTEM_DATA_IMPORT_TIMEOUT_MS }
    )
  })
})
