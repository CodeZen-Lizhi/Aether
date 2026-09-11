<template>
  <div class="space-y-6 pb-8">
    <TableCard
      title="独立余额 API Keys"
      class="responsive-list"
    >
      <template #actions>
        <!-- 创建独立 Key 按钮 -->
        <Button
          variant="ghost"
          size="icon"
          class="h-8 w-8"
          title="创建独立 Key"
          @click="openCreateDialog"
        >
          <Plus class="w-3.5 h-3.5" />
        </Button>

        <!-- 刷新按钮 -->
        <RefreshButton
          :loading="loading"
          @click="refreshApiKeys"
        />
      </template>

      <!-- 加载状态 -->
      <LoadingState
        v-if="loading"
        message="加载中..."
        size="lg"
      />

      <div v-else>
        <div class="responsive-list-table">
          <Table>
            <TableHeader>
              <TableRow class="border-b border-border/60 hover:bg-transparent">
                <TableHead class="w-[22%] h-12 font-semibold">
                  密钥信息
                </TableHead>
                <TableHead class="w-[190px] h-12 font-semibold">
                  统计/限制
                </TableHead>
                <TableHead class="w-[16%] h-12 font-semibold">
                  创建时间
                </TableHead>
                <TableHead class="w-[12%] h-12 font-semibold">
                  有效期
                </TableHead>
                <TableHead class="w-[16%] h-12 font-semibold">
                  最近使用
                </TableHead>
                <TableHead class="w-[10%] h-12 font-semibold">
                  状态
                </TableHead>
                <TableHead class="w-[14%] h-12 font-semibold text-center">
                  操作
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="apiKeys.length === 0">
                <TableCell
                  colspan="7"
                  class="h-64 text-center"
                >
                  <EmptyState
                    type="empty"
                    :icon="Key"
                    title="暂无独立余额 Key"
                    description="点击右上角按钮创建独立余额 Key"
                    size="sm"
                  />
                </TableCell>
              </TableRow>
              <TableRow
                v-for="apiKey in apiKeys"
                :key="apiKey.id"
                class="border-b border-border/40 hover:bg-muted/30 transition-colors"
              >
                <TableCell class="py-4">
                  <div class="space-y-1">
                    <div
                      class="text-sm font-medium text-foreground truncate"
                      :title="apiKey.name || '未命名 Key'"
                    >
                      {{ apiKey.name || '未命名 Key' }}
                    </div>
                    <div class="flex items-center gap-1.5">
                      <code class="text-xs font-mono text-muted-foreground">
                        {{ apiKey.key_display || '****' }}
                      </code>
                      <Button
                        variant="ghost"
                        size="icon"
                        class="h-6 w-6"
                        title="复制完整密钥"
                        @click="copyKeyPrefix(apiKey)"
                      >
                        <Copy class="h-3 w-3" />
                      </Button>
                    </div>
                  </div>
                </TableCell>
                <TableCell class="py-4">
                  <div class="space-y-1 text-xs">
                    <div class="text-muted-foreground">
                      请求: <span class="font-medium text-foreground">{{ (apiKey.total_requests || 0).toLocaleString() }}</span>
                    </div>
                    <div class="text-muted-foreground">
                      Tokens: <span class="font-medium text-foreground">{{ formatApiKeyTotalTokens(apiKey) }}</span>
                    </div>
                    <div class="flex items-center gap-1 text-muted-foreground">
                      <span>限速:</span>
                      <Badge
                        v-if="isRateLimitInherited(apiKey.rate_limit) || isRateLimitUnlimited(apiKey.rate_limit)"
                        variant="secondary"
                        class="h-5 px-1.5 py-0 text-[10px] font-medium"
                      >
                        {{ formatRateLimitInheritable(apiKey.rate_limit) }}
                      </Badge>
                      <span
                        v-else
                        class="font-medium text-foreground"
                      >
                        {{ formatRateLimitInheritable(apiKey.rate_limit) }}
                      </span>
                    </div>
                    <div class="flex items-center gap-1 text-muted-foreground">
                      <span>并发:</span>
                      <Badge
                        v-if="isConcurrentLimitInherited(apiKey.concurrent_limit) || isConcurrentLimitUnlimited(apiKey.concurrent_limit)"
                        variant="secondary"
                        class="h-5 px-1.5 py-0 text-[10px] font-medium"
                      >
                        {{ formatConcurrentLimitInheritable(apiKey.concurrent_limit) }}
                      </Badge>
                      <span
                        v-else
                        class="font-medium text-foreground"
                      >
                        {{ formatConcurrentLimitInheritable(apiKey.concurrent_limit) }}
                      </span>
                    </div>
                  </div>
                </TableCell>
                <TableCell class="py-4">
                  <div class="text-xs">
                    <span class="text-foreground">{{ formatDate(apiKey.created_at) }}</span>
                  </div>
                </TableCell>
                <TableCell class="py-4">
                  <div class="text-xs">
                    <div
                      v-if="apiKey.expires_at"
                      class="space-y-1"
                    >
                      <div class="text-foreground">
                        {{ formatDate(apiKey.expires_at) }}
                      </div>
                      <div class="text-muted-foreground">
                        {{ getRelativeTime(apiKey.expires_at) }}
                      </div>
                    </div>
                    <div
                      v-else
                      class="text-muted-foreground"
                    >
                      永不过期
                    </div>
                  </div>
                </TableCell>
                <TableCell class="py-4">
                  <div class="text-xs">
                    <span
                      v-if="apiKey.last_used_at"
                      class="text-foreground"
                    >{{ formatDate(apiKey.last_used_at) }}</span>
                    <span
                      v-else
                      class="text-muted-foreground"
                    >暂无记录</span>
                  </div>
                </TableCell>
                <TableCell class="w-[10%] py-4">
                  <div class="flex flex-col items-start gap-1.5">
                    <Badge
                      :variant="apiKey.is_active ? 'success' : 'destructive'"
                      class="h-5 px-1.5 py-0 text-[10px] font-medium"
                    >
                      {{ apiKey.is_active ? '活跃' : '禁用' }}
                    </Badge>
                  </div>
                </TableCell>
                <TableCell class="py-4">
                  <div class="flex justify-center gap-1">
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7"
                      title="编辑"
                      @click="editApiKey(apiKey)"
                    >
                      <SquarePen class="h-3.5 w-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7"
                      :title="apiKey.is_active ? '禁用' : '启用'"
                      @click="toggleApiKey(apiKey)"
                    >
                      <Power class="h-3.5 w-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7"
                      title="删除"
                      @click="deleteApiKey(apiKey)"
                    >
                      <Trash2 class="h-3.5 w-3.5" />
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>

        <div class="responsive-list-mobile">
          <EmptyState
            v-if="apiKeys.length === 0"
            class="px-4 py-12"
            type="empty"
            :icon="Key"
            title="暂无独立余额 Key"
            description="点击右上角按钮创建独立余额 Key"
            size="sm"
          />

          <div
            v-else
            class="divide-y divide-border/50"
          >
            <article
              v-for="apiKey in apiKeys"
              :key="apiKey.id"
              class="api-key-card p-4 sm:p-5"
            >
              <div class="api-key-card__header flex items-start justify-between gap-3">
                <div class="min-w-0 flex-1">
                  <div class="flex min-w-0 items-center gap-2">
                    <h4
                      class="min-w-0 break-words text-sm font-semibold text-foreground"
                      :class="{ 'text-muted-foreground': !apiKey.name }"
                    >
                      {{ apiKey.name || '未命名 Key' }}
                    </h4>
                    <Badge
                      :variant="apiKey.is_active ? 'success' : 'destructive'"
                      class="h-5 shrink-0 px-1.5 py-0 text-[10px] font-medium"
                    >
                      {{ apiKey.is_active ? '活跃' : '禁用' }}
                    </Badge>
                    <Badge
                      v-if="apiKey.auto_delete_on_expiry"
                      variant="secondary"
                      class="hidden h-5 shrink-0 px-1.5 py-0 text-[10px] font-medium sm:inline-flex"
                    >
                      过期自动删除
                    </Badge>
                  </div>

                  <div class="api-key-card__key-row mt-1.5 flex min-w-0 items-center gap-1.5">
                    <code class="min-w-0 truncate font-mono text-xs text-muted-foreground">
                      {{ apiKey.key_display || '****' }}
                    </code>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7 shrink-0 rounded-md"
                      title="复制完整密钥"
                      aria-label="复制完整密钥"
                      @click="copyKeyPrefix(apiKey)"
                    >
                      <Copy
                        aria-hidden="true"
                        class="h-3.5 w-3.5"
                      />
                    </Button>
                  </div>
                </div>
              </div>

              <dl class="api-key-card__stats mt-4 grid grid-cols-2 border-y border-border/50 text-xs">
                <div class="api-key-card__stat py-3 pr-3">
                  <dt class="text-muted-foreground">
                    请求次数
                  </dt>
                  <dd class="mt-1 text-sm font-semibold tabular-nums text-foreground">
                    {{ (apiKey.total_requests || 0).toLocaleString() }}
                  </dd>
                </div>
                <div class="api-key-card__stat border-l border-border/50 py-3 pl-3">
                  <dt class="text-muted-foreground">
                    Tokens
                  </dt>
                  <dd class="mt-1 text-sm font-semibold tabular-nums text-foreground">
                    {{ formatApiKeyTotalTokens(apiKey) }}
                  </dd>
                </div>
                <div class="api-key-card__stat border-t border-border/50 py-3 pr-3">
                  <dt class="text-muted-foreground">
                    有效期
                  </dt>
                  <dd class="mt-1 font-medium text-foreground">
                    {{ apiKey.expires_at ? formatDate(apiKey.expires_at) : '永不过期' }}
                  </dd>
                  <dd
                    v-if="apiKey.expires_at"
                    class="mt-0.5 text-[11px] text-muted-foreground"
                  >
                    {{ getRelativeTime(apiKey.expires_at) }}
                  </dd>
                </div>
                <div class="api-key-card__stat border-l border-t border-border/50 py-3 pl-3">
                  <dt class="text-muted-foreground">
                    限制
                  </dt>
                  <dd class="mt-1 flex flex-wrap gap-x-2 gap-y-0.5 font-medium text-foreground">
                    <span>{{ formatRateLimitInheritable(apiKey.rate_limit) }}</span>
                    <span>{{ formatConcurrentLimitInheritable(apiKey.concurrent_limit) }}</span>
                  </dd>
                </div>
              </dl>

              <dl class="api-key-card__dates grid gap-2 py-3 text-xs sm:grid-cols-2 sm:gap-x-6">
                <div class="flex min-w-0 items-baseline justify-between gap-3">
                  <dt class="shrink-0 text-muted-foreground">
                    创建
                  </dt>
                  <dd class="min-w-0 text-right font-medium tabular-nums text-foreground">
                    {{ formatDate(apiKey.created_at) }}
                  </dd>
                </div>
                <div class="flex min-w-0 items-baseline justify-between gap-3">
                  <dt class="shrink-0 text-muted-foreground">
                    最近使用
                  </dt>
                  <dd
                    v-if="apiKey.last_used_at"
                    class="min-w-0 text-right font-medium tabular-nums text-foreground"
                  >
                    {{ formatDate(apiKey.last_used_at) }}
                  </dd>
                  <dd
                    v-else
                    class="text-muted-foreground"
                  >
                    暂无记录
                  </dd>
                </div>
              </dl>

              <div class="api-key-card__actions grid grid-cols-3 gap-1 border-t border-border/50 pt-2">
                <Button
                  variant="ghost"
                  size="sm"
                  class="api-key-card__action-button h-9 text-xs"
                  @click="editApiKey(apiKey)"
                >
                  <SquarePen
                    aria-hidden="true"
                    class="mr-1.5 h-3.5 w-3.5"
                  />
                  编辑
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  class="api-key-card__action-button h-9 text-xs"
                  @click="toggleApiKey(apiKey)"
                >
                  <Power
                    aria-hidden="true"
                    class="mr-1.5 h-3.5 w-3.5"
                  />
                  {{ apiKey.is_active ? '禁用' : '启用' }}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  class="api-key-card__action-button h-9 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive"
                  @click="deleteApiKey(apiKey)"
                >
                  <Trash2
                    aria-hidden="true"
                    class="mr-1.5 h-3.5 w-3.5"
                  />
                  删除
                </Button>
              </div>
            </article>
          </div>
        </div>
      </div>
    </TableCard>

    <!-- 创建/编辑独立Key对话框 -->
    <StandaloneKeyFormDialog
      ref="keyFormDialogRef"
      :open="showKeyFormDialog"
      :api-key="editingKeyData"
      @close="closeKeyFormDialog"
      @submit="handleKeyFormSubmit"
    />

    <!-- 新 Key 显示对话框 -->
    <Dialog
      v-model="showNewKeyDialog"
      size="lg"
    >
      <template #header>
        <div class="border-b border-border px-6 py-4">
          <div class="flex items-center gap-3">
            <div class="flex h-9 w-9 items-center justify-center rounded-lg bg-emerald-100 dark:bg-emerald-900/30 flex-shrink-0">
              <CheckCircle class="h-5 w-5 text-emerald-600 dark:text-emerald-400" />
            </div>
            <div class="flex-1 min-w-0">
              <h3 class="text-lg font-semibold text-foreground leading-tight">
                创建成功
              </h3>
              <p class="text-xs text-muted-foreground">
                请妥善保管, 切勿泄露给他人.
              </p>
            </div>
          </div>
        </div>
      </template>

      <div class="space-y-4">
        <div class="space-y-2">
          <Label class="text-sm font-medium">API Key</Label>
          <div class="flex items-center gap-2">
            <Input
              ref="keyInput"
              type="text"
              :value="newKeyValue"
              readonly
              class="flex-1 font-mono text-sm bg-muted/50 h-11"
              @click="selectKey"
            />
            <Button
              class="h-11"
              @click="copyKey"
            >
              复制
            </Button>
          </div>
        </div>
      </div>

      <template #footer>
        <Button
          class="h-10 px-5"
          @click="closeNewKeyDialog"
        >
          确定
        </Button>
      </template>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useToast } from '@/composables/useToast'
