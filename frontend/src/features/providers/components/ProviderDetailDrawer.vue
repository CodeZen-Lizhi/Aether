<template>
  <!-- 自定义抽屉 -->
  <Teleport to="body">
    <Transition
      name="drawer"
      appear
    >
      <div
        v-if="open && (loading || provider)"
        class="fixed inset-0 z-50 flex justify-end"
        @click.self="handleBackdropClick"
      >
        <!-- 背景遮罩 -->
        <div
          class="absolute inset-0 bg-black/30"
          @click="handleBackdropClick"
        />

        <!-- 抽屉内容 -->
        <Card class="drawer-panel relative h-full w-full sm:w-[700px] sm:max-w-[90vw] rounded-none shadow-2xl overflow-y-auto">
          <!-- 加载状态 -->
          <div
            v-if="loading"
            class="flex items-center justify-center py-12"
          >
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
          </div>

          <template v-else-if="provider">
            <ProviderDetailHeader
              v-model:provider-proxy-popover-open="providerProxyPopoverOpen"
              :provider="provider"
              :endpoints="endpoints"
              :loading-provider-endpoints="loadingProviderEndpoints"
              :system-format-conversion-enabled="systemFormatConversionEnabled"
              :has-failover-rules="hasFailoverRules"
              :provider-proxy-node-name="getProviderProxyNodeName()"
              :saving-provider-proxy="savingProviderProxy"
              @toggle-format-conversion="toggleFormatConversion"
              @open-failover-rules="failoverRulesDialogOpen = true"
              @set-provider-proxy="setProviderProxy"
              @clear-provider-proxy="clearProviderProxy"
              @edit="$emit('edit', $event)"
              @toggle-status="$emit('toggleStatus', $event)"
              @close="handleClose"
              @edit-endpoint="handleEditEndpoint"
              @add-endpoint="showAddEndpointDialog"
            />

            <div class="space-y-6 p-4 sm:p-6">
              <!-- 配额使用情况 -->
              <ProviderMonthlyQuotaCard
                v-if="provider.billing_type === 'monthly_quota' && provider.monthly_quota_usd"
                :used="provider.monthly_used_usd"
                :quota="provider.monthly_quota_usd"
                :reset-day="provider.quota_reset_day"
              />

              <!-- 密钥管理 -->
              <Card class="overflow-hidden">
                <div class="p-4 border-b border-border/60">
                  <div class="flex items-center justify-between">
                    <h3 class="text-sm font-semibold">
                      {{ legacyT('密钥管理') }}
                    </h3>
                    <div class="flex flex-wrap items-center justify-end gap-2">
                      <Button
                        v-if="endpoints.length > 0"
                        variant="outline"
                        size="sm"
                        class="h-9"
                        @click="keyBatchImportDialogOpen = true"
                      >
                        <ListPlus class="mr-1.5 h-3.5 w-3.5" />
                        批量导入
                      </Button>
                      <Button
                        v-if="endpoints.length > 0"
                        variant="outline"
                        size="sm"
                        class="h-9"
                        @click="handleAddKeyToFirstEndpoint"
                      >
                        <Plus class="w-3.5 h-3.5 mr-1.5" />
                        {{ legacyT('添加密钥') }}
                      </Button>
                    </div>
                  </div>
                </div>

                <!-- 密钥列表 -->
                <div
                  v-if="loadingProviderKeys && allKeys.length === 0"
                  class="flex items-center justify-center gap-2 py-12 text-sm text-muted-foreground"
                >
                  <Loader2 class="w-4 h-4 animate-spin" />
                  {{ legacyT('正在加载') }}{{ legacyT('密钥') }}
                </div>

                <div
                  v-else-if="allKeys.length > 0"
                  class="divide-y divide-border/40"
                  :class="shouldPaginateKeys && 'flex flex-col'"
                >
                  <div
                    v-for="({ key, endpoint }, localIdx) in paginatedKeys"
                    :key="key.id"
                    class="px-4 py-2.5 hover:bg-muted/30 transition-colors group/item"
                    :class="{
                      'opacity-40 bg-muted/20': !key.is_active
                    }"
                  >
                    <!-- 第一行：名称 + 状态 + 操作按钮 -->
                    <div class="flex items-center justify-between gap-2">
                      <div class="flex items-center gap-2 flex-1 min-w-0">
                        <ProviderKeyIdentityBlock
                          :api-key="key"
                          :masked-secret-label="getProviderMaskedSecretLabel(key)"
                          @copy-name="copyToClipboard"
                          @copy-full-key="copyFullKey(key)"
                        />
                      </div>
                      <ProviderKeyActionCluster
                        :api-key="key"
                        :recoverable="isKeyRecoverable(key)"
                        :recover-title="getRecoverKeyTitle(key)"
                        :circuit-breaker-title="getKeyCircuitBreakerTitle(key)"
                        :circuit-probe-countdown="getKeyCircuitProbeCountdown(key)"
                        :health-score-bar-class="getHealthScoreBarColor(key.health_score || 0)"
                        :health-score-text-class="getHealthScoreColor(key.health_score || 0)"
                        :proxy-popover-open="proxyPopoverOpenKeyId === key.id"
                        :proxy-node-name="getKeyProxyNodeName(key)"
                        :saving-proxy="savingProxyKeyId === key.id"
                        :toggling="togglingKeyId === key.id"
                        @recover="handleRecoverKey(key)"
                        @permissions="handleKeyPermissions(key)"
                        @update:proxy-popover-open="(v: boolean) => handleProxyPopoverToggle(key.id, v)"
                        @clear-proxy="clearKeyProxy(key)"
                        @set-proxy="(v: string) => setKeyProxy(key, v)"
                        @edit="handleEditKey(endpoint, key)"
                        @toggle-active="toggleKeyActive(key)"
                        @delete="handleDeleteKey(key)"
                      />
                    </div>
                    <!-- 第二行：API 格式（展开显示） + 统计信息 -->
                    <div class="flex items-center gap-1.5 mt-1 text-[11px] text-muted-foreground">
                      <!-- 自动获取模型状态 -->
                      <template v-if="key.auto_fetch_models">
                        <span class="text-muted-foreground/40">|</span>
                        <span
                          class="cursor-help"
                          :class="key.last_models_fetch_error ? 'text-amber-600 dark:text-amber-400' : ''"
                          :title="getAutoFetchStatusTitle(key)"
                        >
                          {{ legacyT(key.last_models_fetch_error ? '同步失败' : '自动同步') }}
                        </span>
                      </template>
                      <!-- RPM 限制信息（第二位） -->
                      <template v-if="key.rpm_limit || key.is_adaptive">
                        <span class="text-muted-foreground/40">|</span>
                        <span v-if="key.is_adaptive">
                          {{ key.learned_rpm_limit != null ? `${key.learned_rpm_limit}` : legacyT('探测中') }} RPM
                          <span class="text-muted-foreground/60">({{ legacyT('自适应') }})</span>
                        </span>
                        <span v-else>{{ key.rpm_limit }} RPM</span>
                      </template>
                      <span class="text-muted-foreground/40">|</span>
                      <!-- API 格式：展开显示每个格式、倍率、熔断状态 -->
                      <template
                        v-for="(format, idx) in getKeyApiFormats(key, endpoint)"
                        :key="format"
                      >
                        <span
                          v-if="idx > 0"
                          class="text-muted-foreground/40"
                        >/</span>
                        <span :class="{ 'text-destructive': isFormatCircuitOpen(key, format) }">
                          {{ formatApiFormatShort(format) }}
                        </span>
                        <span
                          v-if="editingMultiplierKey !== key.id || editingMultiplierFormat !== format"
                          :title="legacyT('点击编辑倍率')"
                          class="cursor-pointer hover:text-primary hover:underline"
                          :class="{ 'text-destructive': isFormatCircuitOpen(key, format) }"
                          @click="startEditMultiplier(key, format)"
                        >{{ getKeyRateMultiplier(key, format) }}x</span>
                        <input
                          v-else
                          ref="multiplierInputRef"
                          v-model="editingMultiplierValue"
                          type="text"
                          inputmode="decimal"
                          pattern="[0-9]*\.?[0-9]*"
                          class="w-10 h-5 px-1 text-[11px] text-center border rounded bg-background focus:outline-none focus:ring-1 focus:ring-primary font-medium text-foreground/80"
                          @keydown="(e) => handleMultiplierKeydown(e, key, format)"
                          @blur="handleMultiplierBlur(key, format)"
                        >
                        <span
                          v-if="getFormatProbeCountdown(key, format)"
                          :class="{ 'text-destructive': isFormatCircuitOpen(key, format) }"
                        >{{ getFormatProbeCountdown(key, format) }}</span>
                      </template>
                    </div>
                  </div>
                  <!-- 分页控制 -->
                  <div
                    v-if="shouldPaginateKeys"
                    class="px-4 py-2 flex items-center justify-between text-xs text-muted-foreground mt-auto"
                  >
                    <span>{{ legacyT('共') }} {{ allKeys.length }} {{ legacyT('个') }}{{ legacyT('密钥') }}</span>
                    <div class="flex items-center gap-1.5">
                      <Button
                        variant="ghost"
                        size="sm"
                        class="h-6 px-2 text-xs"
                        :disabled="loadingProviderKeys || currentKeyPage <= 1"
                        @click="goToKeyPage(currentKeyPage - 1)"
                      >
                        ‹
                      </Button>
                      <span class="tabular-nums">{{ currentKeyPage }} / {{ totalKeyPages }}</span>
                      <Button
                        variant="ghost"
                        size="sm"
                        class="h-6 px-2 text-xs"
                        :disabled="loadingProviderKeys || currentKeyPage >= totalKeyPages"
                        @click="goToKeyPage(currentKeyPage + 1)"
                      >
                        ›
                      </Button>
                    </div>
                  </div>
                </div>

                <!-- 空状态 -->
                <div
                  v-else
                  class="p-8 text-center text-muted-foreground"
                >
                  <Key class="w-12 h-12 mx-auto mb-3 opacity-50" />
                  <p class="text-sm">
                    {{ legacyT('暂无密钥配置') }}
                  </p>
                  <p class="text-xs mt-1">
                    {{ endpoints.length > 0
                      ? legacyT('点击上方"添加密钥"按钮创建第一个密钥')
                      : legacyT('请先添加端点，然后再添加密钥') }}
                  </p>
                </div>
              </Card>

              <!-- 模型查看 -->
              <ModelsTab
                v-if="provider"
                :key="`models-${provider.id}`"
                :provider="provider"
                :models="providerModels"
                :endpoints="endpoints"
                :provider-keys="providerKeys"
                :loading="loadingProviderModels"
                @edit-model="handleEditModel"
                @batch-assign="handleBatchAssign"
                @refresh="loadEndpoints"
              />

              <!-- 模型映射 -->
              <ModelMappingTab
                v-if="provider"
                ref="modelMappingTabRef"
                :key="`mapping-${provider.id}`"
                :provider="provider"
                :endpoints="endpoints"
                :provider-keys="providerKeys"
                :models="providerModels"
                :mapping-preview="providerMappingPreview"
                :loading="loadingProviderMappingPreview"
                @refresh="handleModelMappingChanged"
              />
            </div>
          </template>
        </Card>
      </div>
    </Transition>
  </Teleport>

  <!-- 端点表单对话框（管理/编辑） -->
  <EndpointFormDialog
    v-if="provider && open && endpointDialogOpen"
    v-model="endpointDialogOpen"
    :provider="provider"
    :endpoints="endpoints"
    :system-format-conversion-enabled="systemFormatConversionEnabled"
    :provider-format-conversion-enabled="provider.enable_format_conversion"
    @endpoint-created="handleEndpointChanged"
    @endpoint-updated="handleEndpointChanged"
  />

  <!-- 密钥编辑对话框 -->
  <KeyFormDialog
    v-if="open && keyFormDialogOpen"
    :open="keyFormDialogOpen"
    :endpoint="currentEndpoint"
    :editing-key="editingKey"
    :provider-id="provider ? provider.id : null"
    :provider-type="provider?.provider_type || null"
    :available-api-formats="availableKeyApiFormats"
    @close="keyFormDialogOpen = false"
    @saved="handleKeyChanged"
  />

  <ProviderKeyBatchImportDialog
    v-if="open && keyBatchImportDialogOpen && provider"
    :open="keyBatchImportDialogOpen"
    :provider-id="provider.id"
    :provider-name="provider.name"
    :available-api-formats="availableKeyApiFormats"
    @close="keyBatchImportDialogOpen = false"
    @saved="handleKeyChanged"
  />

  <!-- 模型权限对话框 -->
  <KeyAllowedModelsEditDialog
    v-if="open && keyPermissionsDialogOpen"
    :open="keyPermissionsDialogOpen"
    :api-key="editingKey"
    :provider-id="providerId || ''"
    @close="keyPermissionsDialogOpen = false"
    @saved="handleKeyChanged"
  />

  <!-- 删除密钥确认对话框 -->
  <AlertDialog
    v-if="open && deleteKeyConfirmOpen"
    :model-value="deleteKeyConfirmOpen"
    :title="legacyT('删除密钥')"
    :description="formatDeleteKeyConfirmDescription()"
    :confirm-text="legacyT('删除')"
    :cancel-text="legacyT('取消')"
    type="danger"
    @update:model-value="deleteKeyConfirmOpen = $event"
    @confirm="confirmDeleteKey"
    @cancel="deleteKeyConfirmOpen = false"
  />

  <!-- 添加/编辑模型对话框 -->
  <ProviderModelFormDialog
    v-if="open && modelFormDialogOpen && provider"
    :open="modelFormDialogOpen"
    :provider-id="provider.id"
    :provider-name="provider.name"
    :editing-model="editingModel"
    @update:open="modelFormDialogOpen = $event"
    @saved="handleModelSaved"
  />

  <!-- 批量关联模型对话框 -->
  <BatchAssignModelsDialog
    v-if="open && batchAssignDialogOpen && provider"
    :open="batchAssignDialogOpen"
    :provider-id="provider.id"
    :provider-name="provider.name"
    @update:open="handleBatchAssignDialogOpenUpdate"
    @changed="handleBatchAssignChanged"
  />

  <!-- 故障转移规则弹窗 -->
  <FailoverRulesDialog
    v-if="open && failoverRulesDialogOpen"
    :open="failoverRulesDialogOpen"
    :provider="provider ?? null"
    @update:open="failoverRulesDialogOpen = $event"
    @saved="loadProvider()"
  />
