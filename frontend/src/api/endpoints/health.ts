import client from '../client'

/**
 * 手动恢复指定 API 格式下被熔断的 Key（渠道管理 Routing 页使用）。
 * 健康监控页面已随个人版裁剪移除，仅保留该恢复操作。
 */
export async function recoverKeyHealth(keyId: string, apiFormat: string) {
  const response = await client.patch(
    `/api/admin/endpoints/health/keys/${keyId}`,
    null,
    { params: { api_format: apiFormat } },
  )
  return response.data as { message?: string }
}
