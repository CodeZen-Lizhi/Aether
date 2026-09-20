<template>
  <Dialog
    :model-value="internalOpen"
    :title="legacyT(isEditMode ? '编辑提供商' : '添加提供商')"
    :description="legacyT(isEditMode ? '更新提供商配置。API 端点和密钥需在详情页面单独管理。' : '创建新的提供商配置。创建后可以为其添加 API 端点和密钥。')"
    :icon="isEditMode ? SquarePen : Server"
    size="xl"
    @update:model-value="handleDialogUpdate"
  >
    <form
      class="space-y-5"
      @submit.prevent="handleSubmit"
    >
      <!-- 基本信息 -->
      <div class="space-y-3">
        <h3 class="text-sm font-medium border-b pb-2">
          {{ legacyT('基本信息') }}
        </h3>

        <div class="space-y-1.5">
          <Label for="name">{{ legacyT('名称 *') }}</Label>
          <Input
            id="name"
            v-model="form.name"
            :placeholder="legacyT('例如: OpenAI 主账号')"
          />
        </div>

        <div class="space-y-1.5">
          <Label for="website">{{ legacyT('主站链接') }}</Label>
          <Input
            id="website"
            v-model="form.website"
            :placeholder="legacyT('https://example.com（可选）')"
          />
        </div>
      </div>

      <!-- 请求配置 -->
      <div class="space-y-3">
        <h3 class="text-sm font-medium border-b pb-2">
          {{ legacyT('请求配置') }}
        </h3>
        <div class="dialog-grid-2 gap-4">
          <div class="space-y-1.5">
            <Label for="chat-max-attempts">{{ legacyT('总尝试次数（含首次）') }}</Label>
            <Input
              id="chat-max-attempts"
              :model-value="form.max_attempts ?? ''"
              type="number"
              min="1"
              max="99"
              step="1"
              placeholder="1"
              @update:model-value="(v) => form.max_attempts = v === '' ? undefined : Number(v)"
            />
            <p
              v-if="provider?.effective_max_attempts !== undefined"
              class="text-xs text-muted-foreground"
            >
              {{ legacyT('当前生效次数') }}: {{ provider.effective_max_attempts }}
              <span>{{ legacyT(attemptSourceLabel) }}</span>
            </p>
          </div>
          <div class="space-y-1.5">
            <Label for="stream-total-timeout">{{ legacyT('流式请求总超时') }} <span class="text-xs text-muted-foreground">{{ legacyT('(秒)') }}</span></Label>
            <Input
              id="stream-total-timeout"
              :model-value="form.stream_total_timeout ?? ''"
              type="number"
              min="1"
              max="1200"
              step="0.001"
              placeholder="900"
              aria-describedby="stream-total-timeout-help stream-total-timeout-effective"
              @update:model-value="(v) => form.stream_total_timeout = v === '' ? undefined : Number(v)"
            />
            <p
              id="stream-total-timeout-help"
              class="text-xs text-muted-foreground"
            >
              {{ legacyT('从请求开始到完整回答结束，包含重试、切换和输出；普通流式与压缩共用。清空恢复默认 900 秒。') }}
            </p>
            <p
              id="stream-total-timeout-effective"
              class="text-xs text-muted-foreground"
            >
              {{ legacyT('当前生效总超时') }}: {{ provider?.effective_stream_total_timeout ?? 900 }} {{ legacyT('秒') }}
              <span>{{ legacyT(provider?.effective_stream_total_timeout_source === 'config.stream_total_timeout_ms' ? '提供商配置' : '默认配置') }}</span>
            </p>
          </div>
        </div>

        <!-- 超时配置 -->
        <div class="dialog-grid-2 gap-4">
          <div class="space-y-1.5">
            <Label for="stream-first-response-timeout">
              {{ legacyT('首次响应超时') }}
              <span class="text-xs text-muted-foreground">{{ legacyT('(秒)') }}</span>
            </Label>
            <Input
              id="stream-first-response-timeout"
              :model-value="form.stream_first_byte_timeout ?? ''"
              type="number"
              min="1"
              max="300"
              step="1"
              placeholder="30"
              aria-describedby="stream-first-response-timeout-help"
              @update:model-value="(v) => form.stream_first_byte_timeout = parseNumberInput(v)"
            />
            <p
              id="stream-first-response-timeout-help"
              class="text-xs text-muted-foreground"
            >
              {{ legacyT('每次尝试等待上游成功响应头的上限；收到响应头后，继续等待结果由流式请求总超时限制。') }}
            </p>
          </div>
          <div class="space-y-1.5">
            <Label>
              {{ legacyT('非流式请求超时') }}
              <span class="text-xs text-muted-foreground">{{ legacyT('(秒)') }}</span>
            </Label>
            <Input
              :model-value="form.request_timeout ?? ''"
              type="number"
              min="1"
              max="1200"
              step="1"
              placeholder="300"
              @update:model-value="(v) => form.request_timeout = parseNumberInput(v)"
            />
          </div>
        </div>

        <!-- 提供商内转移限制 -->
        <div class="dialog-grid-2 gap-2 sm:gap-4">
          <div class="min-w-0 space-y-1.5">
            <Label
              for="max-transfer-count"
              class="text-xs sm:text-sm"
            >
              {{ legacyT('最大转移次数') }}
            </Label>
            <Input
              id="max-transfer-count"
              :model-value="form.max_transfer_count === 0 ? '' : form.max_transfer_count"
              type="number"
              min="0"
              step="1"
              :placeholder="legacyT('0 (不限制)')"
              @update:model-value="(v) => form.max_transfer_count = parseNumberInput(v, { min: 0 }) ?? 0"
            />
          </div>
          <div class="min-w-0 space-y-1.5">
            <Label
              for="max-transfer-timeout-seconds"
              class="text-xs sm:text-sm"
            >
              {{ legacyT('最大转移超时') }}
              <span class="text-xs text-muted-foreground">{{ legacyT('(秒)') }}</span>
            </Label>
            <Input
              id="max-transfer-timeout-seconds"
              :model-value="form.max_transfer_timeout_seconds === 0 ? '' : form.max_transfer_timeout_seconds"
              type="number"
              min="0"
              step="1"
              :placeholder="legacyT('0 (不限制)')"
              @update:model-value="(v) => form.max_transfer_timeout_seconds = parseNumberInput(v, { min: 0 }) ?? 0"
            />
          </div>
        </div>
      </div>

      <!-- 功能开关 -->
      <div class="space-y-3">
        <h3 class="text-sm font-medium border-b pb-2">
          {{ legacyT('功能开关') }}
        </h3>

        <div
          class="flex items-center justify-between gap-3 p-3 border rounded-lg bg-muted/50"
          data-testid="responses-websocket-setting"
        >
          <div class="space-y-0.5">
            <Label
              for="responses-websocket-enabled"
              class="text-sm font-medium"
            >
              {{ legacyT('Responses WebSocket 模式') }}
            </Label>
            <p class="text-xs text-muted-foreground leading-relaxed">
              {{ legacyT('允许此提供商处理标准 Responses API WebSocket 请求。仅在已验证兼容性后启用。') }}
            </p>
          </div>
          <Switch
            id="responses-websocket-enabled"
            :model-value="form.responses_websocket_enabled"
            :aria-label="legacyT('Responses WebSocket 模式')"
            @update:model-value="(v: boolean) => form.responses_websocket_enabled = v"
          />
        </div>
      </div>
    </form>

    <template #footer>
      <Button
        type="button"
        variant="outline"
        :disabled="loading"
        @click="handleCancel"
      >
        {{ legacyT('取消') }}
      </Button>
      <Button
        :disabled="loading || !form.name"
        @click="handleSubmit"
      >
        {{ submitLabel }}
      </Button>
    </template>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import {
  Dialog,
  Button,
  Input,
  Label,
  Switch,
} from '@/components/ui'
import { Server, SquarePen } from 'lucide-vue-next'
import { useToast } from '@/composables/useToast'
import { useFormDialog } from '@/composables/useFormDialog'
import { useI18n } from '@/i18n'
import {
  createProvider,
  updateProvider,
  type ProviderWithEndpointsSummary,
} from '@/api/endpoints'
import { parseApiError } from '@/utils/errorParser'
import { parseNumberInput } from '@/utils/form'