</template>

<script setup lang="ts">
import { ref, watch, computed, nextTick } from 'vue'
import {
  Plus,
  Key,
  ListPlus,
  Loader2,
} from 'lucide-vue-next'
import { parseApiError } from '@/utils/errorParser'
import { useEscapeKey } from '@/composables/useEscapeKey'
import { useI18n } from '@/i18n'
import Button from '@/components/ui/button.vue'
import Card from '@/components/ui/card.vue'
import { useToast } from '@/composables/useToast'
import { useConfirm } from '@/composables/useConfirm'
import { useClipboard } from '@/composables/useClipboard'
import { useCountdownTimer, formatCountdown } from '@/composables/useCountdownTimer'
import {
  getProvider,
  getProviderEndpoints,
  updateProvider,
  getProviderModels,
  getProviderMappingPreview,
  type ProviderMappingPreviewResponse,
  type ProviderWithEndpointsSummary,
} from '@/api/endpoints'
import { adminApi } from '@/api/admin'
import {
  KeyFormDialog,
  KeyAllowedModelsEditDialog,
  ModelsTab,
  BatchAssignModelsDialog,
} from '@/features/providers/components'
import ModelMappingTab from '@/features/providers/components/provider-tabs/ModelMappingTab.vue'
import EndpointFormDialog from '@/features/providers/components/EndpointFormDialog.vue'
import ProviderModelFormDialog from '@/features/providers/components/ProviderModelFormDialog.vue'
import AlertDialog from '@/components/common/AlertDialog.vue'
import FailoverRulesDialog from '@/features/providers/components/FailoverRulesDialog.vue'
import ProviderDetailHeader from '@/features/providers/components/ProviderDetailHeader.vue'
import ProviderKeyBatchImportDialog from '@/features/providers/components/ProviderKeyBatchImportDialog.vue'
import ProviderKeyActionCluster from '@/features/providers/components/ProviderKeyActionCluster.vue'
import ProviderKeyIdentityBlock from '@/features/providers/components/ProviderKeyIdentityBlock.vue'
import ProviderMonthlyQuotaCard from '@/features/providers/components/ProviderMonthlyQuotaCard.vue'
import ProviderQuotaProgressRow from '@/features/providers/components/ProviderQuotaProgressRow.vue'
import ProviderQuotaSectionHeader from '@/features/providers/components/ProviderQuotaSectionHeader.vue'
import { useProxyNodesStore } from '@/stores/proxy-nodes'
import {
  deleteEndpointKey,
  recoverKeyHealth,
  getProviderKeysPage,
  updateProviderKey,
  revealEndpointKey,
  type ProviderEndpoint,
  type EndpointAPIKey,
  type Model,
  API_FORMAT_ORDER,
  sortApiFormats,
} from '@/api/endpoints'
import { formatApiFormatShort } from '@/api/endpoints/types/api-format'
import { formatCompactNumber } from '@/utils/format'

