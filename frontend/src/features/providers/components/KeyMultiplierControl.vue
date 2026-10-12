<template>
  <button
    v-if="actionOnly && source === 'upstream'"
    type="button"
    :disabled="busy"
    class="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-primary transition-colors hover:bg-primary/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
    :title="legacyT(busy ? '同步中…' : '立即同步倍率')"
    :aria-label="legacyT(busy ? '同步中…' : '立即同步倍率')"
    @click="runSync()"
  >
    <Loader2
      v-if="busy"
      class="h-4 w-4 animate-spin"
    />
    <BadgePercent
      v-else
      class="h-4 w-4"
    />
  </button>

  <div
    v-else-if="!actionOnly"
    class="inline-flex min-w-0 max-w-full flex-wrap items-center gap-x-1.5 gap-y-1"
  >
    <Input
      v-if="editing"
      v-model="draftValue"
      type="number"
      min="0"
      step="any"
      :disabled="busy"
      :aria-label="legacyT('倍率')"
      :aria-describedby="errorMessage ? `multiplier-error-${apiKey.id}` : undefined"
      :autofocus="true"
      class="h-7 w-[4.5rem] px-1.5 text-xs font-semibold tabular-nums"
      @blur="saveManual"
      @keydown.enter.prevent="saveManual"
      @keydown.esc.prevent="cancelEditing"
    />

    <button
      v-else-if="source === 'manual'"
      type="button"
      :aria-label="`${legacyT('倍率设置')} · ${apiKey.name}`"
      :title="legacyT('点击修改倍率')"
      class="min-h-7 shrink-0 rounded-md bg-transparent px-2 text-xs font-semibold tabular-nums text-foreground transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
      :disabled="busy"
      @click="startEditing"
    >
      {{ displayValue }}
    </button>

    <span
      v-else
      class="inline-flex min-h-7 shrink-0 items-center rounded-md px-2 text-xs font-semibold tabular-nums text-muted-foreground"
      :title="legacyT('跟随上游时倍率由上游决定')"
    >
      {{ displayValue }}
    </span>

    <Select
      :model-value="source"
      @update:model-value="handleSourceChange"
    >
      <SelectTrigger
        class="h-7 min-h-7 w-auto shrink-0 justify-start gap-1.5 rounded-md border-0 bg-[color-mix(in_srgb,var(--muted)_70%,transparent)] px-2 text-xs font-medium text-foreground shadow-none backdrop-blur-none transition-colors hover:bg-muted focus:border-0 focus:ring-2 focus:ring-ring [&>span]:truncate-none [&>svg]:h-3 [&>svg]:w-3 [&>svg]:shrink-0 [&>svg]:text-muted-foreground"
        :data-multiplier-source="source"
        :aria-label="legacyT('倍率来源')"
        :aria-describedby="errorMessage ? `multiplier-error-${apiKey.id}` : undefined"
        :disabled="busy"
        :aria-busy="busy"
      >
        <SelectValue />
      </SelectTrigger>
      <SelectContent
        :searchable="false"
        class="min-w-[6.5rem] rounded-xl p-0.5"
      >
        <SelectItem
          v-for="mode in modes"
          :key="mode.value"
          :value="mode.value"
          class="rounded-lg py-1 pl-7 pr-2 text-xs font-medium"
        >
          {{ legacyT(mode.label) }}
        </SelectItem>
      </SelectContent>
    </Select>

    <p
      v-if="errorMessage"
      :id="`multiplier-error-${apiKey.id}`"
      class="basis-full px-1 pt-0.5 text-xs leading-4 text-destructive"
      role="alert"
    >
      {{ errorMessage }}
    </p>
    <!-- 等待文案与错误提示同位置：等待中说明在查接口，失败后立即被红色原因替换。 -->
    <p
      v-else-if="busy"
      :data-multiplier-busy="pendingMode ?? 'sync'"
      class="inline-flex basis-full items-center gap-1 px-1 pt-0.5 text-xs leading-4 text-muted-foreground"
      role="status"
    >
      <Loader2 class="h-3 w-3 shrink-0 animate-spin" />
      {{ statusText }}
    </p>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { BadgePercent, Loader2 } from 'lucide-vue-next'
