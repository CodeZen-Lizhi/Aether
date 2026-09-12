<template>
  <section class="settings-group">
    <h3 class="settings-heading">
      自动清理与保留
    </h3>
    <div class="settings-row settings-row--toggle">
      <div>
        <Label for="enable-auto-cleanup">自动清理</Label>
        <p class="settings-description">
          每天凌晨执行
        </p>
      </div>
      <Switch
        id="enable-auto-cleanup"
        :model-value="enableAutoCleanup"
        :disabled="autoCleanupLoading || loading"
        @update:model-value="$emit('toggleAutoCleanup', $event)"
      />
    </div>
    <div class="settings-row">
      <div>
        <Label for="compressed-log-retention-days">请求与响应内容保留</Label>
        <p class="settings-description">
          超过期限后删除请求和响应内容。
        </p>
      </div>
      <div class="settings-number">
        <Input
          id="compressed-log-retention-days"
          :model-value="compressedLogRetentionDays"
          type="number"
          min="0"
          step="1"
          @update:model-value="$emit('update:compressedLogRetentionDays', Number($event))"
        />
        <span>天</span>
      </div>
    </div>
    <div class="settings-row">
      <div>
        <Label for="log-retention-days">请求记录保留</Label>
        <p class="settings-description">
          超过期限后删除整条请求记录。
        </p>
      </div>
      <div class="settings-number">
        <Input
          id="log-retention-days"
          :model-value="logRetentionDays"
          type="number"
          min="0"
          step="1"
          @update:model-value="$emit('update:logRetentionDays', Number($event))"
        />
        <span>天</span>
      </div>
    </div>
    <details
      ref="detailsElement"
      class="settings-disclosure"
    >
      <summary>详细保留策略<ChevronDown class="settings-chevron" /></summary>
      <div class="settings-disclosure-content">
        <div class="settings-row">
          <div>
            <Label for="detail-log-retention-days">开始压缩内容</Label>
            <p class="settings-description">
              超过此天数后压缩内容，仍可查看，不代表删除。
            </p>
          </div>
          <div class="settings-number">
            <Input
              id="detail-log-retention-days"
              :model-value="detailLogRetentionDays"
              type="number"
              min="0"
              step="1"
              @update:model-value="$emit('update:detailLogRetentionDays', Number($event))"
            />
            <span>天</span>
          </div>
        </div>
        <div class="settings-row">
          <div>
            <Label for="header-retention-days">请求头保留</Label>
            <p class="settings-description">
              超过期限后清空请求头。
            </p>
          </div>
          <div class="settings-number">
            <Input
              id="header-retention-days"
              :model-value="headerRetentionDays"
              type="number"
              min="0"
              step="1"
              @update:model-value="$emit('update:headerRetentionDays', Number($event))"
            />
            <span>天</span>
          </div>
        </div>
        <div class="settings-row">
          <div>
            <Label for="audit-log-retention-days">审计日志保留</Label>
            <p class="settings-description">
              登录和操作等安全事件的保留期限。
            </p>
          </div>
          <div class="settings-number">
            <Input
              id="audit-log-retention-days"
              :model-value="auditLogRetentionDays"
              type="number"
              min="0"
              step="1"
              @update:model-value="$emit('update:auditLogRetentionDays', Number($event))"
            />
            <span>天</span>
          </div>
        </div>
        <div class="settings-row">
          <div>
            <Label for="request-candidates-retention-days">候选记录保留</Label>
            <p class="settings-description">
              请求调度候选与尝试记录的保留期限。
            </p>
          </div>
          <div class="settings-number">
            <Input
              id="request-candidates-retention-days"
              :model-value="requestCandidatesRetentionDays"
              type="number"
              min="0"
              step="1"
              @update:model-value="$emit('update:requestCandidatesRetentionDays', Number($event))"
            />
            <span>天</span>
          </div>
        </div>
        <div class="settings-row">
          <div>
            <Label for="proxy-node-metrics-1m-retention-days">代理分钟指标保留</Label>
            <p class="settings-description">
              分钟级稳定性指标的保留期限。
            </p>
          </div>
          <div class="settings-number">
            <Input
              id="proxy-node-metrics-1m-retention-days"
              :model-value="proxyNodeMetrics1mRetentionDays"
              type="number"
              min="1"
              max="365"
              step="1"
              @update:model-value="$emit('update:proxyNodeMetrics1mRetentionDays', Number($event))"
            />
            <span>天</span>
          </div>
        </div>
        <div class="settings-row">
          <div>
            <Label for="proxy-node-metrics-1h-retention-days">代理小时指标保留</Label>
            <p class="settings-description">
              长期趋势指标，不能短于分钟指标的保留期限。
            </p>
          </div>
          <div class="settings-number">
            <Input
              id="proxy-node-metrics-1h-retention-days"
              :model-value="proxyNodeMetrics1hRetentionDays"
              type="number"
              min="1"
              max="1095"
              step="1"
              @update:model-value="$emit('update:proxyNodeMetrics1hRetentionDays', Number($event))"
            />
            <span>天</span>
          </div>
        </div>
      </div>
    </details>
    <SettingsSaveActions
      :loading="loading"
      :has-changes="hasChanges"
      :error="validationError || error"
      @save="save"
      @cancel="cancel"
    />
  </section>