// 扩展端点类型,包含密钥列表
interface ProviderEndpointWithKeys extends ProviderEndpoint {
  keys?: EndpointAPIKey[]
  rpm_limit?: number
}

interface Props {
  providerId: string | null
  open: boolean
  initialProvider?: ProviderWithEndpointsSummary | null
}

const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
  (e: 'edit', provider: ProviderWithEndpointsSummary): void
  (e: 'toggleStatus', provider: ProviderWithEndpointsSummary): void
  (e: 'refresh'): void
}>()

const { error: showError, success: showSuccess, warning: showWarning } = useToast()
const { confirm } = useConfirm()
const { copyToClipboard } = useClipboard()
const { tick: countdownTick, start: startCountdownTimer, stop: stopCountdownTimer } = useCountdownTimer()
const { legacyT, locale } = useI18n()

function localizedApiError(err: unknown, fallback: string): string {
  return legacyT(parseApiError(err, fallback))
}

const loading = ref(false)
const provider = ref<ProviderWithEndpointsSummary | null>(null)
const endpoints = ref<ProviderEndpointWithKeys[]>([])
const providerKeys = ref<EndpointAPIKey[]>([])  // Provider 级别的 keys
const providerModels = ref<Model[]>([])  // Provider 级别的 models
const providerMappingPreview = ref<ProviderMappingPreviewResponse | null>(null)  // 映射预览
const loadingProviderEndpoints = ref(false)
const loadingProviderKeys = ref(false)
const loadingProviderModels = ref(false)
const loadingProviderMappingPreview = ref(false)
let providerLoadRequestId = 0
let endpointsLoadRequestId = 0
let keysLoadRequestId = 0
let mappingPreviewLoadRequestId = 0
const CUSTOM_PROVIDER_KEYS_PAGE_SIZE = 4

function applyProviderSnapshot(updated: ProviderWithEndpointsSummary): void {
  if (provider.value?.id === updated.id) {
    Object.assign(provider.value, updated)
    return
  }
  provider.value = updated
}

const systemFormatConversionEnabled = ref(false)

// 端点相关状态
const endpointDialogOpen = ref(false)

// 密钥相关状态
const keyFormDialogOpen = ref(false)
const keyBatchImportDialogOpen = ref(false)
const keyPermissionsDialogOpen = ref(false)
const currentEndpoint = ref<ProviderEndpoint | null>(null)
const editingKey = ref<EndpointAPIKey | null>(null)
const deleteKeyConfirmOpen = ref(false)
const keyToDelete = ref<EndpointAPIKey | null>(null)
const togglingKeyId = ref<string | null>(null)

// 密钥显示状态：key_id -> 完整密钥
const revealedKeys = ref<Map<string, string>>(new Map())