import {
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui'
import { useI18n } from '@/i18n'
import { useToast } from '@/composables/useToast'
import { syncProviderMultiplier } from '@/api/providerOps'
import type { EndpointAPIKey } from '@/api/endpoints'

type MultiplierSource = 'manual' | 'upstream'

/** 列表内倍率编辑器；普通模式直接编辑，actionOnly 仅保留上游同步快捷操作。 */
const props = defineProps<{
  apiKey: EndpointAPIKey
  providerId: string
  actionOnly?: boolean
}>()

const emit = defineEmits<{
  refresh: []
}>()

const { legacyT } = useI18n()
const { error: showError, success } = useToast()
const busy = ref(false)
/** 本次请求要切换到的来源；仅在切来源时非空，用于区分查询上游与保存手动。 */
const pendingMode = ref<MultiplierSource | null>(null)
const editing = ref(false)
const draftValue = ref<string | number>(1)
const errorMessage = ref('')
const source = ref<MultiplierSource>(props.apiKey.multiplier_sync?.source === 'upstream' ? 'upstream' : 'manual')
const displayValue = ref(props.apiKey.default_rate_multiplier ?? 1)
const modes: Array<{ value: MultiplierSource, label: string }> = [
  { value: 'manual', label: '手动' },
  { value: 'upstream', label: '跟随上游' },
]

/** 等待文案区分正在查上游和正在保存手动倍率，让用户知道当前在等哪一步接口。 */
const statusText = computed(() => legacyT(
  pendingMode.value === 'upstream' ? '正在查询上游倍率…' : '正在保存倍率…',
))

/**
 * 失败只提示一次：行内模式由当前行显示错误，缺少行内错误区域的快捷入口才用全局弹窗。
 */
function reportFailure(message: string) {
  errorMessage.value = message
  if (props.actionOnly) showError(message)
}

watch(
  () => [props.apiKey.multiplier_sync?.source, props.apiKey.default_rate_multiplier] as const,
  ([nextSource, nextValue]) => {
    source.value = nextSource === 'upstream' ? 'upstream' : 'manual'
    displayValue.value = nextValue ?? 1
    if (!editing.value) {
      draftValue.value = displayValue.value
    }
  }
)

/** 点击倍率数字进入编辑态，并以服务端当前值初始化草稿。 */
function startEditing() {
  if (busy.value) return
  errorMessage.value = ''
  draftValue.value = displayValue.value
  editing.value = true
}

function cancelEditing() {
  if (busy.value) return
  editing.value = false
  errorMessage.value = ''
  draftValue.value = displayValue.value
}

/** 来源切换点一下即生效：切到上游立即同步，切回手动沿用当前倍率数值。 */
function handleSourceChange(value: string) {
  const nextSource: MultiplierSource = value === 'upstream' ? 'upstream' : 'manual'
  if (busy.value || nextSource === source.value) return
  errorMessage.value = ''

  if (nextSource === 'upstream') void runSync('upstream', undefined, true)
  else void runSync('manual', editing.value ? Number(draftValue.value) : displayValue.value, true)
}

/** 失焦或回车保存草稿倍率；失败时保留输入框，便于直接改正后重试。 */
async function saveManual() {
  if (busy.value || !editing.value) return
  const value = Number(draftValue.value)
  if (draftValue.value === '' || !Number.isFinite(value) || value < 0) {
    reportFailure(legacyT('请输入大于或等于 0 的有效倍率'))
    return
  }
  await runSync('manual', value, true)
}

/** 统一处理同步状态；失败时恢复来源选项并保留倍率草稿，原因同时显示在当前行。 */
async function runSync(mode?: MultiplierSource, multiplier?: number, closeOnSuccess = false) {
  if (busy.value) return
  busy.value = true
  pendingMode.value = mode ?? null
  errorMessage.value = ''

  try {
    const result = await syncProviderMultiplier(props.providerId, props.apiKey.id, mode, multiplier)
    const data = result.data as { failed?: number; results?: { error?: string }[] } | null
    const failureMessage = result.status !== 'success'
      ? result.message || legacyT('倍率操作失败，请稍后重试')
      : data?.failed
        ? data.results?.find(item => item.error)?.error || legacyT('同步失败，已保留当前倍率')
        : ''

    if (failureMessage) {
      reportFailure(failureMessage)
      return
    }

    success(legacyT('倍率状态已更新'))
    if (mode) {
      source.value = mode
      if (mode === 'manual' && multiplier !== undefined) {
        displayValue.value = multiplier
      }
    }
    if (closeOnSuccess) {
      editing.value = false
    }
  } catch {
    reportFailure(legacyT('倍率操作失败，请检查网络后重试'))
  } finally {
    busy.value = false
    pendingMode.value = null
    emit('refresh')
  }
}
</script>
