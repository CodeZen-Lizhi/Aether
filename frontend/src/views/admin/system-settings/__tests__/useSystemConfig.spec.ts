import { beforeEach, describe, expect, it, vi } from 'vitest'

const {
  getAllSystemConfigsMock,
  updateSystemConfigMock,
  getSystemVersionMock,
  desktopStatusMock,
  hasDesktopSessionMock,
} = vi.hoisted(() => ({
  getAllSystemConfigsMock: vi.fn(),
  updateSystemConfigMock: vi.fn(),
  getSystemVersionMock: vi.fn(),
  desktopStatusMock: vi.fn(),
  hasDesktopSessionMock: vi.fn(),
}))

vi.mock('@/api/admin', () => ({
  adminApi: {
    getAllSystemConfigs: getAllSystemConfigsMock,
    updateSystemConfig: updateSystemConfigMock,
    getSystemVersion: getSystemVersionMock,
  },
}))

vi.mock('@/desktop/bridge', () => ({
  desktopApi: {
    status: desktopStatusMock,
  },
}))

vi.mock('@/desktop/session', () => ({
  hasDesktopSession: hasDesktopSessionMock,
}))

vi.mock('@/composables/useToast', () => ({
  useToast: () => ({
    success: vi.fn(),
    error: vi.fn(),
  }),
}))

vi.mock('@/utils/logger', () => ({
  log: {
    error: vi.fn(),
  },
}))

import { useSystemConfig } from '../composables/useSystemConfig'

describe('useSystemConfig', () => {
  beforeEach(() => {
    getAllSystemConfigsMock.mockReset()
    updateSystemConfigMock.mockReset()
    getSystemVersionMock.mockReset()
    desktopStatusMock.mockReset()
    hasDesktopSessionMock.mockReset()
    hasDesktopSessionMock.mockReturnValue(false)
  })

  it('loads config keys in one request and keeps change detection disabled until the baseline is ready', async () => {
    let resolveConfigs!: (value: Array<{ key: string, value: unknown, is_set?: boolean }>) => void
    getAllSystemConfigsMock.mockImplementation(() => new Promise((resolve) => {
      resolveConfigs = resolve
    }))

    const state = useSystemConfig()
    const loadPromise = state.loadSystemConfig()

    expect(getAllSystemConfigsMock).toHaveBeenCalledTimes(1)
    expect(getAllSystemConfigsMock).toHaveBeenCalledWith({ cacheTtlMs: 30_000 })

    state.systemConfig.value.request_record_level = 'headers'
    expect(state.systemConfigLoading.value).toBe(true)
    expect(state.hasLogConfigChanges.value).toBe(false)

    resolveConfigs([
      { key: 'request_record_level', value: 'basic' },
      { key: 'proxy_node_metrics_cleanup_batch_size', value: 5000 },
      { key: 'enable_standard_text_sync_heartbeat', value: false },
    ])
    await loadPromise

    expect(state.systemConfigLoading.value).toBe(false)
    expect(state.systemConfig.value.request_record_level).toBe('basic')
    expect(state.hasLogConfigChanges.value).toBe(false)

    state.systemConfig.value.request_record_level = 'full'
    expect(state.hasLogConfigChanges.value).toBe(true)
  })

  it('loads and saves the standard text sync heartbeat flag as a basic config item', async () => {
    getAllSystemConfigsMock.mockResolvedValue([
      { key: 'enable_standard_text_sync_heartbeat', value: false },
    ])
    updateSystemConfigMock.mockResolvedValue({})

    const state = useSystemConfig()
    await state.loadSystemConfig()

    expect(state.systemConfig.value.enable_standard_text_sync_heartbeat).toBe(false)
    state.systemConfig.value.enable_standard_text_sync_heartbeat = true
    expect(state.hasBasicConfigChanges.value).toBe(true)

    await state.saveBasicConfig()

    expect(updateSystemConfigMock).toHaveBeenCalledWith(
      'enable_standard_text_sync_heartbeat',
      true,
      '标准文本非流式心跳开关：开启后外层 HTTP 状态固定为 200，上游失败写入响应体'
    )
    expect(state.hasBasicConfigChanges.value).toBe(false)
  })

  it('keeps Cyber failover disabled by default and saves the enabled state', async () => {
    getAllSystemConfigsMock.mockResolvedValue([])
    updateSystemConfigMock.mockResolvedValue({})

    const state = useSystemConfig()
    await state.loadSystemConfig()

    expect(state.systemConfig.value.cyber_continue_failover).toBe(false)
    state.systemConfig.value.cyber_continue_failover = true
    expect(state.hasBasicConfigChanges.value).toBe(true)

    await state.saveBasicConfig()

    expect(updateSystemConfigMock).toHaveBeenCalledWith(
      'cyber_continue_failover',
      true,
      'Cyber继续转移开关：开启后在响应内容开始前将Cyber Policy错误按普通错误继续故障转移，可能增加首字等待时间'
    )
    expect(state.hasBasicConfigChanges.value).toBe(false)
  })

  it('uses backend-compatible defaults when config rows have not been persisted yet', async () => {
    getAllSystemConfigsMock.mockResolvedValue([])

    const state = useSystemConfig()
    await state.loadSystemConfig()

    expect(state.systemConfig.value.request_record_level).toBe('full')
    expect(state.systemConfig.value).not.toHaveProperty('site_name')
    expect(state.systemConfig.value).not.toHaveProperty('site_subtitle')
    expect(state.systemConfig.value).not.toHaveProperty('max_request_body_size')
    expect(state.systemConfig.value).not.toHaveProperty('max_response_body_size')
  })

  it('uses the desktop release version when a desktop session is available', async () => {
    hasDesktopSessionMock.mockReturnValue(true)
    desktopStatusMock.mockResolvedValue({ version: '0.1.7' })

    const state = useSystemConfig()
    await state.loadSystemVersion()

    expect(desktopStatusMock).toHaveBeenCalledOnce()
    expect(getSystemVersionMock).not.toHaveBeenCalled()
    expect(state.systemVersion.value).toBe('0.1.7')
  })

  it('falls back to the gateway version when desktop status IPC fails', async () => {
    hasDesktopSessionMock.mockReturnValue(true)
    desktopStatusMock.mockRejectedValue(new Error('native status unavailable'))
    getSystemVersionMock.mockResolvedValue({ version: 'gateway-version' })

    const state = useSystemConfig()
    await state.loadSystemVersion()

    expect(desktopStatusMock).toHaveBeenCalledOnce()
    expect(getSystemVersionMock).toHaveBeenCalledOnce()
    expect(state.systemVersion.value).toBe('gateway-version')
  })

  it('uses the gateway version without probing desktop IPC outside a desktop session', async () => {
    getSystemVersionMock.mockResolvedValue({ version: 'gateway-version' })

    const state = useSystemConfig()
    await state.loadSystemVersion()

    expect(hasDesktopSessionMock).toHaveBeenCalledOnce()
    expect(desktopStatusMock).not.toHaveBeenCalled()
    expect(getSystemVersionMock).toHaveBeenCalledOnce()
    expect(state.systemVersion.value).toBe('gateway-version')
  })
})