// 模型相关状态
const modelFormDialogOpen = ref(false)
const editingModel = ref<Model | null>(null)
const batchAssignDialogOpen = ref(false)
const modelMappingTabRef = ref<InstanceType<typeof ModelMappingTab> | null>(null)

const failoverRulesDialogOpen = ref(false)
const FAILOVER_RULE_ARRAY_KEYS = [
  'success_failover_patterns',
  'error_stop_patterns',
  'stop_status_codes',
  'stop_on_status_codes',
  'early_stop_status_codes',
  'non_retryable_status_codes',
  'continue_on_status_codes',
  'retryable_status_codes',
  'retry_on_status_codes',
  'continue_status_codes',
] as const
const hasFailoverRules = computed(() => {
  const rules = provider.value?.failover_rules
  if (!rules) return false
  return FAILOVER_RULE_ARRAY_KEYS.some(key => (rules[key]?.length || 0) > 0)
    || typeof rules.max_retries === 'number'
    || rules.stop_on_transport_errors === true
})

// Provider 级别代理配置状态
const proxyNodesStore = useProxyNodesStore()
const providerProxyPopoverOpen = ref(false)
const savingProviderProxy = ref(false)

// Key 级别代理配置状态
const savingProxyKeyId = ref<string | null>(null)
const proxyPopoverOpenKeyId = ref<string | null>(null)

// 点击编辑倍率相关状态
const editingMultiplierKey = ref<string | null>(null)
const editingMultiplierFormat = ref<string | null>(null)
const editingMultiplierValue = ref<number>(1.0)
const multiplierInputRef = ref<HTMLInputElement[] | null>(null)
const multiplierSaving = ref(false)

// 任意模态窗口打开时,阻止抽屉被误关闭
const hasBlockingDialogOpen = computed(() =>
  endpointDialogOpen.value ||
  keyFormDialogOpen.value ||
  keyBatchImportDialogOpen.value ||
  keyPermissionsDialogOpen.value ||
  deleteKeyConfirmOpen.value ||
  modelFormDialogOpen.value ||
  batchAssignDialogOpen.value ||
  modelMappingTabRef.value?.dialogOpen
)

// 当前后端分页页内的密钥列表。key 通过 api_formats 字段确定支持的格式，endpoint 可能为 undefined。
const allKeys = computed(() => {
  return providerKeys.value.map(key => ({ key, endpoint: undefined as ProviderEndpointWithKeys | undefined }))
})

const availableKeyApiFormats = computed(() => {
  const formatSet = new Set<string>()

  for (const format of provider.value?.api_formats || []) {
    if (format) {
      formatSet.add(format)
    }
  }

  for (const endpoint of endpoints.value) {
    if (endpoint.api_format) {
      formatSet.add(endpoint.api_format)
    }
  }

  return sortApiFormats([...formatSet])
})

function syncCurrentSelections(
  nextEndpoints: ProviderEndpointWithKeys[] = endpoints.value,
  nextProviderKeys: EndpointAPIKey[] = providerKeys.value
) {
  if (currentEndpoint.value) {
    currentEndpoint.value = nextEndpoints.find(endpoint => endpoint.id === currentEndpoint.value?.id) ?? null
  }

  if (!editingKey.value) {
    return
  }

  const latestKeys: EndpointAPIKey[] = []
  const seenKeyIds = new Set<string>()

  for (const key of nextProviderKeys) {
    if (!seenKeyIds.has(key.id)) {
      seenKeyIds.add(key.id)
      latestKeys.push(key)
    }
  }

  for (const endpoint of nextEndpoints) {
    for (const key of endpoint.keys || []) {
      if (!seenKeyIds.has(key.id)) {
        seenKeyIds.add(key.id)
        latestKeys.push(key)
      }
    }
  }

  const latestEditingKey = latestKeys.find(key => key.id === editingKey.value?.id) || null
  editingKey.value = latestEditingKey

  if (!latestEditingKey) {
    keyFormDialogOpen.value = false
    keyPermissionsDialogOpen.value = false
  }
}

// ===== 账号列表后端分页 =====
const providerKeysTotal = ref(0)
const currentKeyPage = ref(1)
const keyPageSize = ref(CUSTOM_PROVIDER_KEYS_PAGE_SIZE)
const totalKeyPages = computed(() => Math.max(1, Math.ceil(providerKeysTotal.value / keyPageSize.value)))
const shouldPaginateKeys = computed(() => totalKeyPages.value > 1)
const paginatedKeys = computed(() => allKeys.value)

async function goToKeyPage(page: number) {
  const nextPage = Math.min(Math.max(page, 1), totalKeyPages.value)
  if (nextPage === currentKeyPage.value && providerKeys.value.length > 0) return
  await loadProviderKeysPage(nextPage)
}

// 合并监听 providerId 和 open，避免同一 tick 内两个 watcher 都触发导致重复请求
watch(
  [() => props.providerId, () => props.open],
  async ([newId, newOpen], [_oldId, oldOpen]) => {
    if (newOpen && newId) {
      if (!oldOpen || provider.value?.id !== newId) {
        currentKeyPage.value = 1
        providerKeysTotal.value = 0
      }
      const hasInitialProvider = props.initialProvider?.id === newId
      if (hasInitialProvider) {
        provider.value = props.initialProvider
        keyPageSize.value = CUSTOM_PROVIDER_KEYS_PAGE_SIZE
        loading.value = false
      }
      void loadSystemFormatConversionConfig()
      if (!hasInitialProvider) {
        await loadProvider()
      }
      const endpointsPromise = loadEndpoints()
      // 仅在抽屉刚打开时启动倒计时
      if (newOpen && !oldOpen) {
        startCountdownTimer()
      }
      // 优先完成端点、密钥和模型的首屏数据，再请求计算量较大的映射预览。
      // 同时校验抽屉状态，避免关闭或切换 Provider 后启动无用请求。
      void endpointsPromise.then(() => {
        if (!props.open || props.providerId !== newId) return
        void loadMappingPreview()
      })
    } else if (!newOpen && oldOpen) {
      // 使在途请求失效，避免关闭后旧响应回写
      providerLoadRequestId += 1
      endpointsLoadRequestId += 1
      keysLoadRequestId += 1
      mappingPreviewLoadRequestId += 1

      // 停止倒计时定时器
      stopCountdownTimer()
      // 重置所有状态
      loading.value = false
      provider.value = null
      endpoints.value = []
      providerKeys.value = []  // 清空 Provider 级别的 keys
      providerKeysTotal.value = 0
      currentKeyPage.value = 1
      keyPageSize.value = CUSTOM_PROVIDER_KEYS_PAGE_SIZE
      providerModels.value = []
      providerMappingPreview.value = null
      loadingProviderEndpoints.value = false
      loadingProviderKeys.value = false
      loadingProviderModels.value = false
      loadingProviderMappingPreview.value = false

      // 重置所有对话框状态
      endpointDialogOpen.value = false
      keyFormDialogOpen.value = false
      keyBatchImportDialogOpen.value = false
      keyPermissionsDialogOpen.value = false
      deleteKeyConfirmOpen.value = false
      batchAssignDialogOpen.value = false

      // 重置临时数据
      currentEndpoint.value = null
      editingKey.value = null
      keyToDelete.value = null

      // 清除已显示的密钥（安全考虑）
      revealedKeys.value.clear()
    }
  },
  { immediate: true },
)

