<template>
  <div class="space-y-4">
    <div class="space-y-2">
      <Label class="text-xs font-medium">支持的 API 格式</Label>
      <div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
        <label
          v-for="format in availableApiFormats"
          :key="format"
          class="flex min-h-10 cursor-pointer items-center gap-2 rounded-lg bg-background px-3 text-xs transition-[box-shadow,background-color]"
          :class="apiFormats.includes(format)
            ? 'bg-primary/5 shadow-[0_0_0_1px_rgb(0_0_0/0.10),0_1px_2px_rgb(0_0_0/0.04)] dark:shadow-[0_0_0_1px_rgb(255_255_255/0.12)]'
            : 'shadow-[0_0_0_1px_rgb(0_0_0/0.06)] hover:bg-muted/30 hover:shadow-[0_0_0_1px_rgb(0_0_0/0.10)] dark:shadow-[0_0_0_1px_rgb(255_255_255/0.08)]'"
        >
          <Checkbox
            :checked="apiFormats.includes(format)"
            @update:checked="(checked) => toggleApiFormat(format, checked === true)"
          />
          <span class="truncate">{{ formatApiFormat(format) }}</span>
        </label>
      </div>
    </div>

    <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      <div class="space-y-1.5">
        <Label class="text-xs">认证类型</Label>
        <Select v-model="authTypeModel">
          <SelectTrigger class="h-10">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="api_key">API Key</SelectItem>
            <SelectItem value="bearer">Bearer Token</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div class="space-y-1.5">
        <Label class="text-xs">RPM 限制</Label>
        <Input
          :model-value="settings.rpm_limit ?? ''"
          type="number"
          min="1"
          max="10000"
          class="h-10"
          placeholder="自适应"
          @update:model-value="updateSetting('rpm_limit', parseNullableNumberInput($event, { min: 1, max: 10000 }))"
        />
      </div>
      <div class="space-y-1.5">
        <Label class="text-xs">并发请求上限</Label>
        <Input
          :model-value="settings.concurrent_limit ?? ''"
          type="number"
          min="0"
          class="h-10"
          placeholder="不限制"
          @update:model-value="updateSetting('concurrent_limit', parseNullableNumberInput($event, { min: 0 }))"
        />
      </div>
      <div class="space-y-1.5">
        <Label class="text-xs">缓存 TTL（分钟）</Label>
        <Input
          :model-value="settings.cache_ttl_minutes"
          type="number"
          min="0"
          max="60"
          class="h-10"
          @update:model-value="updateSetting('cache_ttl_minutes', parseNumberInput($event, { min: 0, max: 60 }) ?? 5)"
        />
      </div>
      <div class="space-y-1.5">
        <Label class="text-xs">熔断探测（分钟）</Label>
        <Input
          :model-value="settings.max_probe_interval_minutes"
          type="number"
          min="0"
          max="32"
          class="h-10"
          @update:model-value="updateSetting('max_probe_interval_minutes', parseNumberInput($event, { min: 0, max: 32 }) ?? 32)"
        />
      </div>
    </div>

    <div class="grid gap-3 sm:grid-cols-2">
      <div class="space-y-1.5">
        <Label class="text-xs">代理节点</Label>
        <ProxyNodeSelect
          :model-value="settings.proxy_node_id"
          trigger-class="h-10"
          @update:model-value="updateSetting('proxy_node_id', $event)"
        />
      </div>
      <div class="space-y-1.5">
        <Label class="text-xs">备注</Label>
        <Input
          :model-value="settings.note"
          class="h-10"
          placeholder="可选"
          @update:model-value="updateSetting('note', String($event))"
        />
      </div>
    </div>

    <div class="flex min-h-12 items-center justify-between gap-3 rounded-lg bg-background px-3 shadow-[0_0_0_1px_rgb(0_0_0/0.06),0_1px_2px_rgb(0_0_0/0.04)] dark:shadow-[0_0_0_1px_rgb(255_255_255/0.08)]">
      <div>
        <div class="text-xs font-medium">导入后立即启用</div>
        <div class="text-[11px] text-muted-foreground">关闭后仍会创建，但不会进入调度</div>
      </div>
      <Switch
        :model-value="settings.is_active"
        @update:model-value="updateSetting('is_active', $event)"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  Checkbox,
  Input,
  Label,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Switch,
} from '@/components/ui'
import { formatApiFormat } from '@/api/endpoints/types/api-format'
import { parseNullableNumberInput, parseNumberInput } from '@/utils/form'
import ProxyNodeSelect from './ProxyNodeSelect.vue'

type AuthType = 'api_key' | 'bearer'

export interface KeyImportSettings {
  rpm_limit: number | null
  concurrent_limit: number | null
  cache_ttl_minutes: number
  max_probe_interval_minutes: number
  is_active: boolean
  note: string
  proxy_node_id: string
}

const props = defineProps<{
  authType: AuthType
  apiFormats: string[]
  settings: KeyImportSettings
  availableApiFormats: string[]
}>()

const emit = defineEmits<{
  'update:authType': [value: AuthType]
  'update:apiFormats': [value: string[]]
  'update:settings': [value: KeyImportSettings]
}>()

const authTypeModel = computed<AuthType>({
  get: () => props.authType,
  set: value => emit('update:authType', value),
})

function toggleApiFormat(format: string, checked: boolean): void {
  const selected = new Set(props.apiFormats)
  if (checked) selected.add(format)
  else selected.delete(format)
  emit('update:apiFormats', props.availableApiFormats.filter(item => selected.has(item)))
}

function updateSetting<Key extends keyof KeyImportSettings>(
  key: Key,
  value: KeyImportSettings[Key],
): void {
  emit('update:settings', { ...props.settings, [key]: value })
}
</script>