const props = defineProps<{
  modelValue: boolean
  provider?: ProviderWithEndpointsSummary | null  // 编辑模式时传入
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  'providerCreated': []
  'providerUpdated': [provider: ProviderWithEndpointsSummary]
}>()

const { success, error: showError } = useToast()
const { legacyT } = useI18n()
const loading = ref(false)

// 内部状态
const internalOpen = computed(() => props.modelValue)

const submitLabel = computed(() => {
  if (loading.value) {
    return legacyT(isEditMode.value ? '保存中...' : '创建中...')
  }
  return legacyT(isEditMode.value ? '保存' : '创建')
})

// 表单数据
const form = ref({
  name: '',
  description: '',
  website: '',
  // 状态配置
  is_active: true,
  rate_limit: undefined as number | undefined,
  concurrent_limit: undefined as number | undefined,
  // 请求配置
  max_attempts: undefined as number | undefined,
  stream_total_timeout: undefined as number | undefined,
  max_transfer_count: 0,
  max_transfer_timeout_seconds: 0,
  // 超时配置（秒）
  stream_first_byte_timeout: undefined as number | undefined,
  request_timeout: undefined as number | undefined,
  // Responses WebSocket 配置
  responses_websocket_enabled: false,
})

// 重置表单
function resetForm() {
  form.value = {
    name: '',
    description: '',
    website: '',
    is_active: true,
    rate_limit: undefined,
    concurrent_limit: undefined,
    // 请求配置
    max_attempts: undefined,
    stream_total_timeout: undefined,
    max_transfer_count: 0,
    max_transfer_timeout_seconds: 0,
    // 超时配置
    stream_first_byte_timeout: undefined,
    request_timeout: undefined,
    // Responses WebSocket 配置
    responses_websocket_enabled: false,
  }
}