// 处理背景点击
function handleBackdropClick() {
  if (!hasBlockingDialogOpen.value) {
    handleClose()
  }
}

// 关闭抽屉
function handleClose() {
  if (!hasBlockingDialogOpen.value) {
    emit('update:open', false)
  }
}

// 切换格式转换开关
async function toggleFormatConversion() {
  if (!provider.value) return
  const newValue = !provider.value.enable_format_conversion
  try {
    const updated = await updateProvider(provider.value.id, { enable_format_conversion: newValue })
    applyProviderSnapshot(updated)
    showSuccess(legacyT(newValue ? '已启用格式转换' : '已禁用格式转换'))
    emit('refresh')
  } catch {
    showError(legacyT('切换格式转换失败'))
  }
}

function getProviderProxyNodeName(): string {
  const nodeId = provider.value?.proxy?.node_id
  if (!nodeId) return legacyT('未知节点')
  const node = proxyNodesStore.nodes.find(n => n.id === nodeId)
  return node ? node.name : `${nodeId.slice(0, 8)}...`
}

async function setProviderProxy(nodeId: string) {
  if (!provider.value) return
  savingProviderProxy.value = true
  try {
    const updated = await updateProvider(provider.value.id, {
      proxy: { node_id: nodeId, enabled: true },
    })
    applyProviderSnapshot(updated)
    providerProxyPopoverOpen.value = false
    showSuccess(legacyT('代理节点已设置'))
    emit('refresh')
  } catch (err: unknown) {
    showError(localizedApiError(err, '设置代理失败'))
  } finally {
    savingProviderProxy.value = false
  }
}

async function clearProviderProxy() {
  if (!provider.value) return
  savingProviderProxy.value = true
  try {
    const updated = await updateProvider(provider.value.id, { proxy: null })
    applyProviderSnapshot(updated)
    providerProxyPopoverOpen.value = false
    showSuccess(legacyT('已清除提供商代理'))
    emit('refresh')
  } catch (err: unknown) {
    showError(localizedApiError(err, '清除代理失败'))
  } finally {
    savingProviderProxy.value = false
  }
}

// 显示端点管理对话框
function showAddEndpointDialog() {
  endpointDialogOpen.value = true
}

// ===== 端点事件处理 =====
function handleEditEndpoint(_endpoint: ProviderEndpoint) {
  // 点击任何端点都打开管理对话框
  endpointDialogOpen.value = true
}

async function handleEndpointChanged() {
  await Promise.all([loadProvider(), loadEndpoints(), loadMappingPreview()])
  emit('refresh')
}

// ===== 密钥事件处理 =====
function handleAddKey(endpoint: ProviderEndpoint) {
  currentEndpoint.value = endpoint
  editingKey.value = null
  keyFormDialogOpen.value = true
}

// 添加密钥/账号（如果有多个端点则添加到第一个）
function handleAddKeyToFirstEndpoint() {
  if (endpoints.value.length === 0) return
  handleAddKey(endpoints.value[0])
}

function handleEditKey(endpoint: ProviderEndpoint | undefined, key: EndpointAPIKey) {
  currentEndpoint.value = endpoint || null
  editingKey.value = key
  keyFormDialogOpen.value = true
}

function handleKeyPermissions(key: EndpointAPIKey) {
  editingKey.value = key
  keyPermissionsDialogOpen.value = true
}

// 复制完整密钥或认证配置
function getProviderMaskedSecretLabel(key: EndpointAPIKey): string {
  return key.runtime_auth_kind === 'bearer' ? '[Bearer Token]' : '[Key]'
}

async function copyFullKey(key: EndpointAPIKey) {
  const cached = revealedKeys.value.get(key.id)
  if (cached) {
    copyToClipboard(cached)
    return
  }

  // 否则先获取再复制
  try {
    const result = await revealEndpointKey(key.id)
    let textToCopy: string

    if (result.auth_type === 'service_account' && result.auth_config) {
      // Service Account 类型：复制 auth_config JSON
      textToCopy = typeof result.auth_config === 'string'
        ? result.auth_config
        : JSON.stringify(result.auth_config, null, 2)
    } else {
      // API Key 类型：复制 api_key
      textToCopy = result.api_key || ''
    }

    revealedKeys.value.set(key.id, textToCopy)
    copyToClipboard(textToCopy)
  } catch (err: unknown) {
    showError(localizedApiError(err, '获取密钥失败'), legacyT('错误'))
  }
}

// 下载 OAuth 凭据文件（后端统一导出，前端只负责下载）
function handleDeleteKey(key: EndpointAPIKey) {
  keyToDelete.value = key
  deleteKeyConfirmOpen.value = true
}

function formatDeleteKeyConfirmDescription(): string {
  const keyName = keyToDelete.value?.api_key_masked || keyToDelete.value?.name || ''
  return locale.value === 'en-US'
    ? `Delete key ${keyName}?`
    : `确定要删除密钥 ${keyName} 吗？`
}

async function confirmDeleteKey() {
  if (!keyToDelete.value) return

  const keyId = keyToDelete.value.id
  deleteKeyConfirmOpen.value = false
  keyToDelete.value = null

  try {
    await deleteEndpointKey(keyId)
    showSuccess(legacyT('密钥已删除'))
    // 刷新端点列表及模型数据（删除 Key 触发自动解除模型关联）
    await Promise.all([loadProvider(), loadEndpoints(), loadMappingPreview()])
    emit('refresh')
  } catch (err: unknown) {
    showError(localizedApiError(err, '删除密钥失败'), legacyT('错误'))
  }
}

async function handleRecoverKey(key: EndpointAPIKey) {
  try {
    const result = await recoverKeyHealth(key.id)
    showSuccess(legacyT(result.message || 'Key已完全恢复'))
    await Promise.all([loadProvider(), loadEndpoints()])
    emit('refresh')
  } catch (err: unknown) {
    showError(localizedApiError(err, 'Key恢复失败'), legacyT('错误'))
  }
}

