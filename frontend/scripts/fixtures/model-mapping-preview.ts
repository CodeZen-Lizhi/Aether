import { createApp, defineComponent, h, ref } from 'vue'
import BatchModelMappingDialog from '@/features/providers/components/BatchModelMappingDialog.vue'
import ModelMappingTab from '@/features/providers/components/provider-tabs/ModelMappingTab.vue'
import { createI18n, setI18nLocale } from '@/i18n'
import client from '@/api/client'
import { adminApi } from '@/api/admin'
import '@/style.css'

const previewLocale = new URLSearchParams(location.search).get('locale') === 'en' ? 'en-US' : 'zh-CN'
setI18nLocale(previewLocale)

const models = [
  {
    id: 'model-gpt-56-sol',
    provider_model_name: 'gpt-5.6-sol',
    global_model_name: 'gpt-5.6-sol',
    global_model_display_name: 'GPT-5.6 Sol',
    provider_model_mappings: [
      { name: 'gpt-5.6-sol', priority: 1 },
      { name: 'gpt-5.6-pro', priority: 2 },
      { name: 'gpt-5.6-mini', priority: 3 },
    ],
  },
  {
    id: 'model-gpt-6-astra',
    provider_model_name: 'gpt-6-astra',
    global_model_name: 'gpt-6-astra',
    global_model_display_name: 'GPT-6 Astra',
    provider_model_mappings: [{ name: 'gpt-6.1-sol', priority: 1 }],
  },
  {
    id: 'model-gpt-6-luna',
    provider_model_name: 'gpt-6-luna',
    global_model_name: 'gpt-6-luna',
    global_model_display_name: 'GPT-6 Luna',
    provider_model_mappings: [{ name: 'gpt-6.1-sol', priority: 1 }],
  },
  {
    id: 'model-gpt-61-sol',
    provider_model_name: 'gpt-6.1-sol',
    global_model_name: 'gpt-6.1-sol',
    global_model_display_name: 'GPT-6.1 Sol',
    provider_model_mappings: [],
  },
]
const provider = {
  id: 'provider-shenwen-ai',
  name: '神稳AI',
  is_active: true,
  active_keys: 1,
  api_formats: ['openai:responses', 'openai:responses:compact', 'openai:search'],
}
// 上游模型请求由固定业务数据拦截，避免预览修改真实供应商配置。
adminApi.queryProviderModels = async () => ({
  success: true,
  data: {
    models: [
      { id: 'gpt-5.6-sol', api_formats: ['openai:responses'] },
      { id: 'gpt-5.6-pro', api_formats: ['openai:responses'] },
      { id: 'gpt-5.6-mini', api_formats: ['openai:responses'] },
      { id: 'gpt-6-astra', api_formats: ['openai:responses'] },
      { id: 'gpt-6-luna', api_formats: ['openai:responses'] },
      { id: 'gpt-6.1-sol', api_formats: ['openai:responses'] },
      { id: 'gpt-6.1-pro', api_formats: ['openai:responses'] },
      { id: 'gpt-6.1-flash', api_formats: ['openai:responses'] },
      { id: 'claude-5-sonnet', api_formats: ['claude:messages'] },
      { id: 'gemini-4-pro', api_formats: ['gemini:generate_content'] },
      { id: 'workbuddy/kimi-k3.1-pro', api_formats: ['openai:responses'] },
    ],
  },
})

// 预览只记录保存载荷，防止误触按钮时写入真实本地网关。
const previewWrites: Array<{ url: string; data: unknown }> = []
client.patch = async (url, data) => {
  previewWrites.push({ url, data })
  return {
    data: models.find(model => url.endsWith(model.id)) ?? models[0],
    status: 200,
    statusText: 'OK',
    headers: {},
    config: { headers: {} },
  }
}
client.get = async url => {
  if (url.includes('/endpoints')) {
    return {
      data: provider.api_formats.map(apiFormat => ({
        id: `endpoint-${apiFormat}`,
        provider_id: provider.id,
        provider_name: provider.name,
        api_format: apiFormat,
        base_url: 'https://fixture.invalid',
        is_active: true,
        total_keys: 1,
        active_keys: 1,
      })),
    }
  }
  throw new Error(`未配置 fixture 请求: ${url}`)
}
;(window as Window & { __modelMappingPreviewWrites?: typeof previewWrites })
  .__modelMappingPreviewWrites = previewWrites

// 验收预览组合真实组件与固定业务数据，不写数据库。
const Preview = defineComponent({
  setup() {
    const mode = ref(new URLSearchParams(location.search).get('mode') ?? 'list')
    const batchOpen = ref(mode.value === 'batch')
    return () => h('main', { class: 'min-h-screen bg-background p-6 text-foreground' }, [
      h('div', { class: 'mb-4 flex gap-2' }, [
        h('button', { id: 'show-list', onClick: () => { mode.value = 'list' } }, '映射列表'),
        h('button', { id: 'show-batch', onClick: () => { batchOpen.value = true } }, '管理映射'),
      ]),
      mode.value === 'list' ? h(ModelMappingTab, {
        provider,
        models,
        providerKeys: [],
        mappingPreview: null,
        loading: false,
      }) : null,
      h(BatchModelMappingDialog, {
        open: batchOpen.value,
        providerId: provider.id,
        models,
        canFetchUpstreamModels: true,
        'onUpdate:open': (value: boolean) => { batchOpen.value = value },
      }),
    ])
  },
})

createApp(Preview).use(createI18n()).mount('#app')
