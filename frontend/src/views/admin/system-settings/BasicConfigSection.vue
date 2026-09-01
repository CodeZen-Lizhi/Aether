<template>
  <CardSection
    title="基础配置"
    description="配置系统默认参数"
  >
    <template #actions>
      <Button
        size="sm"
        :disabled="loading || !hasChanges"
        @click="$emit('save')"
      >
        {{ loading ? '保存中...' : '保存' }}
      </Button>
    </template>
    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
      <div>
        <Label
          for="rate-limit"
          class="block text-sm font-medium"
        >
          默认速率限制 (请求/分钟)
        </Label>
        <Input
          id="rate-limit"
          :model-value="rateLimitPerMinute"
          type="number"
          placeholder="0"
          class="mt-1"
          @update:model-value="$emit('update:rateLimitPerMinute', Number($event))"
        />
        <p class="mt-1 text-xs text-muted-foreground">
          0 表示默认不限制；未单独配置的 Key 会跟随这里
        </p>
      </div>

      <div class="flex items-center h-full">
        <div class="flex items-center space-x-2">
          <Checkbox
            id="auto-delete-expired-keys"
            :checked="autoDeleteExpiredKeys"
            @update:checked="$emit('update:autoDeleteExpiredKeys', $event)"
          />
          <div>
            <Label
              for="auto-delete-expired-keys"
              class="cursor-pointer"
            >
              自动删除过期 Key
            </Label>
            <p class="text-xs text-muted-foreground">
              关闭时仅禁用过期的独立余额 Key
            </p>
          </div>
        </div>
      </div>

      <div class="flex items-center h-full">
        <div class="flex items-center space-x-2">
          <Checkbox
            id="enable-format-conversion"
            :checked="enableFormatConversion"
            @update:checked="$emit('update:enableFormatConversion', $event)"
          />
          <div>
            <Label
              for="enable-format-conversion"
              class="cursor-pointer"
            >
              全局格式转换
            </Label>
            <p class="text-xs text-muted-foreground">
              开启后强制允许所有提供商接受跨格式请求
            </p>
          </div>
        </div>
      </div>

      <div class="flex items-center h-full">
        <div class="flex items-center space-x-2">
          <Checkbox
            id="enable-openai-image-sync-heartbeat"
            :checked="enableOpenaiImageSyncHeartbeat"
            @update:checked="$emit('update:enableOpenaiImageSyncHeartbeat', $event)"
          />
          <div>
            <Label
              for="enable-openai-image-sync-heartbeat"
              class="cursor-pointer"
            >
              同步生图心跳
            </Label>
            <p class="text-xs text-muted-foreground">
              开启后同步生图外层 HTTP 状态固定为 200，上游失败需读取响应体 error.upstream_status
            </p>
          </div>
        </div>
      </div>

      <div class="flex items-center h-full">
        <div class="flex items-center space-x-2">
          <Checkbox
            id="enable-standard-text-sync-heartbeat"
            :checked="enableStandardTextSyncHeartbeat"
            @update:checked="$emit('update:enableStandardTextSyncHeartbeat', $event)"
          />
          <div>
            <Label
              for="enable-standard-text-sync-heartbeat"
              class="cursor-pointer"
            >
              标准文本非流式心跳
            </Label>
            <p class="text-xs text-muted-foreground">
              开启后标准文本非流式接口外层 HTTP 状态固定为 200，上游失败需读取响应体 error.upstream_status
            </p>
          </div>
        </div>
      </div>

      <div class="flex items-center h-full">
        <div class="flex items-center space-x-2">
          <Checkbox
            id="cyber-continue-failover"
            :checked="cyberContinueFailover"
            @update:checked="$emit('update:cyberContinueFailover', $event)"
          />
          <div>
            <Label
              for="cyber-continue-failover"
              class="cursor-pointer"
            >
              Cyber继续转移
            </Label>
            <p class="text-xs text-muted-foreground">
              关闭时Cyber Policy错误直接返回客户端；开启后在响应内容开始前按普通错误继续故障转移，可能增加首字等待时间
            </p>
          </div>
        </div>
      </div>
    </div>
  </CardSection>
</template>

<script setup lang="ts">
import Button from '@/components/ui/button.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import Checkbox from '@/components/ui/checkbox.vue'
import { CardSection } from '@/components/layout'

defineProps<{
  rateLimitPerMinute: number
  autoDeleteExpiredKeys: boolean
  enableFormatConversion: boolean
  enableOpenaiImageSyncHeartbeat: boolean
  enableStandardTextSyncHeartbeat: boolean
  cyberContinueFailover: boolean
  loading: boolean
  hasChanges: boolean
}>()

defineEmits<{
  save: []
  'update:rateLimitPerMinute': [value: number]
  'update:autoDeleteExpiredKeys': [value: boolean]
  'update:enableFormatConversion': [value: boolean]
  'update:enableOpenaiImageSyncHeartbeat': [value: boolean]
  'update:enableStandardTextSyncHeartbeat': [value: boolean]
  'update:cyberContinueFailover': [value: boolean]
}>()
</script>
