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
    <button
      v-if="!editing"
      type="button"
      :aria-label="`${legacyT('倍率设置')} · ${apiKey.name}`"
      class="inline-flex min-w-0 items-center gap-2 rounded-md bg-muted/60 px-2.5 py-1 text-xs transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
      :disabled="busy"
      @click="startEditing"
    >
      <span class="font-semibold tabular-nums text-foreground">{{ displayValue }}</span>
      <span
        class="h-1.5 w-1.5 shrink-0 rounded-full"
        :class="source === 'upstream' ? 'bg-primary' : 'bg-muted-foreground/50'"
        aria-hidden="true"
      />
      <span :class="source === 'upstream' ? 'text-primary' : 'text-muted-foreground'">
        {{ legacyT(source === 'upstream' ? '跟随上游' : '手动') }}
      </span>
      <ChevronDown class="h-3 w-3 shrink-0 text-muted-foreground" />
    </button>

    <div
      v-else
      class="inline-flex min-w-0 items-center gap-1 rounded-md border border-primary/30 bg-background p-0.5 shadow-sm"
    >
      <Input
        v-model="draftValue"
        type="number"
        min="0"
        step="any"
        :disabled="busy || draftSource === 'upstream'"
        :aria-label="legacyT('倍率')"
        :aria-describedby="errorMessage ? `multiplier-error-${apiKey.id}` : undefined"
        :autofocus="true"
        class="h-7 w-[4.5rem] border-0 bg-transparent px-1.5 text-xs font-semibold tabular-nums shadow-none focus-visible:ring-1 focus-visible:ring-primary/40"
        @keydown.enter.prevent="saveManual"
        @keydown.esc.prevent="cancelEditing"
      />

      <Select
        :model-value="draftSource"
        @update:model-value="handleSourceChange"
      >
        <SelectTrigger
          class="h-7 w-[5.5rem] shrink-0 rounded-md border-0 bg-transparent px-1.5 text-xs shadow-none focus:ring-1 focus:ring-primary/40"
          :aria-label="legacyT('倍率来源')"
          :aria-describedby="errorMessage ? `multiplier-error-${apiKey.id}` : undefined"
          :disabled="busy"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent :searchable="false">
          <SelectItem
            v-for="mode in modes"
            :key="mode.value"
            :value="mode.value"
          >
            {{ legacyT(mode.label) }}
          </SelectItem>
        </SelectContent>
      </Select>

      <Button
        variant="ghost"
        size="icon"
        class="h-7 w-7 rounded-md text-primary hover:bg-primary/10"
        :disabled="busy"
        :aria-label="legacyT(draftSource === 'manual' ? (busy ? '保存中…' : '保存倍率') : '完成编辑')"
        @click="commitEditing"
      >
        <Loader2
          v-if="busy"
          class="h-3.5 w-3.5 animate-spin"
        />
        <Check
          v-else
          class="h-3.5 w-3.5"
        />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        class="h-7 w-7 rounded-md text-muted-foreground hover:bg-muted hover:text-foreground"
        :disabled="busy"
        :aria-label="legacyT('取消')"
        @click="cancelEditing"
      >
        <X class="h-3.5 w-3.5" />
      </Button>
    </div>

    <p
      v-if="errorMessage"
      :id="`multiplier-error-${apiKey.id}`"
      class="basis-full rounded-md border border-destructive/20 bg-destructive/5 px-2 py-1.5 text-xs leading-4 text-destructive"
      role="alert"
    >
      {{ errorMessage }}
    </p>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { BadgePercent, Check, ChevronDown, Loader2, X } from 'lucide-vue-next'
import {
  Button,
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
const editing = ref(false)
const draftSource = ref<MultiplierSource>('manual')
const draftValue = ref<string | number>(1)
const errorMessage = ref('')
const source = ref<MultiplierSource>(props.apiKey.multiplier_sync?.source === 'upstream' ? 'upstream' : 'manual')
const displayValue = ref(props.apiKey.default_rate_multiplier ?? 1)
const modes: Array<{ value: MultiplierSource, label: string }> = [
  { value: 'manual', label: '手动' },
  { value: 'upstream', label: '跟随上游' },
]

watch(
  () => [props.apiKey.multiplier_sync?.source, props.apiKey.default_rate_multiplier] as const,
  ([nextSource, nextValue]) => {
    source.value = nextSource === 'upstream' ? 'upstream' : 'manual'
    displayValue.value = nextValue ?? 1
    if (!editing.value) {
      draftSource.value = source.value
      draftValue.value = displayValue.value
    }
  }
)

/** 从列表展示态进入编辑态，并以服务端当前值初始化草稿。 */
function startEditing() {
  if (busy.value) return
  errorMessage.value = ''
  draftSource.value = source.value
  draftValue.value = displayValue.value
  editing.value = true
}

function cancelEditing() {
  if (busy.value) return
  editing.value = false
  errorMessage.value = ''
}

function commitEditing() {
  if (draftSource.value === 'upstream') {
    editing.value = false
    errorMessage.value = ''
    return
  }
  void saveManual()
}

/** 来源切换立即提交；手动倍率数值仍通过 Enter 或确认按钮提交。 */
function handleSourceChange(value: string) {
  const nextSource: MultiplierSource = value === 'upstream' ? 'upstream' : 'manual'
  if (busy.value || nextSource === draftSource.value) return
  draftSource.value = nextSource
  errorMessage.value = ''

  if (nextSource === 'manual') {
    const value = Number(draftValue.value)
    if (!Number.isFinite(value) || value < 0) {
      errorMessage.value = legacyT('请输入大于或等于 0 的有效倍率')
      draftSource.value = source.value
      showError(errorMessage.value)
      return
    }
    void runSync('manual', value, true)
    return
  }

  void runSync('upstream', undefined, true)
}

async function saveManual() {
  if (busy.value) return
  const value = Number(draftValue.value)
  if (draftValue.value === '' || !Number.isFinite(value) || value < 0) {
    errorMessage.value = legacyT('请输入大于或等于 0 的有效倍率')
    showError(errorMessage.value)
    return
  }
  await runSync('manual', value, true)
}

/** 统一处理同步状态；失败时恢复来源选项并保留倍率草稿，原因同时显示在当前行。 */
async function runSync(mode?: MultiplierSource, multiplier?: number, closeOnSuccess = false) {
  if (busy.value) return
  busy.value = true
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
      errorMessage.value = failureMessage
      draftSource.value = source.value
      showError(failureMessage)
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
    const failureMessage = legacyT('倍率操作失败，请检查网络后重试')
    errorMessage.value = failureMessage
    draftSource.value = source.value
    showError(failureMessage)
  } finally {
    busy.value = false
    emit('refresh')
  }
}
</script>
