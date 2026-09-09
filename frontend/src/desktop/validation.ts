import { MAX_GATEWAY_PORT, MIN_GATEWAY_PORT } from './bridge'

export function validatePort(port: string): string {
  const value = Number(port)
  if (!/^\d+$/.test(port) || !Number.isInteger(value) || value < MIN_GATEWAY_PORT || value > MAX_GATEWAY_PORT) {
    return '请输入 1024–65535 之间的整数端口。'
  }
  return ''
}