function applyUpdatedKeySnapshot(updatedKey: EndpointAPIKey) {
  const index = providerKeys.value.findIndex(key => key.id === updatedKey.id)
  if (index >= 0) {
    providerKeys.value.splice(index, 1, updatedKey)
  }
  if (editingKey.value?.id === updatedKey.id) {
    editingKey.value = updatedKey
  }
  syncCurrentSelections(endpoints.value, providerKeys.value)
}

async function handleKeyChanged(updatedKey?: EndpointAPIKey) {
  if (updatedKey) applyUpdatedKeySnapshot(updatedKey)
  await Promise.all([loadProvider(), loadEndpoints(), loadMappingPreview()])
  if (updatedKey) applyUpdatedKeySnapshot(updatedKey)
  emit('refresh')
}

// 切换密钥启用状态
async function toggleKeyActive(key: EndpointAPIKey) {
  if (togglingKeyId.value) return

  togglingKeyId.value = key.id
  try {
    const newStatus = !key.is_active
    const updated = await updateProviderKey(key.id, { is_active: newStatus })
    Object.assign(key, updated)
    key.is_active = newStatus
    await Promise.all([loadProvider(), loadEndpoints()])
    showSuccess(legacyT(newStatus ? '密钥已启用' : '密钥已停用'))
    emit('refresh')
  } catch (err: unknown) {
    showError(localizedApiError(err, '操作失败'), legacyT('错误'))
  } finally {
    togglingKeyId.value = null
  }
}

// ===== Key 级别代理配置 =====

/** 获取 Key 当前代理节点的名称（用于显示） */
function getKeyProxyNodeName(key: EndpointAPIKey): string | null {
  if (!key.proxy?.node_id) return null
  const node = proxyNodesStore.nodes.find(n => n.id === key.proxy?.node_id)
  return node ? node.name : `${key.proxy.node_id.slice(0, 8)  }...`
}

/** 切换代理 Popover 的打开/关闭状态 */
function handleProxyPopoverToggle(keyId: string, open: boolean) {
  proxyPopoverOpenKeyId.value = open ? keyId : null
  if (open) {
    proxyNodesStore.ensureLoaded()
  }
}

/** 设置 Key 的代理节点 */
async function setKeyProxy(key: EndpointAPIKey, nodeId: string) {
  savingProxyKeyId.value = key.id
  try {
    await updateProviderKey(key.id, {
      proxy: { node_id: nodeId, enabled: true },
    })
    key.proxy = { node_id: nodeId, enabled: true }
    proxyPopoverOpenKeyId.value = null
    showSuccess(legacyT('代理节点已设置'))
    emit('refresh')
  } catch (err: unknown) {
    showError(localizedApiError(err, '设置代理失败'), legacyT('错误'))
  } finally {
    savingProxyKeyId.value = null
  }
}

/** 清除 Key 的代理节点（回退到 Provider 级别代理） */
async function clearKeyProxy(key: EndpointAPIKey) {
  savingProxyKeyId.value = key.id
  try {
    await updateProviderKey(key.id, { proxy: null })
    key.proxy = null
    proxyPopoverOpenKeyId.value = null
    showSuccess(legacyT('已清除账号代理，将使用提供商级别代理'))
    emit('refresh')
  } catch (err: unknown) {
    showError(localizedApiError(err, '清除代理失败'), legacyT('错误'))
  } finally {
    savingProxyKeyId.value = null
  }
}

// ===== 模型事件处理 =====
// 处理编辑模型
function handleEditModel(model: Model) {
  editingModel.value = model
  modelFormDialogOpen.value = true
}

// 处理打开批量关联对话框
function handleBatchAssign() {
  batchAssignDialogOpen.value = true
}

function handleBatchAssignDialogOpenUpdate(value: boolean) {
  batchAssignDialogOpen.value = value
}

// 处理批量关联完成
async function handleBatchAssignChanged() {
  await Promise.all([loadProvider(), loadEndpoints(), loadMappingPreview()])
  emit('refresh')
}

// 处理模型映射变更
async function handleModelMappingChanged() {
  await Promise.all([loadProvider(), loadEndpoints(), loadMappingPreview()])
  emit('refresh')
}

// 处理模型保存完成
async function handleModelSaved() {
  editingModel.value = null
  await Promise.all([loadProvider(), loadEndpoints(), loadMappingPreview()])
  emit('refresh')
}

// ===== 点击编辑优先级 =====
function startEditMultiplier(key: EndpointAPIKey, format: string) {
  editingMultiplierKey.value = key.id
  editingMultiplierFormat.value = format
  editingMultiplierValue.value = getKeyRateMultiplier(key, format)
  multiplierSaving.value = false
  nextTick(() => {
    const input = Array.isArray(multiplierInputRef.value) ? multiplierInputRef.value[0] : multiplierInputRef.value
    input?.focus()
    input?.select()
  })
}

function cancelEditMultiplier() {
  editingMultiplierKey.value = null
  editingMultiplierFormat.value = null
}

function handleMultiplierKeydown(e: KeyboardEvent, key: EndpointAPIKey, format: string) {
  if (e.key === 'Enter') {
    e.preventDefault()
    e.stopPropagation()
    saveMultiplier(key, format)
  } else if (e.key === 'Escape') {
    e.preventDefault()
    multiplierSaving.value = true // 阻止 blur 触发保存
    cancelEditMultiplier()
  }
}

function handleMultiplierBlur(key: EndpointAPIKey, format: string) {
  if (multiplierSaving.value) return
  saveMultiplier(key, format)
}

async function saveMultiplier(key: EndpointAPIKey, format: string) {
  // 防止重复调用（Enter 触发后阻止 blur 再次进入）
  if (multiplierSaving.value) return
  multiplierSaving.value = true

  const keyId = editingMultiplierKey.value
  const newMultiplier = parseFloat(String(editingMultiplierValue.value))

  // 验证输入有效性
  if (!keyId || isNaN(newMultiplier)) {
    showError(legacyT('请输入有效的倍率值'))
    cancelEditMultiplier()
    multiplierSaving.value = false
    return
  }

  // 验证合理范围
  if (newMultiplier <= 0 || newMultiplier > 100) {
    showError(legacyT('倍率必须在 0.01 到 100 之间'))
    cancelEditMultiplier()
    multiplierSaving.value = false
    return
  }

  // 如果倍率没有变化,直接取消编辑（使用精度容差比较浮点数）
  const currentMultiplier = getKeyRateMultiplier(key, format)
  if (Math.abs(currentMultiplier - newMultiplier) < 0.0001) {
    cancelEditMultiplier()
    multiplierSaving.value = false
    return
  }

  cancelEditMultiplier()

  try {
    // 构建 rate_multipliers 对象
    const rateMultipliers = { ...(key.rate_multipliers || {}) }
    rateMultipliers[format] = newMultiplier

    await updateProviderKey(keyId, { rate_multipliers: rateMultipliers })
    showSuccess(legacyT('倍率已更新'))

    // 更新本地数据
    const keyToUpdate = providerKeys.value.find(k => k.id === keyId)
    if (keyToUpdate) {
      keyToUpdate.rate_multipliers = rateMultipliers
    }
    emit('refresh')
  } catch (err: unknown) {
    showError(localizedApiError(err, '更新倍率失败'), legacyT('错误'))
  } finally {
    multiplierSaving.value = false
  }
}

