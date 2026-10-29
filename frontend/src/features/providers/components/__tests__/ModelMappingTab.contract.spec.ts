import { describe, expect, it } from 'vitest'
import { createSSRApp, h } from 'vue'
import { renderToString } from '@vue/server-renderer'

import type {
  Model,
  ProviderMappingPreviewResponse,
  ProviderWithEndpointsSummary,
} from '@/api/endpoints'
import ModelMappingTab from '../provider-tabs/ModelMappingTab.vue'

const provider = {
  id: 'provider-demo',
  name: 'Demo Provider',
  provider_type: 'custom',
  is_active: true,
  active_keys: 0,
  api_formats: [],
} as unknown as ProviderWithEndpointsSummary

describe('ModelMappingTab response contracts', () => {
  it('keeps the module visible when a legacy or malformed preview reaches the component', async () => {
    const app = createSSRApp({
      render: () => h(ModelMappingTab, {
        provider,
        models: [],
        mappingPreview: {
          message: '演示模式：该接口暂未模拟',
          demo_mode: true,
        } as unknown as ProviderMappingPreviewResponse,
        loading: false,
      }),
    })

    const html = await renderToString(app)

    expect(html).toContain('模型映射')
    expect(html).toContain('暂无模型映射')
    expect(html).toContain('批量映射')
    expect(html).not.toContain('单条添加')
    expect(html).not.toContain('测试映射')
  })

  it('hides every endpoint and request scope label in the mapping list', async () => {
    const models = [
      {
        id: 'default-model',
        provider_model_name: 'default-provider-model',
        global_model_display_name: 'Default mapping',
        provider_model_mappings: [{ name: 'default-upstream', priority: 1 }],
      },
      {
        id: 'special-model',
        provider_model_name: 'special-provider-model',
        global_model_display_name: 'Special mapping',
        provider_model_mappings: [{
          name: 'special-upstream',
          priority: 1,
          endpoint_ids: ['responses-endpoint'],
          operations: ['compact'],
        }],
      },
    ] as unknown as Model[]
    const app = createSSRApp({
      render: () => h(ModelMappingTab, {
        provider,
        models,
        mappingPreview: null,
        loading: false,
      }),
    })

    const html = await renderToString(app)

    expect(html).toContain('Default mapping')
    expect(html).toContain('Special mapping')
    expect(html).not.toContain('全部端点')
    expect(html).not.toContain('全部请求')
    expect(html).not.toContain('Responses')
    expect(html).not.toContain('仅会话压缩')
    expect(html).not.toContain('适用端点')
    expect(html).not.toContain('适用请求')
    expect(html).not.toContain('测试映射')
  })
})