import { useConfirm } from '@/composables/useConfirm'
import { useClipboard } from '@/composables/useClipboard'
import { adminApi, type AdminApiKey, type CreateStandaloneApiKeyRequest } from '@/api/admin'
import { EmptyState, LoadingState } from '@/components/common'

import {
  Dialog,
  TableCard,
  Button,
  Badge,
  Input,
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
  RefreshButton,
  Label
} from '@/components/ui'

import {
  Plus,
  Key,
  Trash2,
  Power,
  Copy,
  CheckCircle,
  SquarePen
} from 'lucide-vue-next'

import { StandaloneKeyFormDialog, type StandaloneKeyFormData } from '@/features/api-keys'
import { parseApiError } from '@/utils/errorParser'
import { formatTokens, formatRateLimitInheritable, isRateLimitInherited, isRateLimitUnlimited } from '@/utils/format'
import { log } from '@/utils/logger'

const { success, error } = useToast()
const { confirmDanger } = useConfirm()
const { copyToClipboard } = useClipboard()

const apiKeys = ref<AdminApiKey[]>([])
const loading = ref(false)
const showNewKeyDialog = ref(false)
const newKeyValue = ref('')
const keyInput = ref<HTMLInputElement>()

// 统一的表单对话框状态
const showKeyFormDialog = ref(false)
const editingKeyData = ref<StandaloneKeyFormData | null>(null)
const keyFormDialogRef = ref<InstanceType<typeof StandaloneKeyFormDialog>>()

