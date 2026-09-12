<template>
  <section class="settings-group">
    <h3 class="settings-heading">
      请求记录
    </h3>
    <div class="settings-row">
      <div>
        <Label for="request-log-level">记录详细程度</Label>
        <p class="settings-description">
          敏感信息会自动脱敏
        </p>
      </div>
      <div class="settings-row-control">
        <Select
          :model-value="requestRecordLevel"
          @update:model-value="$emit('update:requestRecordLevel', $event)"
        >
          <SelectTrigger id="request-log-level">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="basic">
              基本信息
            </SelectItem>
            <SelectItem value="headers">
              基本信息与请求头
            </SelectItem>
            <SelectItem value="full">
              完整请求与响应
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>
    <details class="settings-disclosure">
      <summary>敏感请求头<ChevronDown class="settings-chevron" /></summary>
      <div class="settings-disclosure-content">
        <Label for="sensitive-headers">脱敏请求头</Label>
        <Input
          id="sensitive-headers"
          :model-value="sensitiveHeadersStr"
          placeholder="authorization, x-api-key, cookie"
          class="mt-2 rounded-md"
          @update:model-value="$emit('update:sensitiveHeadersStr', $event)"
        />
        <p class="settings-description">
          逗号分隔，这些请求头会被脱敏处理
        </p>
      </div>
    </details>
    <SettingsSaveActions
      :loading="loading"
      :has-changes="hasChanges"
      :error="error"
      @save="$emit('save')"
      @cancel="$emit('cancel')"
    />
  </section>
</template>

<script setup lang="ts">
import { ChevronDown } from 'lucide-vue-next'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import Select from '@/components/ui/select.vue'
import SelectTrigger from '@/components/ui/select-trigger.vue'
import SelectValue from '@/components/ui/select-value.vue'
import SelectContent from '@/components/ui/select-content.vue'
import SelectItem from '@/components/ui/select-item.vue'
import SettingsSaveActions from './SettingsSaveActions.vue'

defineProps<{
  requestRecordLevel: string
  sensitiveHeadersStr: string
  loading: boolean
  hasChanges: boolean
  error?: string
}>()

defineEmits<{
  save: []
  cancel: []
  'update:requestRecordLevel': [value: string]
  'update:sensitiveHeadersStr': [value: string]
}>()
</script>
