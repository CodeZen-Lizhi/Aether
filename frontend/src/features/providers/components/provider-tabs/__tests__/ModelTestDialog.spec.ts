import { describe, expect, it } from 'vitest'
import { createApp, h, nextTick } from 'vue'

import type { TestModelFailoverResponse } from '@/api/endpoints/providers'
import type { RequestTrace } from '@/api/requestTrace'
import ModelTestDialog from '../ModelTestDialog.vue'

const endpoint = {
  id: 'endpoint-1',
  api_format: 'openai:responses',
  base_url: 'https://api.example/v1',
  is_active: true,
}

describe('ModelTestDialog', () => {
  it('defaults both selectors to automatic and keeps Key before mapping', async () => {
    const container = document.createElement('div')
    document.body.appendChild(container)
    const app = createApp({
      render: () => h(ModelTestDialog, {
        open: true,
        result: null,
        selectingModelName: 'gpt-5.6',
        endpoints: [endpoint],
        selectedEndpoint: endpoint,
        modelMappingOptions: [{ name: 'mapping-a', priority: 10 }],
        selectedModelMapping: null,
        keyOptions: [{ value: 'key-1', label: 'key-1' }],
        selectedKeyId: 'auto',
        requestHeadersDraft: '{}',
        requestBodyDraft: '{}',
      }),
    })
    app.mount(container)
    await nextTick()

    const text = document.body.textContent ?? ''
    const keyIndex = text.indexOf('测试 Key')
    const mappingIndex = text.indexOf('模型映射')

    expect(keyIndex).toBeGreaterThan(-1)
    expect(mappingIndex).toBeGreaterThan(keyIndex)
    expect(text).toContain('自动')
    expect(text).toContain('测试配置')
    expect(text).toContain('测试记录')
    expect(text).toContain('等待开始测试')
    expect(text).toContain('待开始')
    expect(document.body.querySelector('[aria-controls="model-test-config-content"]')?.getAttribute('aria-expanded'))
      .toBe('true')
    expect(text).not.toContain('个候选')
    expect(text).not.toContain('端点兼容且有模型权限')
    expect(text).not.toContain('返回')

    app.unmount()
    container.remove()
  })

  it('renders Key, model, status and unused candidates in result order', async () => {
    const result = {
      success: true,
      model: 'gpt-5.6',
      provider: { id: 'provider-1', name: 'Provider' },
      total_candidates: 2,
      total_attempts: 1,
      attempts: [
        {
          candidate_index: 0,
          endpoint_api_format: 'openai:responses',
          endpoint_base_url: 'https://api.example/v1',
          key_name: 'key-1',
          key_id: 'key-1',
          auth_type: 'api_key',
          mapping_name: 'mapping-a',
          effective_model: 'mapping-a',
          status: 'success',
          status_code: 200,
          latency_ms: 120,
          response_body: { output_text: 'ok' },
        },
        {
          candidate_index: 1,
          endpoint_api_format: 'openai:responses',
          endpoint_base_url: 'https://api.example/v1',
          key_name: 'key-2',
          key_id: 'key-2',
          auth_type: 'api_key',
          mapping_name: 'mapping-b',
          effective_model: 'mapping-b',
          status: 'unused',
        },
      ],
    } as TestModelFailoverResponse
    const container = document.createElement('div')
    document.body.appendChild(container)
    const app = createApp({
      render: () => h(ModelTestDialog, {
        open: true,
        result,
        endpoints: [endpoint],
        selectedEndpoint: endpoint,
      }),
    })
    app.mount(container)
    await nextTick()

    const text = document.body.textContent ?? ''
    const headers = Array.from(document.body.querySelectorAll('th'))
      .map(header => header.textContent?.trim() ?? '')

    expect(headers.slice(0, 6)).toEqual(['#', 'Key', '模型', '状态', '耗时', '详情'])
    expect(text).toContain('mapping-a')
    expect(text).toContain('mapping-b')
    expect(text).toContain('#1')
    expect(text).toContain('未使用')
    expect(text).toContain('测试配置')
    expect(text).toContain('测试记录')
    expect(text).toContain('模型测试成功')
    expect(text).not.toContain('模型测试结果')
    expect(text).not.toContain('返回')
    expect(document.body.querySelector('[aria-controls="model-test-config-content"]')?.getAttribute('aria-expanded'))
      .toBe('false')
    expect(text).not.toContain('候选 2')
    expect(text).not.toContain('无请求体数据')
    expect([...document.body.querySelectorAll('button')].map(button => button.textContent?.trim()))
      .toEqual(expect.arrayContaining(['重新测试', '关闭']))

    app.unmount()
    container.remove()
  })

  it('keeps the same configuration and stops in place while testing', async () => {
    const trace = {
      request_id: 'provider-test-1',
      request_path: '/v1/responses',
      total_candidates: 3,
      final_status: 'streaming',
      total_latency_ms: 240,
      candidates: [
        {
          id: 'candidate-1',
          request_id: 'provider-test-1',
          candidate_index: 0,
          retry_index: 0,
          key_name: 'key-1',
          status: 'streaming',
          is_cached: false,
          created_at: '2026-10-29T00:00:00Z',
        },
      ],
    } as RequestTrace
    let cancelCount = 0
    const container = document.createElement('div')
    document.body.appendChild(container)
    const app = createApp({
      render: () => h(ModelTestDialog, {
        open: true,
        result: null,
        testing: true,
        trace,
        requestId: 'provider-test-1',
        selectingModelName: 'gpt-5.6',
        endpoints: [endpoint],
        selectedEndpoint: endpoint,
        requestHeadersDraft: '{}',
        requestBodyDraft: '{}',
        onCancel: () => {
          cancelCount += 1
        },
      }),
    })
    app.mount(container)
    await nextTick()

    const text = document.body.textContent ?? ''
    const stopButton = [...document.body.querySelectorAll<HTMLButtonElement>('button')]
      .find(button => button.textContent?.trim() === '停止测试')

    expect(text).toContain('测试配置')
    expect(text).toContain('正在验证候选链路')
    expect(text).toContain('请求 ID')
    expect(text).toContain('key-1')
    expect(text).not.toContain('返回')
    expect(document.body.querySelector('[aria-controls="model-test-config-content"]')?.getAttribute('aria-expanded'))
      .toBe('false')
    expect(stopButton).toBeDefined()
    expect(stopButton?.disabled).toBe(false)

    stopButton?.click()
    await nextTick()
    expect(cancelCount).toBe(1)

    app.unmount()
    container.remove()
  })
})
