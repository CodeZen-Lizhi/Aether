import type { ProxyNodeTestResult } from '@/api/proxy-nodes'

export function formatProxyTestSuccessText(
  result: Pick<ProxyNodeTestResult, 'latency_ms' | 'exit_ip'>,
): string {
  const parts: string[] = []
  if (result.latency_ms != null) parts.push(`延迟 ${result.latency_ms}ms`)
  if (result.exit_ip) parts.push(`出口 IP ${result.exit_ip}`)
  return parts.length ? `测试通过：${parts.join(' · ')}` : '测试通过'
}