onMounted(async () => {
  await refreshApiKeys()
})

async function refreshApiKeys() {
  loading.value = true
  try {
    const response = await adminApi.getAllApiKeys({
      limit: 500
    })
    const standaloneKeys = response.api_keys.filter((key) => key.is_standalone === true)
    if (standaloneKeys.length !== response.api_keys.length) {
      log.warn('独立 Key 页面收到了非 standalone 记录，已在前端过滤', {
        received: response.api_keys.length,
        kept: standaloneKeys.length
      })
    }
    apiKeys.value = standaloneKeys
  } catch (err: unknown) {
    log.error('加载独立Keys失败:', err)
    error(parseApiError(err, '加载独立 Keys 失败'))
  } finally {
    loading.value = false
  }
}

async function toggleApiKey(apiKey: AdminApiKey) {
  try {
    const response = await adminApi.toggleApiKey(apiKey.id)
    const index = apiKeys.value.findIndex(k => k.id === apiKey.id)
    if (index !== -1) {
      apiKeys.value[index].is_active = response.is_active
    }
    success(response.message)
  } catch (err: unknown) {
    log.error('切换密钥状态失败:', err)
    error(parseApiError(err, '操作失败'))
  }
}

async function deleteApiKey(apiKey: AdminApiKey) {
  const confirmed = await confirmDanger(
    `确定要删除这个独立余额 Key 吗？\n\n${apiKey.name || apiKey.key_display || '****'}\n\n此操作无法撤销。`,
    '删除独立 Key'
  )

  if (!confirmed) return

  try {
    const response = await adminApi.deleteApiKey(apiKey.id)
    apiKeys.value = apiKeys.value.filter(k => k.id !== apiKey.id)
    success(response.message)
  } catch (err: unknown) {
    log.error('删除密钥失败:', err)
    error(parseApiError(err, '删除失败'))
  }
}

