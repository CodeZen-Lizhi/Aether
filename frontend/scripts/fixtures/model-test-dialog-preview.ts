import { createApp, defineComponent, h } from 'vue'
import type { TestModelFailoverResponse } from '@/api/endpoints/providers'
import ModelTestDialog from '@/features/providers/components/provider-tabs/ModelTestDialog.vue'
import { createI18n, setI18nLocale } from '@/i18n'
import '@/style.css'

const query = new URLSearchParams(location.search)
setI18nLocale(query.get('locale') === 'en' ? 'en-US' : 'zh-CN')

const endpoint = {
  id: 'endpoint-responses',
  api_format: 'openai:responses',
  base_url: 'https://relay.example.com/v1',
  is_active: true,
}

const result: TestModelFailoverResponse = {
  success: true,
  model: 'gpt-5.6',
  provider: { id: 'provider-demo', name: 'Demo Provider' },
  total_candidates: 4,
  total_attempts: 2,
  attempts: [
    {
      candidate_index: 0,
      endpoint_api_format: 'openai:responses',
      endpoint_base_url: endpoint.base_url,
      key_name: 'production-key-a',
      key_id: 'key-a',
      auth_type: 'api_key',
      mapping_name: 'mapping-a',
      effective_model: 'gpt-5.6-sol',
      status: 'failed',
      status_code: 502,
      error_message: '上游返回 502 Bad Gateway',
      latency_ms: 834,
    },
    {
      candidate_index: 1,
      endpoint_api_format: 'openai:responses',
      endpoint_base_url: endpoint.base_url,
      key_name: 'backup-key-b',
      key_id: 'key-b',
      auth_type: 'api_key',
      mapping_name: 'mapping-a',
      effective_model: 'gpt-5.6-sol',
      status: 'success',
      status_code: 200,
      latency_ms: 412,
      response_body: {
        output_text: 'Hello! This is a test message.',
      },
    },
    {
      candidate_index: 2,
      endpoint_api_format: 'openai:responses',
      endpoint_base_url: endpoint.base_url,
      key_name: 'production-key-a',
      key_id: 'key-a',
      auth_type: 'api_key',
      mapping_name: 'mapping-b',
      effective_model: 'gpt-5.6-terra',
      status: 'unused',
    },
    {
      candidate_index: 3,
      endpoint_api_format: 'openai:responses',
      endpoint_base_url: endpoint.base_url,
      key_name: 'backup-key-b',
      key_id: 'key-b',
      auth_type: 'api_key',
      mapping_name: 'mapping-b',
      effective_model: 'gpt-5.6-terra',
      status: 'unused',
    },
  ],
}

/** 使用正式模型测试弹窗与固定结果数据，供浏览器布局和交互验收。 */
const Preview = defineComponent({
  setup() {
    const showResult = query.get('mode') === 'result'
    return () => h('main', {
      class: 'min-h-screen bg-background p-4 text-foreground sm:p-8',
    }, [
      h(ModelTestDialog, {
        open: true,
        result: showResult ? result : null,
        selectingModelName: 'gpt-5.6',
        endpoints: [endpoint],
        selectedEndpoint: endpoint,
        modelMappingOptions: [
          { name: 'mapping-a', priority: 10 },
          { name: 'mapping-b', priority: 20 },
        ],
        selectedModelMapping: null,
        keyOptions: [
          { value: 'key-a', label: 'production-key-a · sk-prod-***91' },
          { value: 'key-b', label: 'backup-key-b · sk-backup-***27' },
        ],
        selectedKeyId: 'auto',
        requestHeadersDraft: '{}',
        requestHeadersResetValue: '{}',
        requestBodyDraft: JSON.stringify({
          model: 'gpt-5.6',
          input: 'Hello! This is a test message.',
        }, null, 2),
        requestBodyResetValue: JSON.stringify({
          model: 'gpt-5.6',
          input: 'Hello! This is a test message.',
        }, null, 2),
      }),
    ])
  },
})

createApp(Preview).use(createI18n()).mount('#app')
