import client from '../client'

/**
 * 手动恢复 Key 的本地健康和熔断状态。
 * 传入 apiFormat 时仅恢复指定格式；省略时恢复该 Key 的全部格式。
 */
export async function recoverKeyHealth(keyId: string, apiFormat?: string) {
  const params = apiFormat ? { api_format: apiFormat } : undefined
  const response = await client.patch(
    `/api/admin/endpoints/health/keys/${keyId}`,
    null,
    { params },
  )
  return response.data as { message?: string }
}
