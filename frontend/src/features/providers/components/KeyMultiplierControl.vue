<template>
  <button v-if="actionOnly && source === 'upstream'" type="button" :disabled="busy" class="inline-flex shrink-0 items-center gap-1.5 rounded-md px-2 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-primary disabled:opacity-50" @click="runSync()">
    <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': busy }" />
    {{ legacyT(busy ? '同步中…' : '同步倍率') }}
  </button>
  <template v-else-if="!actionOnly">
    <button type="button" :aria-label="`${legacyT('倍率设置')} · ${apiKey.name}`" class="inline-flex items-center gap-2 rounded-md bg-muted/60 px-2.5 py-1 text-xs transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring" @click="openSettings">
      <span class="font-semibold tabular-nums text-foreground">{{ apiKey.default_rate_multiplier ?? 1 }}</span>
      <span class="h-1.5 w-1.5 rounded-full" :class="source === 'upstream' ? 'bg-primary' : 'bg-muted-foreground/50'" aria-hidden="true" />
      <span :class="source === 'upstream' ? 'text-primary' : 'text-muted-foreground'">{{ legacyT(source === 'upstream' ? '跟随上游' : '手动') }}</span>
      <ChevronDown class="h-3 w-3 text-muted-foreground" />
    </button>
    <Dialog :model-value="open" :title="legacyT('倍率设置')" :description="apiKey.name" size="sm" @update:model-value="setOpen">
      <div class="space-y-5">
        <div>
          <p class="mb-2 text-xs text-muted-foreground">{{ legacyT('倍率来源') }}</p>
          <div class="flex gap-1 rounded-lg bg-muted/60 p-1" role="group" :aria-label="legacyT('倍率来源')">
            <button v-for="mode in modes" :key="mode.value" type="button" :disabled="busy" :aria-pressed="draftSource === mode.value" class="flex-1 rounded-md px-3 py-2 text-sm transition-colors" :class="draftSource === mode.value ? 'bg-background text-primary shadow-sm' : 'text-muted-foreground hover:text-foreground'" @click="draftSource = mode.value">{{ legacyT(mode.label) }}</button>
          </div>
        </div>
        <div>
          <label :for="`rate-${apiKey.id}`" class="text-xs text-muted-foreground">{{ legacyT(draftSource === 'manual' ? '手动倍率' : '当前倍率') }}</label>
          <div class="relative mt-2">
            <input :id="`rate-${apiKey.id}`" v-model="draftValue" type="number" min="0" step="any" :disabled="busy || draftSource === 'upstream'" class="h-11 w-full rounded-lg border border-border bg-background px-3 pr-9 text-lg font-semibold tabular-nums focus:outline-none focus:ring-2 focus:ring-ring disabled:bg-muted/30">
            <span class="absolute right-3 top-2.5 text-muted-foreground">×</span>
          </div>
        </div>
        <p class="text-xs leading-relaxed text-muted-foreground">{{ legacyT(draftSource === 'upstream' ? '随余额监控周期更新。认证失效或同步失败时，继续使用当前倍率。' : '默认 1×。保存后固定使用你填写的倍率。') }}</p>
        <p v-if="source === 'upstream' && apiKey.multiplier_sync?.last_success_at" class="text-xs text-muted-foreground">{{ legacyT('最近成功同步') }} {{ new Date(apiKey.multiplier_sync.last_success_at).toLocaleString() }}</p>
      </div>
      <template #footer>
        <Button variant="outline" :disabled="busy" @click="setOpen(false)">{{ legacyT('取消') }}</Button>
        <Button :disabled="busy" @click="saveSettings">{{ legacyT(busy ? '保存中…' : '保存设置') }}</Button>
      </template>
    </Dialog>
  </template>
</template>
<script setup lang="ts">
import { computed, ref } from 'vue'
import { ChevronDown, RefreshCw } from 'lucide-vue-next'
import { Dialog, Button } from '@/components/ui'
import { useI18n } from '@/i18n'
import { useToast } from '@/composables/useToast'
import { syncProviderMultiplier } from '@/api/providerOps'
import type { EndpointAPIKey } from '@/api/endpoints'

/** 紧凑倍率入口；独立操作样式用于密钥行右侧。 */
const props = defineProps<{ apiKey: EndpointAPIKey; providerId: string; actionOnly?: boolean }>()
const emit = defineEmits<{ refresh: []; 'settings-open': [open: boolean] }>()
const { legacyT } = useI18n()
const { error: showError, success } = useToast()
const busy = ref(false)
const open = ref(false)
const source = computed(() => props.apiKey.multiplier_sync?.source || 'manual')
const draftSource = ref<'manual' | 'upstream'>('manual')
const draftValue = ref<string | number>(1)
const modes = [{ value: 'manual' as const, label: '手动设置' }, { value: 'upstream' as const, label: '跟随上游' }]

/** 同步通知父抽屉，模态设置期间禁止误关闭抽屉。 */
function setOpen(value: boolean) { open.value = value; emit('settings-open', value) }
/** 每次打开从已保存倍率生成草稿。 */
function openSettings() { draftSource.value = source.value; draftValue.value = props.apiKey.default_rate_multiplier ?? 1; setOpen(true) }
/** 校验手动输入后提交；上游能力由服务端查询结果确认。 */
function saveSettings() {
  if (draftSource.value === 'manual' && (draftValue.value === '' || !Number.isFinite(Number(draftValue.value)) || Number(draftValue.value) < 0)) {
    showError(legacyT('请输入大于或等于 0 的有效倍率')); return
  }
  void runSync(draftSource.value, draftSource.value === 'manual' ? Number(draftValue.value) : undefined)
}
/** 主动操作才弹提示，后台失败状态不在列表持续展示。 */
async function runSync(mode?: 'manual' | 'upstream', multiplier?: number) {
  if (busy.value) return
  busy.value = true
  try {
    const result = await syncProviderMultiplier(props.providerId, props.apiKey.id, mode, multiplier)
    const data = result.data as { failed?: number; results?: { error?: string }[] } | null
    if (result.status !== 'success') showError(result.message || legacyT('倍率操作失败，请稍后重试'))
    else if (data?.failed) showError(data.results?.find(item => item.error)?.error || legacyT('同步失败，已保留当前倍率'))
    else { success(legacyT('倍率状态已更新')); setOpen(false) }
  } catch { showError(legacyT('倍率操作失败，请检查网络后重试')) }
  finally { busy.value = false; emit('refresh') }
}
</script>
