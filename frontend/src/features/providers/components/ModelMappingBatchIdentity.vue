<template>
  <div class="min-w-0">
    <div
      class="truncate text-sm font-semibold text-foreground"
      :title="displayName"
    >
      {{ displayName }}
    </div>
    <div class="mt-1 flex min-w-0 items-center gap-1.5">
      <span
        class="min-w-0 truncate font-mono text-[11px] text-muted-foreground"
        :title="model.provider_model_name"
      >
        {{ model.provider_model_name }}
      </span>
      <Button
        variant="ghost"
        size="icon"
        class="h-5 w-5 shrink-0 text-muted-foreground"
        :title="`复制 ${model.provider_model_name}`"
        :aria-label="`复制 ${model.provider_model_name}`"
        @click.stop="$emit('copy', model.provider_model_name)"
      >
        <Copy class="h-3 w-3" />
      </Button>
      <Badge
        v-if="hidden"
        variant="secondary"
        class="shrink-0 text-[10px]"
      >
        已隐藏
      </Badge>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { Copy } from 'lucide-vue-next'
import { Badge, Button } from '@/components/ui'
import type { Model } from '@/api/endpoints'

/** 批量映射表格中的客户端模型身份单元，展示名称、供应商名和隐藏状态。 */
const props = defineProps<{
  /** 客户端模型及其真实供应商模型名。 */
  model: Model
  /** 是否仅在批量工作台中隐藏，不表示模型被停用。 */
  hidden: boolean
}>()

defineEmits<{
  /** 请求复制真实客户端模型 ID。 */
  copy: [modelId: string]
}>()

/** 优先展示全局模型名，缺失时回退到供应商模型名。 */
const displayName = computed(() =>
  props.model.global_model_display_name
  || props.model.global_model_name
  || props.model.provider_model_name)
</script>
