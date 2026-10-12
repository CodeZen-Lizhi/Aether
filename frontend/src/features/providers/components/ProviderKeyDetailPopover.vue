<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child>
      <Button
        variant="ghost"
        size="icon"
        class="h-7 w-7"
        :title="legacyT('密钥详情')"
        :aria-label="legacyT('密钥详情')"
        data-testid="provider-key-detail-trigger"
        @click.stop
      >
        <Info class="h-3.5 w-3.5" />
      </Button>
    </PopoverTrigger>

    <PopoverContent
      class="w-80 p-0"
      side="bottom"
      align="end"
    >
      <div class="max-h-[60vh] overflow-y-auto">
        <div class="border-b border-border/60 px-3 py-2">
          <p class="text-xs font-medium">
            {{ apiKey.name || legacyT('未命名密钥') }}
          </p>
          <p class="mt-0.5 text-[10px] text-muted-foreground">
            {{ maskedSecretLabel || apiKey.api_key_masked || '****' }}
          </p>
        </div>

        <!-- 备注保留换行并完整展示，不做截断。 -->
        <div class="border-b border-border/60 px-3 py-2">
          <p class="text-[10px] font-medium text-muted-foreground">
            {{ legacyT('备注') }}
          </p>
          <p
            v-if="apiKey.note"
            class="mt-1 whitespace-pre-wrap break-words text-xs leading-relaxed text-foreground"
            data-testid="provider-key-detail-note"
          >
            {{ apiKey.note }}
          </p>
          <p
            v-else
            class="mt-1 text-xs text-muted-foreground"
          >
            {{ legacyT('暂无备注') }}
          </p>
        </div>

        <dl class="space-y-1.5 px-3 py-2 text-[11px]">
          <div
            v-for="row in detailRows"
            :key="row.label"
            class="flex items-start gap-2"
          >
            <dt class="w-20 shrink-0 text-muted-foreground">
              {{ row.label }}
            </dt>
            <dd class="min-w-0 flex-1 break-words text-foreground">
              {{ row.value }}
            </dd>
          </div>
        </dl>
      </div>

      <div class="border-t border-border/60 px-3 py-2 text-right">
        <Button
          variant="outline"
          size="sm"
          class="h-7 px-2 text-xs"
          data-testid="provider-key-detail-edit"
          @click="handleEdit"
        >
          {{ legacyT('编辑密钥') }}
        </Button>
      </div>
    </PopoverContent>
  </Popover>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { Info } from 'lucide-vue-next'
import Button from '@/components/ui/button.vue'
import { Popover, PopoverTrigger, PopoverContent } from '@/components/ui'
import { useI18n } from '@/i18n'
import { formatDate } from '@/utils/format'
import { formatApiFormatShort } from '@/api/endpoints/types/api-format'
import type { EndpointAPIKey } from '@/api/endpoints'

const props = withDefaults(defineProps<{
  apiKey: EndpointAPIKey
  maskedSecretLabel?: string
}>(), {
  maskedSecretLabel: '',
})

const emit = defineEmits<{
  (e: 'edit'): void
}>()

const { legacyT } = useI18n()

const open = ref(false)

/** 点击“编辑密钥”后先收起浮层，避免与编辑对话框重叠。 */
function handleEdit() {
  open.value = false
  emit('edit')
}

/** 详情浮层只展示已存在的字段，缺省项不占行，避免大量空值噪音。 */
const detailRows = computed(() => {
  const key = props.apiKey
  const rows: Array<{ label: string, value: string }> = []

  if (key.api_formats?.length) {
    rows.push({
      label: legacyT('API 格式'),
      value: key.api_formats.map(formatApiFormatShort).join(' / '),
    })
  }

  rows.push({
    label: legacyT('RPM 限制'),
    value: key.rpm_limit != null ? String(key.rpm_limit) : legacyT('无限制'),
  })

  rows.push({
    label: legacyT('并发限制'),
    value: key.concurrent_limit ? String(key.concurrent_limit) : legacyT('不限制'),
  })

  if (key.default_rate_multiplier != null) {
    rows.push({
      label: legacyT('成本倍率'),
      value: `x${key.default_rate_multiplier}`,
    })
  }

  if (Array.isArray(key.allowed_models) && key.allowed_models.length > 0) {
    rows.push({
      label: legacyT('允许的模型'),
      value: key.allowed_models.join(', '),
    })
  }

  rows.push({
    label: legacyT('创建时间'),
    value: formatDate(key.created_at),
  })

  rows.push({
    label: legacyT('最近使用'),
    value: key.last_used_at ? formatDate(key.last_used_at) : legacyT('暂无记录'),
  })

  return rows
})
</script>
