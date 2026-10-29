import { createApp, defineComponent, h, onBeforeUnmount, ref } from 'vue'
import type { TestModelFailoverResponse } from '@/api/endpoints/providers'
import type { RequestTrace } from '@/api/requestTrace'
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

const failureResult: TestModelFailoverResponse = {
  success: false,
  model: 'gpt-5.6',
  provider: { id: 'provider-demo', name: 'Demo Provider' },
  total_candidates: 2,
  total_attempts: 2,
  error: '全部候选均测试失败：production-key-a 返回 502 Bad Gateway；backup-key-b 请求超时',
  attempts: result.attempts.slice(0, 2).map((attempt, index) => ({
    ...attempt,
    status: 'failed',
    status_code: index === 0 ? 502 : null,
    error_message: index === 0 ? '上游返回 502 Bad Gateway' : '请求超时',
  })),
}

const testingTrace: RequestTrace = {
  request_id: 'provider-test-preview',
  request_path: '/v1/responses',
  total_candidates: 4,
  final_status: 'streaming',
  total_latency_ms: 834,
  candidates: [
    {
      id: 'candidate-preview-1',
      request_id: 'provider-test-preview',
      candidate_index: 0,
      retry_index: 0,
      endpoint_name: 'openai:responses',
      key_name: 'production-key-a',
      key_id: 'key-a',
      status: 'failed',
      status_code: 502,
      error_message: '上游返回 502 Bad Gateway',
      latency_ms: 834,
      is_cached: false,
      created_at: '2026-10-29T00:00:00Z',
      extra_data: {
        admin_model_test: {
          mapping_name: 'mapping-a',
          effective_model: 'gpt-5.6-sol',
        },
      },
    },
    {
      id: 'candidate-preview-2',
      request_id: 'provider-test-preview',
      candidate_index: 1,
      retry_index: 0,
      endpoint_name: 'openai:responses',
      key_name: 'backup-key-b',
      key_id: 'key-b',
      status: 'streaming',
      is_cached: false,
      created_at: '2026-10-29T00:00:00Z',
      extra_data: {
        admin_model_test: {
          mapping_name: 'mapping-a',
          effective_model: 'gpt-5.6-sol',
        },
      },
    },
  ],
}

/** 使用正式模型测试弹窗与固定结果数据，供浏览器布局和交互验收。 */
const Preview = defineComponent({
  setup() {
    const requestedMode = query.get('mode')
    const open = ref(true)
    const testing = ref(requestedMode === 'testing' || requestedMode === 'stopped')
    const stopped = ref(requestedMode === 'stopped')
    const hasRunContext = ['testing', 'stopped', 'result', 'failure'].includes(requestedMode ?? '')
    const requestId = ref(hasRunContext ? testingTrace.request_id : null)
    const trace = ref<RequestTrace | null>(testing.value ? testingTrace : null)
    const currentResult = ref<TestModelFailoverResponse | null>(
      requestedMode === 'result'
        ? result
        : requestedMode === 'failure'
          ? failureResult
          : null,
    )
    let resultTimer: ReturnType<typeof setTimeout> | null = null

    function clearResultTimer() {
      if (!resultTimer) return
      clearTimeout(resultTimer)
      resultTimer = null
    }

    function startPreviewTest() {
      clearResultTimer()
      testing.value = true
      stopped.value = false
      currentResult.value = null
      requestId.value = testingTrace.request_id
      trace.value = testingTrace
      resultTimer = setTimeout(() => {
        testing.value = false
        trace.value = null
        currentResult.value = query.get('mode') === 'failure' ? failureResult : result
      }, 1200)
    }

    function stopPreviewTest() {
      clearResultTimer()
      testing.value = false
      stopped.value = true
    }

    onBeforeUnmount(clearResultTimer)

    return () => h('main', {
      class: 'min-h-screen bg-background p-4 text-foreground sm:p-8',
    }, [
      h(ModelTestDialog, {
        open: open.value,
        result: currentResult.value,
        selectingModelName: 'gpt-5.6',
        endpoints: [endpoint],
        selectedEndpoint: endpoint,
        testing: testing.value,
        stopped: stopped.value,
        trace: trace.value,
        requestId: requestId.value,
        modelMappingOptions: [
          { name: 'mapping-a', priority: 10 },
          { name: 'mapping-b', priority: 20 },
        ],
        selectedModelMapping: null,
        keyOptions: [
          { value: 'key-a', label: 'production-key-a' },
          { value: 'key-b', label: 'backup-key-b' },
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
        onStart: startPreviewTest,
        onCancel: stopPreviewTest,
        onClose: () => {
          open.value = false
        },
      }),
    ])
  },
})

createApp(Preview).use(createI18n()).mount('#app')
