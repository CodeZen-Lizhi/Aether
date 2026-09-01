import apiClient from './client'

export interface ProxyNodeRemoteConfig {
  node_name?: string
  allowed_ports?: number[]
  log_level?: string
  heartbeat_interval?: number
  scheduling_state?: ProxyNodeSchedulingState | null
  upgrade_to?: string | null
}

export type ProxyNodeSchedulingState = 'active' | 'draining' | 'cordoned'

export interface ProxyNode {
  id: string
  name: string
  ip: string
  port: number
  region: string | null
  status: 'online' | 'offline'
  is_manual: boolean
  tunnel_mode: boolean
  tunnel_connected: boolean
  tunnel_connected_at: string | null
  // 手动节点专用字段。列表接口返回脱敏密码，详情接口返回明文密码。
  proxy_url?: string
  proxy_username?: string
  proxy_password?: string
  // 硬件信息（aether-tunnel 节点）
  hardware_info: Record<string, unknown> | null
  estimated_max_concurrency: number | null
  // 远程配置（aether-tunnel 节点）
  remote_config: ProxyNodeRemoteConfig | null
  config_version: number
  registered_by: string | null
  last_heartbeat_at: string | null
  heartbeat_interval: number
  active_connections: number
  total_requests: number
  avg_latency_ms: number | null
  failed_requests: number
  dns_failures: number
  stream_errors: number
  proxy_metadata: Record<string, unknown> | null
  created_at: string
  updated_at: string
}

export interface ProxyNodeListResponse {
  items: ProxyNode[]
  total: number
  skip: number
  limit: number
}

export interface ManualProxyNodeCreateRequest {
  name: string
  proxy_url: string
  username?: string
  password?: string
  region?: string
}

export interface ManualProxyNodeUpdateRequest {
  name?: string
  proxy_url?: string
  username?: string
  password?: string
  region?: string
}

export interface ProxyNodeTestResult {
  success: boolean
  latency_ms: number | null
  exit_ip: string | null
  error: string | null
  probe_url: string
  timeout_secs: number
}

export const proxyNodesApi = {
  async listProxyNodes(params?: { status?: string; skip?: number; limit?: number }): Promise<ProxyNodeListResponse> {
    const response = await apiClient.get<ProxyNodeListResponse>('/api/admin/proxy-nodes', { params })
    return response.data
  },

  async getNode(nodeId: string): Promise<{ node: ProxyNode }> {
    const response = await apiClient.get<{ node: ProxyNode }>(`/api/admin/proxy-nodes/${nodeId}`)
    return response.data
  },

  async createManualNode(data: ManualProxyNodeCreateRequest): Promise<{ node_id: string; node: ProxyNode }> {
    const response = await apiClient.post<{ node_id: string; node: ProxyNode }>('/api/admin/proxy-nodes/manual', data)
    return response.data
  },

  async updateManualNode(nodeId: string, data: ManualProxyNodeUpdateRequest): Promise<{ node_id: string; node: ProxyNode }> {
    const response = await apiClient.patch<{ node_id: string; node: ProxyNode }>(`/api/admin/proxy-nodes/${nodeId}`, data)
    return response.data
  },

  async deleteProxyNode(nodeId: string): Promise<{
    message: string
    node_id: string
    cleared_system_proxy: boolean
    cleared_external_models_proxy: boolean
  }> {
    const response = await apiClient.delete<{
      message: string
      node_id: string
      cleared_system_proxy: boolean
      cleared_external_models_proxy: boolean
    }>(`/api/admin/proxy-nodes/${nodeId}`)
    return response.data
  },

  async testProxyUrl(data: { proxy_url: string; username?: string; password?: string }): Promise<ProxyNodeTestResult> {
    const response = await apiClient.post<ProxyNodeTestResult>('/api/admin/proxy-nodes/test-url', data)
    return response.data
  },
}
