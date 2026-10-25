import { createApp, defineComponent, h, ref } from 'vue'
import ModelMappingDialog, { type AliasGroup } from '@/features/providers/components/ModelMappingDialog.vue'
import BatchModelMappingDialog from '@/features/providers/components/BatchModelMappingDialog.vue'
import ModelMappingTab from '@/features/providers/components/provider-tabs/ModelMappingTab.vue'
import { createI18n } from '@/i18n'
import '@/style.css'

const models = [{
  id: 'model-primary',
  provider_model_name: 'primary-provider-model',
  global_model_display_name: '主模型映射',
  provider_model_mappings: [{ name: 'primary-upstream', priority: 1 }],
}, {
  id: 'model-secondary',
  provider_model_name: 'secondary-provider-model',
  global_model_display_name: '备用模型映射',
  provider_model_mappings: [{ name: 'secondary-upstream', priority: 1 }],
}]
const provider = {
  id: 'provider-preview',
  name: 'Preview Provider',
  is_active: true,
  active_keys: 1,
  api_formats: ['openai:responses'],
}
const defaultGroup: AliasGroup = {
  model: models[0] as never,
  apiFormatsKey: '',
  apiFormats: [],
  endpointIdsKey: '',
  endpointIds: [],
  operationsKey: '',
  operations: [],
  aliases: models[0].provider_model_mappings,
}

// 验收预览只组合真实组件与固定业务数据，不承载产品逻辑。
const Preview = defineComponent({
  setup() {
    const mode = ref(new URLSearchParams(location.search).get('mode') ?? 'list')
    const singleOpen = ref(mode.value === 'single')
    const batchOpen = ref(mode.value === 'batch')
    return () => h('main', { class: 'min-h-screen bg-background p-6 text-foreground' }, [
      h('div', { class: 'mb-4 flex gap-2' }, [
        h('button', { id: 'show-list', onClick: () => { mode.value = 'list' } }, '映射列表'),
        h('button', { id: 'show-single', onClick: () => { singleOpen.value = true } }, '单条编辑'),
        h('button', { id: 'show-batch', onClick: () => { batchOpen.value = true } }, '批量编辑'),
      ]),
      mode.value === 'list' ? h(ModelMappingTab, {
        provider,
        models,
        providerKeys: [],
        mappingPreview: null,
        loading: false,
      }) : null,
      h(ModelMappingDialog, {
        open: singleOpen.value,
        providerId: provider.id,
        models,
        editingGroup: defaultGroup,
        hasAutoFetchKey: true,
        'onUpdate:open': (value: boolean) => { singleOpen.value = value },
      }),
      h(BatchModelMappingDialog, {
        open: batchOpen.value,
        providerId: provider.id,
        models,
        hasAutoFetchKey: true,
        'onUpdate:open': (value: boolean) => { batchOpen.value = value },
      }),
    ])
  },
})

createApp(Preview).use(createI18n()).mount('#app')