// 获取密钥的 API 格式列表（按指定顺序排序）
function getKeyApiFormats(key: EndpointAPIKey, endpoint?: ProviderEndpointWithKeys): string[] {
  let formats: string[] = []
  if (key.api_formats && key.api_formats.length > 0) {
    formats = [...key.api_formats]
  } else if (endpoint) {
    formats = [endpoint.api_format]
  }
  // 使用统一的排序函数
  return sortApiFormats(formats)
}

// 获取密钥在指定 API 格式下的成本倍率
function getKeyRateMultiplier(key: EndpointAPIKey, format: string): number {
  if (key.rate_multipliers && key.rate_multipliers[format] !== undefined) {
    return key.rate_multipliers[format]
  }
  return 1.0
}

// OAuth 订阅类型格式化
function getQuotaRemainingClass(usedPercent: number): string {
  const remaining = 100 - usedPercent
  if (remaining <= 10) return 'text-red-600 dark:text-red-400'
  if (remaining <= 30) return 'text-yellow-600 dark:text-yellow-400'
  return 'text-green-600 dark:text-green-400'
}

// Codex 剩余额度进度条颜色
function getQuotaRemainingBarColor(usedPercent: number): string {
  const remaining = 100 - usedPercent
  if (remaining <= 10) return 'bg-red-500 dark:bg-red-400'
  if (remaining <= 30) return 'bg-yellow-500 dark:bg-yellow-400'
  return 'bg-green-500 dark:bg-green-400'
}

// 判断是否为 Codex Team/Plus/Enterprise 账号（有 5H 限额，显示 3 列）
function getHealthScoreColor(score: number): string {
  if (score >= 0.8) return 'text-green-600 dark:text-green-400'
  if (score >= 0.5) return 'text-yellow-600 dark:text-yellow-400'
  return 'text-red-600 dark:text-red-400'
}

function getHealthScoreBarColor(score: number): string {
  if (score >= 0.8) return 'bg-green-500 dark:bg-green-400'
  if (score >= 0.5) return 'bg-yellow-500 dark:bg-yellow-400'
  return 'bg-red-500 dark:bg-red-400'
}

function isKeyRecoverable(key: EndpointAPIKey): boolean {
  return Boolean(
    key.circuit_breaker_open
    || (key.health_score !== undefined && key.health_score < 0.5)
  )
}

function getOpenCircuitEntries(key: EndpointAPIKey): Array<[string, NonNullable<EndpointAPIKey['circuit_breaker_by_format']>[string]]> {
  return Object.entries(key.circuit_breaker_by_format || {})
    .filter(([, value]) => value?.open === true)
}

function getKeyCircuitProbeCountdown(key: EndpointAPIKey): string {
  void countdownTick.value
  const nextProbe = getOpenCircuitEntries(key)
    .map(([, value]) => {
      if (typeof value.next_probe_at_unix_secs === 'number' && Number.isFinite(value.next_probe_at_unix_secs)) {
        return value.next_probe_at_unix_secs * 1000
      }
      if (value.next_probe_at) {
        const ms = new Date(value.next_probe_at).getTime()
        return Number.isFinite(ms) ? ms : null
      }
      return null
    })
    .filter((value): value is number => value !== null)
    .sort((a, b) => a - b)[0]
  if (!nextProbe) {
    return ''
  }
  const diffMs = nextProbe - Date.now()
  return diffMs > 0 ? ` ${formatCountdown(diffMs)}` : ` ${legacyT('探测中')}`
}

function getKeyCircuitBreakerTitle(key: EndpointAPIKey): string {
  const entries = getOpenCircuitEntries(key)
  if (entries.length === 0) return legacyT('熔断器已打开')
  const parts = entries.map(([format, value]) => {
    const label = formatApiFormatShort(format)
    const reason = value.reason ? `${legacyT('原因')}: ${value.reason}` : `${legacyT('原因')}: ${legacyT('连续失败')}`
    const interval = typeof value.probe_interval_minutes === 'number'
      ? `${legacyT('探测间隔')}: ${value.probe_interval_minutes} ${legacyT('分钟')}`
      : ''
    const countdown = getFormatProbeCountdown(key, format).trim()
    return [label, reason, interval, countdown ? `${legacyT('状态')}: ${countdown}` : '']
      .filter(Boolean)
      .join(' / ')
  })
  parts.push(legacyT('点击恢复按钮可重置熔断器'))
  return parts.join('\n')
}

function getRecoverKeyTitle(key: EndpointAPIKey): string {
  if (key.circuit_breaker_open) {
    return legacyT('重置熔断器并恢复健康状态')
  }
  return legacyT('刷新健康状态')
}

// 获取自动获取模型状态的 title 提示
function getAutoFetchStatusTitle(key: EndpointAPIKey): string {
  const parts: string[] = [legacyT('自动获取模型已启用')]

  if (key.last_models_fetch_at) {
    const date = new Date(key.last_models_fetch_at)
    parts.push(`${legacyT('上次同步')}: ${date.toLocaleString(locale.value)}`)
  }

  if (key.last_models_fetch_error) {
    parts.push(`${legacyT('错误')}: ${key.last_models_fetch_error}`)
  }

  return parts.join('\n')
}

// 检查指定格式是否熔断
function isFormatCircuitOpen(key: EndpointAPIKey, format: string): boolean {
  if (!key.circuit_breaker_by_format) return false
  const formatData = key.circuit_breaker_by_format[format]
  return formatData?.open === true
}

// 获取指定格式的探测倒计时（如果熔断，返回带空格前缀的倒计时文本）
function getFormatProbeCountdown(key: EndpointAPIKey, format: string): string {
  // 触发响应式更新
  void countdownTick.value

  if (!key.circuit_breaker_by_format) return ''
  const formatData = key.circuit_breaker_by_format[format]
  if (!formatData?.open) return ''

  // 半开状态
  if (formatData.half_open_until) {
    const halfOpenUntil = new Date(formatData.half_open_until)
    const now = new Date()
    if (halfOpenUntil > now) {
      return ` ${legacyT('探测中')}`
    }
  }
  // 等待探测
  if (formatData.next_probe_at_unix_secs || formatData.next_probe_at) {
    const nextProbeMs = typeof formatData.next_probe_at_unix_secs === 'number'
      ? formatData.next_probe_at_unix_secs * 1000
      : new Date(formatData.next_probe_at || '').getTime()
    const diffMs = nextProbeMs - Date.now()
    if (diffMs > 0) {
      return ` ${formatCountdown(diffMs)}`
    } else {
      return ` ${legacyT('探测中')}`
    }
  }
  return ''
}

