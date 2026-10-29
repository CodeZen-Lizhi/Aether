import { describe, expect, it } from 'vitest'
import { createApp, h, nextTick } from 'vue'

import type { TestModelFailoverResponse } from '@/api/endpoints/providers'
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
    expect(text).not.toContain('个候选')
    expect(text).not.toContain('端点兼容且有模型权限')

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
    expect(text).not.toContain('候选 2')
    expect(text).not.toContain('无请求体数据')

    app.unmount()
    container.remove()
  })
})