function formatDateForInput(dateString: string): string | undefined {
  const date = new Date(dateString)
  if (Number.isNaN(date.getTime())) {
    return undefined
  }
  const year = date.getFullYear()
  const month = `${date.getMonth() + 1}`.padStart(2, '0')
  const day = `${date.getDate()}`.padStart(2, '0')
  return `${year}-${month}-${day}`
}

function parseDateInput(dateString: string): Date | null {
  const [year, month, day] = dateString.split('-').map(part => Number.parseInt(part, 10))
  if (!year || !month || !day) {
    return null
  }
  const date = new Date(year, month - 1, day)
  return Number.isNaN(date.getTime()) ? null : date
}

function serializeExpiryDate(dateString?: string): string | null {
  if (!dateString) {
    return null
  }
  const date = parseDateInput(dateString)
  if (!date) {
    return null
  }
  date.setHours(23, 59, 59, 999)
  return date.toISOString()
}

function editApiKey(apiKey: AdminApiKey) {
  // 解析过期日期为 YYYY-MM-DD 格式
  // 保留原始日期，不做时间过滤（避免编辑当天过期的 Key 时意外清空）
  let expiresAt: string | undefined = undefined

  if (apiKey.expires_at) {
    expiresAt = formatDateForInput(apiKey.expires_at)
  }

  editingKeyData.value = {
    id: apiKey.id,
    name: apiKey.name || '',
    unlimited_balance: isApiKeyUnlimited(apiKey),
    expires_at: expiresAt,
    rate_limit: apiKey.rate_limit ?? undefined,
    concurrent_limit: apiKey.concurrent_limit ?? undefined,
    auto_delete_on_expiry: apiKey.auto_delete_on_expiry || false,
    allowed_providers: apiKey.allowed_providers == null ? null : [...apiKey.allowed_providers],
    allowed_api_formats: apiKey.allowed_api_formats == null ? null : [...apiKey.allowed_api_formats],
    allowed_models: apiKey.allowed_models == null ? null : [...apiKey.allowed_models],
    feature_settings: apiKey.feature_settings ?? null
  }

  showKeyFormDialog.value = true
}

