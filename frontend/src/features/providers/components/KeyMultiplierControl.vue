<template>
  <div class="flex flex-wrap items-center gap-2 py-2 text-xs">
    <label :for="`multiplier-source-${apiKey.id}`">{{ legacyT('倍率来源') }}</label>
    <select :id="`multiplier-source-${apiKey.id}`" :value="apiKey.multiplier_sync?.source || 'manual'" :disabled="busy" class="h-8 rounded border border-border bg-background px-2" @change="changeSource">
      <option value="manual">{{ legacyT('手动倍率') }}</option>
      <option value="upstream">{{ legacyT('跟随上游') }}</option>
    </select>
    <button v-if="apiKey.multiplier_sync?.source === 'upstream'" type="button" :disabled="busy" :title="legacyT('同步上游倍率')" :aria-label="legacyT('同步上游倍率')" class="inline-flex h-8 w-8 items-center justify-center rounded hover:bg-muted disabled:opacity-50" @click="runSync()"><RefreshCw class="h-4 w-4" :class="{ 'animate-spin': busy }" /></button>
    <span v-if="apiKey.multiplier_sync?.source === 'upstream'" class="min-w-0 break-words text-muted-foreground">
      {{ apiKey.multiplier_sync.group_name || '' }}
      {{ legacyT(apiKey.multiplier_sync.status === 'success' ? '已同步' : apiKey.multiplier_sync.status === 'failed' ? '同步失败' : '尚未同步') }}
      {{ apiKey.multiplier_sync.last_success_at ? new Date(apiKey.multiplier_sync.last_success_at).toLocaleString() : '' }}
    </span>
    <p v-if="apiKey.multiplier_sync?.error" class="w-full break-words text-destructive">{{ apiKey.multiplier_sync.error }}</p>
  </div>
</template>
<script setup lang="ts">
import { ref } from 'vue'
import { RefreshCw } from 'lucide-vue-next'
import { useI18n } from '@/i18n'
import { useToast } from '@/composables/useToast'
import { syncProviderMultiplier } from '@/api/providerOps'
import type { EndpointAPIKey } from '@/api/endpoints'

/** 已保存密钥的倍率来源控件，切换立即持久化并刷新父级数据。 */
const props = defineProps<{
  /** 已保存的本地密钥和倍率状态。 */
  apiKey: EndpointAPIKey
  /** 所属供应商 ID，服务端再次校验归属。 */
  providerId: string
}>()
const emit = defineEmits<{ refresh: [] }>()
const { legacyT } = useI18n()
const { error: showError, success } = useToast()
const busy = ref(false)

/** 将原生选择值收窄为允许的倍率来源。 */
function changeSource(event: Event) {
  const value = (event.target as HTMLSelectElement).value
  if (value === 'manual' || value === 'upstream') void runSync(value)
}

/** 执行模式切换或单密钥同步，始终重读已持久化状态。 */
async function runSync(mode?: 'manual' | 'upstream') {
  if (busy.value) return
  busy.value = true
  try {
    const result = await syncProviderMultiplier(props.providerId, props.apiKey.id, mode)
    if (result.status !== 'success') showError(result.message || legacyT('同步失败'))
    else if ((result.data as { failed?: number }).failed) showError(legacyT('同步失败，请查看密钥详情'))
    else success(legacyT('倍率状态已更新'))
  } catch {
    showError(legacyT('倍率操作失败'))
  } finally {
    busy.value = false
    emit('refresh')
  }
}
</script>
