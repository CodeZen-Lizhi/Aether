import { describe, expect, it } from 'vitest'

import { parseApiError, parseUpstreamModelError } from '@/utils/errorParser'

describe('errorParser', () => {
  it('normalizes reused refresh token errors from legacy string details', () => {
    const error = {
      response: {
        data: {
          detail: "token refresh 失败: {'message': 'Your refresh token has already been used to generate a new access token. Please try signing in again.', 'type': 'invalid_request_error', 'param': None, 'code': 'refresh_token_reused'}",
        },
      },
    }

    expect(parseApiError(error, 'Token 刷新失败')).toBe(
      'Token 刷新失败：refresh_token 已被使用并轮换，请重新登录授权',
    )
  })

  it('keeps normalized Chinese refresh failures intact', () => {
    const error = {
      response: {
        data: {
          detail: 'Token 刷新失败：refresh_token 已被使用并轮换，请重新登录授权',
        },
      },
    }

    expect(parseApiError(error, 'Token 刷新失败')).toBe(
      'Token 刷新失败：refresh_token 已被使用并轮换，请重新登录授权',
    )
  })

  it('normalizes expired refresh token errors', () => {
    const error = {
      response: {
        data: {
          detail: '{"error":{"message":"Could not validate your refresh token. Please try signing in again.","type":"invalid_request_error","code":"refresh_token_expired"}}',
        },
      },
    }

    expect(parseApiError(error, 'Token 刷新失败')).toBe(
      'Token 刷新失败：refresh_token 无效、已过期或已撤销，请重新登录授权',
    )
  })
})

describe('parseUpstreamModelError', () => {
  it('preserves long errors and every key or format failure', () => {
    const error = `Key 渠道 A: openai:responses: failed to execute upstream request: error sending request for url (https://upstream.example.test/${'long-path-'.repeat(20)}/models): connection refused; Key 渠道 B: claude:chat: certificate verification failed`

    expect(parseUpstreamModelError(error)).toBe(error)
  })

  it.each([
    ['HTTP 401: {"error":{"message":"invalid api key: account disabled; contact the administrator","code":"account_disabled"}}', '密钥无效'],
    [`HTTP 503: {"error":{"message":"${'Service unavailable. '.repeat(15)}request-id: last-detail"}}`, '上游服务暂时不可用'],
    ['HTTP 403: upstream gateway rejected this account; request-id: final-detail', '密钥权限不足'],
    ['HTTP 418: {"detail":{"reason":"unsupported account","request_id":"last-detail"}}', 'HTTP 418'],
    ['Request error: connection timeout to https://upstream.example.test/models; retry exhausted', '请求超时'],
    ['Unknown API format: custom-format; supported formats: openai:chat', '不支持的 API 格式'],
  ])('keeps the complete upstream response alongside a useful hint: %s', (error, hint) => {
    const message = parseUpstreamModelError(error)

    expect(message).toContain(hint)
    expect(message).toContain(error)
  })
})
