import { invoke, isTauri } from '@tauri-apps/api/core'

export type GatewayPhase = 'setup' | 'starting' | 'running' | 'stopping' | 'stopped' | 'failed'

export interface DesktopStatus {
  phase: GatewayPhase
  configured: boolean
  port: number
  gateway_url: string
  data_dir: string
  log_dir: string
  autostart: boolean
  pid: number | null
  error: string | null
  version: string
}

const phases: readonly GatewayPhase[] = ['setup', 'starting', 'running', 'stopping', 'stopped', 'failed']
export const MIN_GATEWAY_PORT = 1024
export const MAX_GATEWAY_PORT = 65535
export const MAX_LOG_LINES = 100

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function isPort(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value)
    && value >= MIN_GATEWAY_PORT && value <= MAX_GATEWAY_PORT
}

/** Decode IPC once, before malformed or mismatched native data reaches the UI. */
function decodeStatus(value: unknown): DesktopStatus {
  if (!isRecord(value)
    || !phases.includes(value.phase as GatewayPhase)
    || typeof value.configured !== 'boolean'
    || !isPort(value.port)
    || typeof value.gateway_url !== 'string'
    || ![ `http://127.0.0.1:${value.port}`, `http://127.0.0.1:${value.port}/` ].includes(String(value.gateway_url))
    || typeof value.data_dir !== 'string'
    || typeof value.log_dir !== 'string'
    || typeof value.autostart !== 'boolean'
    || !(value.pid === null || (typeof value.pid === 'number' && Number.isSafeInteger(value.pid) && value.pid > 0))
    || !(value.error === null || typeof value.error === 'string')
    || typeof value.version !== 'string') {
    throw new Error('客户端返回了无法识别的状态，请退出并重新打开 Aether。')
  }
  return value as unknown as DesktopStatus
}

async function call(command: string, args?: Record<string, unknown>): Promise<unknown> {
  if (!isTauri()) {
    throw new Error('请在 Aether macOS 客户端中打开此页面。浏览器无法管理本机网关。')
  }
  return invoke<unknown>(command, args)
}

async function statusCommand(command: string, args?: Record<string, unknown>): Promise<DesktopStatus> {
  return decodeStatus(await call(command, args))
}

async function voidCommand(command: string): Promise<void> {
  await call(command)
}

export const desktopApi = {
  status: () => statusCommand('desktop_status'),
  start: () => statusCommand('desktop_start'),
  stop: () => statusCommand('desktop_stop'),
  restart: () => statusCommand('desktop_restart'),
  setPort: (port: number) => statusCommand('desktop_set_port', { port }),
  setAutostart: (enabled: boolean) => statusCommand('desktop_set_autostart', { enabled }),
  openDashboard: () => voidCommand('desktop_open_dashboard'),
  openDataDir: () => voidCommand('desktop_open_data_dir'),
  openLogDir: () => voidCommand('desktop_open_log_dir'),
  quit: () => voidCommand('desktop_quit'),
  async logs(): Promise<string[]> {
    const value = await call('desktop_logs')
    if (!Array.isArray(value) || !value.every(line => typeof line === 'string')) {
      throw new Error('无法读取诊断日志，请重试或打开日志目录。')
    }
    // The host filters secrets. Keep the rendered diagnostics bounded as well.
    return value.slice(-MAX_LOG_LINES).map(line => line.slice(0, 4096))
  },
}

export function describeDesktopError(error: unknown): string {
  const message = error instanceof Error ? error.message : typeof error === 'string' ? error : ''
  return message.trim().slice(0, 2000) || '操作未完成，请重试或查看诊断日志。'
}