// 加载系统级格式转换配置
async function loadSystemFormatConversionConfig() {
  try {
    const result = await adminApi.getSystemConfig('enable_format_conversion')
    systemFormatConversionEnabled.value = result.value === true
  } catch {
    // 获取失败时默认为关闭
    systemFormatConversionEnabled.value = false
  }
}

// 加载 Provider 信息
async function loadProvider() {
  if (!props.providerId) return
  const requestId = ++providerLoadRequestId
  const shouldShowSpinner = !provider.value || provider.value.id !== props.providerId

  try {
    if (shouldShowSpinner) {
      loading.value = true
    }
    // 系统级格式转换配置只影响一个图标状态，不应阻塞详情抽屉首屏。
    void loadSystemFormatConversionConfig()
    const providerData = await getProvider(props.providerId)
    if (requestId !== providerLoadRequestId) return
    applyProviderSnapshot(providerData)
    keyPageSize.value = CUSTOM_PROVIDER_KEYS_PAGE_SIZE

    if (!provider.value) {
      throw new Error(legacyT('Provider 不存在'))
    }
  } catch (err: unknown) {
    if (requestId !== providerLoadRequestId) return
    showError(localizedApiError(err, '加载失败'), legacyT('错误'))
  } finally {
    if (requestId === providerLoadRequestId && shouldShowSpinner) {
      loading.value = false
    }
  }
}

async function loadProviderKeysPage(page = currentKeyPage.value) {
  if (!props.providerId) return
  const providerId = props.providerId
  const requestId = ++keysLoadRequestId
  loadingProviderKeys.value = true

  try {
    const result = await getProviderKeysPage(providerId, {
      page,
      page_size: keyPageSize.value,
    })
    if (requestId !== keysLoadRequestId || props.providerId !== providerId) return

    const nextTotalPages = Math.max(1, Math.ceil(result.total / result.page_size))
    if (result.keys.length === 0 && result.total > 0 && result.page > nextTotalPages) {
      await loadProviderKeysPage(nextTotalPages)
      return
    }

    providerKeys.value = result.keys
    providerKeysTotal.value = result.total
    currentKeyPage.value = Math.min(result.page, nextTotalPages)
    keyPageSize.value = result.page_size
    syncCurrentSelections(endpoints.value, result.keys)
  } catch (err: unknown) {
    if (requestId !== keysLoadRequestId || props.providerId !== providerId) return
    providerKeys.value = []
    providerKeysTotal.value = 0
    syncCurrentSelections(endpoints.value, [])
    showError(localizedApiError(err, '加载密钥失败'), legacyT('错误'))
  } finally {
    if (requestId === keysLoadRequestId) {
      loadingProviderKeys.value = false
    }
  }
}

// 加载端点列表
async function loadEndpoints() {
  if (!props.providerId) return
  const providerId = props.providerId
  const requestId = ++endpointsLoadRequestId
  loadingProviderEndpoints.value = true
  loadingProviderKeys.value = true
  loadingProviderModels.value = true

  const sortEndpoints = (items: ProviderEndpoint[]): ProviderEndpointWithKeys[] => {
    return [...items].sort((a, b) => {
      const aIdx = API_FORMAT_ORDER.indexOf(a.api_format)
      const bIdx = API_FORMAT_ORDER.indexOf(b.api_format)
      if (aIdx === -1 && bIdx === -1) return 0
      if (aIdx === -1) return 1
      if (bIdx === -1) return -1
      return aIdx - bIdx
    })
  }

  const endpointsPromise = getProviderEndpoints(providerId)
    .then((endpointsList) => {
      if (requestId !== endpointsLoadRequestId) return
      const sortedEndpoints = sortEndpoints(endpointsList)
      endpoints.value = sortedEndpoints
      syncCurrentSelections(sortedEndpoints, providerKeys.value)
    })
    .catch((err: unknown) => {
      if (requestId !== endpointsLoadRequestId) return
      endpoints.value = []
      syncCurrentSelections([], providerKeys.value)
      showError(localizedApiError(err, '加载端点失败'), legacyT('错误'))
    })
    .finally(() => {
      if (requestId === endpointsLoadRequestId) {
        loadingProviderEndpoints.value = false
      }
    })

  const providerKeysPromise = loadProviderKeysPage(currentKeyPage.value)

  const modelsPromise = getProviderModels(providerId)
    .catch(() => [])
    .then((modelsResult) => {
      if (requestId !== endpointsLoadRequestId) return
      providerModels.value = modelsResult
    })
    .finally(() => {
      if (requestId === endpointsLoadRequestId) {
        loadingProviderModels.value = false
      }
    })

  await Promise.allSettled([endpointsPromise, providerKeysPromise, modelsPromise])
}

// 加载映射预览（独立于 loadEndpoints，不阻塞首屏渲染）
async function loadMappingPreview() {
  if (!props.providerId) return
  const requestId = ++mappingPreviewLoadRequestId
  loadingProviderMappingPreview.value = true
  try {
    const preview = await getProviderMappingPreview(props.providerId)
    if (requestId !== mappingPreviewLoadRequestId) return
    providerMappingPreview.value = preview
  } catch {
    if (requestId !== mappingPreviewLoadRequestId) return
    providerMappingPreview.value = null
  } finally {
    if (requestId === mappingPreviewLoadRequestId) {
      loadingProviderMappingPreview.value = false
    }
  }
}

// 添加 ESC 键监听
useEscapeKey(() => {
  if (props.open) {
    handleClose()
  }
}, {
  disableOnInput: true,
  once: false
})
</script>

<style scoped>
/* 抽屉过渡动画 */
.drawer-enter-active,
.drawer-leave-active {
  transition: opacity 0.3s ease;
}

.drawer-enter-active .drawer-panel,
.drawer-leave-active .drawer-panel {
  transition: transform 0.3s ease;
}

.drawer-enter-from,
.drawer-leave-to {
  opacity: 0;
}

.drawer-enter-from .drawer-panel {
  transform: translateX(100%);
}

.drawer-leave-to .drawer-panel {
  transform: translateX(100%);
}

.drawer-enter-to .drawer-panel,
.drawer-leave-from .drawer-panel {
  transform: translateX(0);
}

/* 轻量滚动条（用于 Antigravity 模型配额等小区域） */
.custom-scrollbar::-webkit-scrollbar {
  width: 4px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background-color: hsl(var(--muted-foreground) / 0.2);
  border-radius: 4px;
}
.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background-color: hsl(var(--muted-foreground) / 0.4);
}
</style>