// 加载提供商数据（编辑模式）
function loadProviderData() {
  if (!props.provider) return
  form.value = {
    name: props.provider.name,
    description: props.provider.description || '',
    website: props.provider.website || '',
    is_active: props.provider.is_active,
    rate_limit: undefined,
    concurrent_limit: undefined,
    // 请求配置
    max_attempts: props.provider.effective_max_attempts,
    stream_total_timeout: props.provider.stream_total_timeout ?? undefined,
    max_transfer_count: props.provider.max_transfer_count ?? 0,
    max_transfer_timeout_seconds: props.provider.max_transfer_timeout_seconds ?? 0,
    // 超时配置
    stream_first_byte_timeout: props.provider.stream_first_byte_timeout ?? undefined,
    request_timeout: props.provider.request_timeout ?? undefined,
    // Responses WebSocket 配置
    responses_websocket_enabled: props.provider.responses_websocket_enabled ?? false,
  }
}

// 使用 useFormDialog 统一处理对话框逻辑
const { isEditMode, handleDialogUpdate, handleCancel } = useFormDialog({
  isOpen: () => props.modelValue,
  entity: () => props.provider,
  isLoading: loading,
  onClose: () => emit('update:modelValue', false),
  loadData: loadProviderData,
  resetForm,
})

const attemptSourceLabel = computed(() => {
  const source = props.provider?.effective_max_attempts_source ?? 'default'
  if (source.startsWith('failover_rules.')) return '故障转移规则'
  if (source.startsWith('endpoint.')) return '端点配置'
  if (source.startsWith('provider.')) return '提供商配置'
  return '默认配置'
})

// 提交表单
const handleSubmit = async () => {
  loading.value = true
  try {
    for (const [value, max, message] of [
      [form.value.max_attempts, 99, '总尝试次数必须是 1 到 99 之间的整数'],
    ] as const) {
      if (value !== undefined && (!Number.isInteger(value) || value < 1 || value > max)) {
        throw new Error(legacyT(message))
      }
    }
    const totalSeconds = form.value.stream_total_timeout
    if (totalSeconds !== undefined && (
      !Number.isFinite(totalSeconds)
      || totalSeconds < 1
      || totalSeconds > 1200
      || Number(totalSeconds.toFixed(3)) !== totalSeconds
    )) {
      throw new Error(legacyT('流式请求总超时必须在 1 到 1200 秒之间，最多保留三位小数'))
    }
    const retryPatch: Record<string, number | null> = {}
    if (form.value.max_attempts !== props.provider?.effective_max_attempts) {
      retryPatch.max_attempts = form.value.max_attempts ?? null
    }
    const basePayload = {
      name: form.value.name,
      description: form.value.description || undefined,
      website: form.value.website || undefined,
      responses_websocket_enabled: form.value.responses_websocket_enabled,
      is_active: form.value.is_active,
      // 请求配置
      ...(Object.keys(retryPatch).length ? { failover_rules: retryPatch } : {}),
      max_transfer_count: form.value.max_transfer_count,
      max_transfer_timeout_seconds: form.value.max_transfer_timeout_seconds,
      // 超时配置（null 表示清除，使用全局配置）
      stream_first_byte_timeout: form.value.stream_first_byte_timeout ?? null,
      ...(totalSeconds !== (props.provider?.stream_total_timeout ?? undefined)
        ? { stream_total_timeout: totalSeconds ?? null }
        : {}),
      request_timeout: form.value.request_timeout ?? null,
    }

    if (isEditMode.value && props.provider) {
      // 更新提供商
      const updated = await updateProvider(props.provider.id, basePayload)
      success(legacyT('提供商更新成功'))
      emit('providerUpdated', updated)
    } else {
      // 创建提供商
      await createProvider(basePayload)
      success(legacyT('提供商已创建，请继续添加端点和密钥'), legacyT('创建成功'))
      emit('providerCreated')
    }

    emit('update:modelValue', false)
  } catch (error: unknown) {
    const action = isEditMode.value ? '更新' : '创建'
    showError(parseApiError(error, legacyT(`${action}提供商失败`)), legacyT(`${action}失败`))
  } finally {
    loading.value = false
  }
}
</script>