</template>

<script setup lang="ts">
import { nextTick, ref } from 'vue'
import { ChevronDown } from 'lucide-vue-next'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import Switch from '@/components/ui/switch.vue'
import SettingsSaveActions from './SettingsSaveActions.vue'

const props = defineProps<{
  enableAutoCleanup: boolean
  compressedLogRetentionDays: number
  logRetentionDays: number
  detailLogRetentionDays: number
  headerRetentionDays: number
  auditLogRetentionDays: number
  requestCandidatesRetentionDays: number
  proxyNodeMetrics1mRetentionDays: number
  proxyNodeMetrics1hRetentionDays: number
  autoCleanupLoading: boolean
  loading: boolean
  hasChanges: boolean
  error?: string
}>()

const emit = defineEmits<{
  save: []
  cancel: []
  toggleAutoCleanup: [enabled: boolean]
  'update:compressedLogRetentionDays': [value: number]
  'update:logRetentionDays': [value: number]
  'update:detailLogRetentionDays': [value: number]
  'update:headerRetentionDays': [value: number]
  'update:auditLogRetentionDays': [value: number]
  'update:requestCandidatesRetentionDays': [value: number]
  'update:proxyNodeMetrics1mRetentionDays': [value: number]
  'update:proxyNodeMetrics1hRetentionDays': [value: number]
}>()

const detailsElement = ref<(HTMLElement & { open: boolean }) | null>(null)
const validationError = ref('')

async function save() {
  validationError.value = ''
  let invalidId = ''
  if (props.detailLogRetentionDays > props.compressedLogRetentionDays) {
    validationError.value = '开始压缩内容的天数不能超过请求与响应内容保留天数。'
    invalidId = 'detail-log-retention-days'
  } else if (props.proxyNodeMetrics1hRetentionDays < props.proxyNodeMetrics1mRetentionDays) {
    validationError.value = '代理小时指标的保留天数不能短于分钟指标。'
    invalidId = 'proxy-node-metrics-1h-retention-days'
  } else {
    const inputs = detailsElement.value?.closest('section')?.querySelectorAll<HTMLInputElement>('input[type="number"]')
    for (const input of inputs ?? []) {
      if (!input.value || !input.checkValidity()) {
        validationError.value = '请填写有效的保留天数。'
        invalidId = input.id
        break
      }
    }
  }
  if (invalidId) {
    if (detailsElement.value?.querySelector(`#${  invalidId}`)) detailsElement.value.open = true
    await nextTick()
    document.getElementById(invalidId)?.focus()
    return
  }
  emit('save')
}

function cancel() {
  validationError.value = ''
  emit('cancel')
}
</script>
