<template>
  <section class="settings-group">
    <h3 class="settings-heading">
      请求兼容
    </h3>
    <div class="settings-row settings-row--toggle">
      <div>
        <Label for="enable-format-conversion">全局格式转换</Label>
        <p class="settings-description">
          开启后强制允许所有提供商接受跨格式请求
        </p>
      </div>
      <Switch
        id="enable-format-conversion"
        :model-value="enableFormatConversion"
        @update:model-value="$emit('update:enableFormatConversion', $event)"
      />
    </div>
    <div class="settings-row settings-row--toggle">
      <div>
        <Label for="enable-openai-image-sync-heartbeat">同步生图心跳</Label>
        <p class="settings-description">
          开启后同步生图外层 HTTP 状态固定为 200，上游失败需读取响应体 error.upstream_status
        </p>
      </div>
      <Switch
        id="enable-openai-image-sync-heartbeat"
        :model-value="enableOpenaiImageSyncHeartbeat"
        @update:model-value="$emit('update:enableOpenaiImageSyncHeartbeat', $event)"
      />
    </div>
    <div class="settings-row settings-row--toggle">
      <div>
        <Label for="enable-standard-text-sync-heartbeat">标准文本非流式心跳</Label>
        <p class="settings-description">
          开启后标准文本非流式接口外层 HTTP 状态固定为 200，上游失败需读取响应体 error.upstream_status
        </p>
      </div>
      <Switch
        id="enable-standard-text-sync-heartbeat"
        :model-value="enableStandardTextSyncHeartbeat"
        @update:model-value="$emit('update:enableStandardTextSyncHeartbeat', $event)"
      />
    </div>
    <div class="settings-row settings-row--toggle">
      <div>
        <Label for="cyber-continue-failover">Cyber 错误继续转移</Label>
        <p class="settings-description">
          关闭时直接返回 Cyber Policy 错误；开启后在响应内容开始前继续尝试其他渠道，可能增加首字等待时间
        </p>
      </div>
      <Switch
        id="cyber-continue-failover"
        :model-value="cyberContinueFailover"
        @update:model-value="$emit('update:cyberContinueFailover', $event)"
      />
    </div>
    <h3 class="settings-heading mt-7">
      密钥默认规则
    </h3>
    <div class="settings-row">
      <div>
        <Label for="rate-limit">默认请求限速</Label>
        <p class="settings-description">
          0 表示默认不限制；未单独配置的 Key 会跟随这里
        </p>
      </div>
      <div class="settings-number">
        <Input
          id="rate-limit"
          :model-value="rateLimitPerMinute"
          type="number"
          min="0"
          step="1"
          @update:model-value="$emit('update:rateLimitPerMinute', Number($event))"
        />
        <span>请求/分钟</span>
      </div>
    </div>
    <div class="settings-row settings-row--toggle">
      <div>
        <Label for="auto-delete-expired-keys">自动删除过期 Key</Label>
        <p class="settings-description">
          关闭时仅禁用过期的独立余额 Key
        </p>
      </div>
      <Switch
        id="auto-delete-expired-keys"
        :model-value="autoDeleteExpiredKeys"
        @update:model-value="$emit('update:autoDeleteExpiredKeys', $event)"
      />
    </div>
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
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import Switch from '@/components/ui/switch.vue'
import SettingsSaveActions from './SettingsSaveActions.vue'

defineProps<{
  rateLimitPerMinute: number
  autoDeleteExpiredKeys: boolean
  enableFormatConversion: boolean
  enableOpenaiImageSyncHeartbeat: boolean
  enableStandardTextSyncHeartbeat: boolean
  cyberContinueFailover: boolean
  loading: boolean
  hasChanges: boolean
  error?: string
}>()

defineEmits<{
  save: []
  cancel: []
  'update:rateLimitPerMinute': [value: number]
  'update:autoDeleteExpiredKeys': [value: boolean]
  'update:enableFormatConversion': [value: boolean]
  'update:enableOpenaiImageSyncHeartbeat': [value: boolean]
  'update:enableStandardTextSyncHeartbeat': [value: boolean]
  'update:cyberContinueFailover': [value: boolean]
}>()
</script>