function isApiKeyUnlimited(apiKey: AdminApiKey): boolean {
  return apiKey.rate_limit === 0
}

function formatApiKeyTotalTokens(apiKey: AdminApiKey): string {
  if (apiKey.total_tokens == null) {
    return '未统计'
  }
  return formatTokens(apiKey.total_tokens)
}

function formatConcurrentLimitInheritable(concurrentLimit?: number | null): string {
  if (concurrentLimit == null) return '不限并发'
  if (concurrentLimit === 0) return '不限并发'
  return `${concurrentLimit} 并发`
}

function isConcurrentLimitInherited(concurrentLimit?: number | null): boolean {
  return concurrentLimit == null
}

function isConcurrentLimitUnlimited(concurrentLimit?: number | null): boolean {
  return concurrentLimit === 0
}

function selectKey() {
  keyInput.value?.select()
}

async function copyKey() {
  await copyToClipboard(newKeyValue.value)
}

async function copyKeyPrefix(apiKey: AdminApiKey) {
  try {
    // 调用后端 API 获取完整密钥
    const response = await adminApi.getFullApiKey(apiKey.id)
    const copied = await copyToClipboard(response.key, false)
    if (copied) {
      success('完整密钥已复制到剪贴板')
    } else {
      error('复制失败，请手动复制')
    }
  } catch (err) {
    log.error('复制密钥失败:', err)
    error('复制失败，请重试')
  }
}

function closeNewKeyDialog() {
  showNewKeyDialog.value = false
  newKeyValue.value = ''
}

function formatDate(dateString: string): string {
  return new Date(dateString).toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  })
}

function getRelativeTime(dateString: string): string {
  const date = new Date(dateString)
  const now = new Date()
  const diff = date.getTime() - now.getTime()

  if (diff < 0) return '已过期'

  const days = Math.floor(diff / (1000 * 60 * 60 * 24))
  const hours = Math.floor(diff / (1000 * 60 * 60))

  if (days > 0) return `${days}天后过期`
  if (hours > 0) return `${hours}小时后过期`
  return '即将过期'
}

// ========== 统一表单对话框方法 ==========

// 打开创建对话框
function openCreateDialog() {
  editingKeyData.value = null
  showKeyFormDialog.value = true
}

// 关闭表单对话框
function closeKeyFormDialog() {
  showKeyFormDialog.value = false
  editingKeyData.value = null
}

// 统一处理表单提交
async function handleKeyFormSubmit(data: StandaloneKeyFormData) {
  // 验证过期日期（如果设置了，必须晚于今天）
  if (data.expires_at) {
    const selectedDate = parseDateInput(data.expires_at)
    if (!selectedDate) {
      error('过期日期格式无效')
      return
    }
    selectedDate.setHours(0, 0, 0, 0)
    const today = new Date()
    today.setHours(0, 0, 0, 0)
    if (selectedDate <= today) {
      error('过期日期必须晚于今天')
      return
    }
  }

  keyFormDialogRef.value?.setSaving(true)
  try {
    if (data.id) {
      // 更新
      const updateData: Partial<CreateStandaloneApiKeyRequest> = {
        name: data.name || undefined,
        unlimited_balance: Boolean(data.unlimited_balance),
        rate_limit: data.rate_limit ?? null,  // undefined = 跟随系统默认，显式传 null
        concurrent_limit: data.concurrent_limit ?? null,
        expires_at: serializeExpiryDate(data.expires_at),
        auto_delete_on_expiry: data.auto_delete_on_expiry,
        // 空数组表示清除限制（允许全部），后端会将空数组存为 NULL
        allowed_providers: data.allowed_providers,
        allowed_api_formats: data.allowed_api_formats,
        allowed_models: data.allowed_models,
        ip_rules: data.ip_rules,
        feature_settings: data.feature_settings ?? null
      }
      const { message: _, ...updated } = await adminApi.updateApiKey(data.id, updateData)
      // 局部更新：合并字段，避免覆盖丢失列表已有信息
      const index = apiKeys.value.findIndex(k => k.id === data.id)
      if (index !== -1) {
        apiKeys.value[index] = {
          ...apiKeys.value[index],
          ...updated,
        }
      }
      success('API Key 更新成功')
    } else {
      // 创建
      const isUnlimited = Boolean(data.unlimited_balance)
      if (!isUnlimited && (!data.initial_balance_usd || data.initial_balance_usd <= 0)) {
        error('初始余额必须大于 0')
        return
      }
      const createData: CreateStandaloneApiKeyRequest = {
        name: data.name || undefined,
        initial_balance_usd: isUnlimited ? null : (data.initial_balance_usd as number),
        rate_limit: data.rate_limit ?? null,  // undefined = 跟随系统默认，显式传 null
        concurrent_limit: data.concurrent_limit ?? null,
        expires_at: serializeExpiryDate(data.expires_at),
        auto_delete_on_expiry: data.auto_delete_on_expiry,
        // 空数组表示不设置限制（允许全部），后端会将空数组存为 NULL
        allowed_providers: data.allowed_providers,
        allowed_api_formats: data.allowed_api_formats,
        allowed_models: data.allowed_models,
        ip_rules: data.ip_rules,
        feature_settings: data.feature_settings ?? null
      }
      const response = await adminApi.createStandaloneApiKey(createData)
      newKeyValue.value = response.key
      showNewKeyDialog.value = true
      success('独立 Key 创建成功')
      await refreshApiKeys()
    }
    closeKeyFormDialog()
  } catch (err: unknown) {
    log.error('保存独立Key失败:', err)
    error(parseApiError(err, '保存失败'))
  } finally {
    keyFormDialogRef.value?.setSaving(false)
  }
}
</script>

<style scoped>
/* Keep the card layout readable while reducing vertical chrome in the narrow
 * desktop content column. The named responsive-list container switches to the
 * table layout at 52rem, so this rule only affects the card view. */
@container list (max-width: 51.99rem) {
  .api-key-card {
    padding-block: 0.75rem;
  }

  .api-key-card__header {
    gap: 0.5rem;
  }

  .api-key-card__key-row {
    margin-top: 0.25rem;
    gap: 0.5rem;
  }

  .api-key-card__stats {
    margin-top: 0.75rem;
  }

  .api-key-card__stat {
    padding-block: 0.5rem;
  }

  .api-key-card__dates {
    gap: 0.25rem;
    padding-block: 0.5rem;
  }

  .api-key-card__actions {
    gap: 0.25rem;
    padding-top: 0.5rem;
  }

  .api-key-card__action-button {
    height: 2rem;
    min-height: 2rem;
  }
}
</style>
