<template>
  <Dialog
    :model-value="isOpen"
    :title="props.apiKey?.name ? `模型权限 - ${props.apiKey.name}` : '模型权限'"
    description="选中的模型将被允许访问，不选择则允许全部"
    :icon="Shield"
    size="2xl"
    @update:model-value="handleDialogUpdate"
  >
    <template #default>
      <div class="space-y-4">
        <!-- 搜索栏 -->
        <div class="flex items-center gap-2">
          <div class="flex-1 relative">
            <Search class="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
            <Input
              v-model="searchQuery"
              placeholder="搜索模型或添加自定义模型..."
              class="pl-8 h-9"
            />
          </div>
          <!-- 已选数量徽章 -->
          <span
            v-if="selectedModels.length === 0"
            class="h-7 px-2.5 text-xs rounded-md flex items-center bg-muted text-muted-foreground shrink-0"
          >
            全部模型
          </span>
          <span
            v-else
            class="h-7 px-2.5 text-xs rounded-md flex items-center bg-primary/10 text-primary shrink-0"
          >
            已选 {{ selectedModels.length }} 个
          </span>
          <!-- 刷新上游模型按钮 -->
          <button
            v-if="upstreamModelsLoaded"
            type="button"
            class="p-2 hover:bg-muted rounded-md transition-colors shrink-0"
            :disabled="fetchingUpstreamModels"
            title="刷新上游模型"
            @click="refreshUpstreamModels"
          >
            <RefreshCw
              class="w-4 h-4"
              :class="{ 'animate-spin': fetchingUpstreamModels }"
            />
          </button>
          <button
            v-else-if="!fetchingUpstreamModels"
            type="button"
            class="p-2 hover:bg-muted rounded-md transition-colors shrink-0"
            title="从提供商获取模型"
            @click="refreshUpstreamModels"
          >
            <Zap class="w-4 h-4" />
          </button>
          <Loader2
            v-else
            class="w-4 h-4 animate-spin text-muted-foreground shrink-0"
          />
        </div>

        <!-- 模型列表 -->
        <div class="border rounded-lg overflow-hidden">
          <div class="max-h-96 overflow-y-auto">
            <!-- 加载中 -->
            <div
              v-if="loadingProviderModels"
              class="flex items-center justify-center py-12"
            >
              <Loader2 class="w-6 h-6 animate-spin text-primary" />
            </div>

            <template v-else>
              <!-- 添加自定义模型（搜索内容不在列表中时显示） -->
              <div
                v-if="searchQuery && canAddAsCustom"
                class="px-3 py-2 border-b bg-background sticky top-0 z-30"
              >
                <div
                  class="flex items-center justify-between px-3 py-2 rounded-lg border border-dashed hover:border-primary hover:bg-primary/5 cursor-pointer transition-colors"
                  @click="addCustomModel"
                >
                  <div class="flex items-center gap-2">
                    <Plus class="w-4 h-4 text-muted-foreground" />
                    <span class="text-sm font-mono">{{ searchQuery }}</span>
                  </div>
                  <span class="text-xs text-muted-foreground">添加自定义模型</span>
                </div>
              </div>

              <!-- 自定义模型 -->
              <div v-if="customModels.length > 0">
                <div
                  class="flex items-center justify-between px-3 py-2 bg-muted sticky top-0 z-20 cursor-pointer hover:bg-muted/80 transition-colors"
                  @click="toggleGroupCollapse('custom')"
                >
                  <div class="flex items-center gap-2">
                    <ChevronDown
                      class="w-4 h-4 transition-transform shrink-0"
                      :class="collapsedGroups.has('custom') ? '-rotate-90' : ''"
                    />
                    <span class="text-xs font-medium">自定义模型</span>
                    <span class="text-xs text-muted-foreground">({{ customModels.length }})</span>
                  </div>
                </div>
                <div
                  v-show="!collapsedGroups.has('custom')"
                  class="space-y-1 p-2"
                >
                  <div
                    v-for="model in sortedCustomModels"
                    :key="model"
                    class="flex items-center gap-2 px-2 py-1.5 rounded hover:bg-muted cursor-pointer"
                    @click="toggleModel(model)"
                  >
                    <div
                      class="w-4 h-4 border rounded flex items-center justify-center shrink-0"
                      :class="selectedModels.includes(model) ? 'bg-primary border-primary' : ''"
                    >
                      <Check
                        v-if="selectedModels.includes(model)"
                        class="w-3 h-3 text-primary-foreground"
                      />
                    </div>
                    <span class="text-sm font-mono truncate flex-1">{{ model }}</span>
                  </div>
                </div>
              </div>

              <!-- 提供商模型 -->
              <template v-if="filteredProviderModels.length > 0">
                <!-- 标题 sticky top -->
                <div
                  class="flex items-center justify-between px-3 py-2 bg-muted sticky top-0 z-20 cursor-pointer hover:bg-muted/80 transition-colors"
                  @click="toggleGroupCollapse('global')"
                >
                  <div class="flex items-center gap-2">
                    <ChevronDown
                      class="w-4 h-4 transition-transform shrink-0"
                      :class="collapsedGroups.has('global') ? '-rotate-90' : ''"
                    />
                    <span class="text-xs font-medium">提供商模型</span>
                    <span class="text-xs text-muted-foreground">({{ filteredProviderModels.length }})</span>
                  </div>
                  <button
                    type="button"
                    class="text-xs text-primary hover:underline"
                    @click.stop="toggleAllProviderModels"
                  >
                    {{ isAllProviderModelsSelected ? '取消全选' : '全选' }}
                  </button>
                </div>
                <!-- 内容 -->
                <div
                  v-show="!collapsedGroups.has('global')"
                  class="space-y-1 p-2"
                >
                  <div
                    v-for="model in filteredProviderModels"
                    :key="model.name"
                    class="flex items-center gap-2 px-2 py-1.5 rounded hover:bg-muted cursor-pointer"
                    @click="toggleModel(model.name)"
                  >
                    <div
                      class="w-4 h-4 border rounded flex items-center justify-center shrink-0"
                      :class="selectedModels.includes(model.name) ? 'bg-primary border-primary' : ''"
                    >
                      <Check
                        v-if="selectedModels.includes(model.name)"
                        class="w-3 h-3 text-primary-foreground"
                      />
                    </div>
                    <div class="flex-1 min-w-0">
                      <p class="text-sm font-medium truncate">
                        {{ model.name }}
                      </p>
                      <p
                        v-if="model.global_model_display_name || model.global_model_name"
                        class="text-xs text-muted-foreground truncate font-mono"
                      >
                        {{ model.global_model_display_name || model.global_model_name }}
                      </p>
                    </div>
                  </div>
                </div>
              </template>

              <!-- 上游模型 -->
              <template v-if="filteredUpstreamModels.length > 0">
                <!-- 标题 sticky -->
                <div
                  class="flex items-center justify-between px-3 py-2 bg-muted sticky top-0 z-20 cursor-pointer hover:bg-muted/80 transition-colors"
                  @click="toggleGroupCollapse('upstream')"
                >
                  <div class="flex items-center gap-2">
                    <ChevronDown
                      class="w-4 h-4 transition-transform shrink-0"
                      :class="collapsedGroups.has('upstream') ? '-rotate-90' : ''"
                    />
                    <span class="text-xs font-medium">上游模型</span>
                    <span class="text-xs text-muted-foreground">({{ upstreamModelNames.length }})</span>
                  </div>
                  <button
                    type="button"
                    class="text-xs text-primary hover:underline"
                    @click.stop="toggleAllUpstreamModels"
                  >
                    {{ isAllUpstreamModelsSelected ? '取消全选' : '全选' }}
                  </button>
                </div>
                <!-- 内容 -->
                <div
                  v-show="!collapsedGroups.has('upstream')"
                  class="space-y-1 p-2"
                >
                  <div
                    v-for="model in filteredUpstreamModels"
                    :key="model.id"
                    class="flex items-center gap-2 px-2 py-1.5 rounded hover:bg-muted cursor-pointer"
                    @click="handleUpstreamModelClick(model)"
                  >
                    <div
                      class="w-4 h-4 border rounded flex items-center justify-center shrink-0"
                      :class="selectedModels.includes(model.id) ? 'bg-primary border-primary' : ''"
                    >
                      <Check
                        v-if="selectedModels.includes(model.id)"
                        class="w-3 h-3 text-primary-foreground"
                      />
                    </div>
                    <span class="text-sm font-mono truncate flex-1">{{ model.id }}</span>
                    <!-- API 格式标签 -->
                    <div class="flex items-center gap-1 shrink-0">
                      <span
                        v-for="fmt in model.api_formats"
                        :key="fmt"
                        class="text-[10px] px-1.5 py-0.5 rounded bg-muted text-muted-foreground"
                      >
                        {{ formatApiFormatShort(fmt) }}
                      </span>
                    </div>
                  </div>
                </div>
              </template>

              <!-- 空状态 -->
              <div
                v-if="showEmptyState"
                class="flex flex-col items-center justify-center py-12 text-muted-foreground"
              >
                <Shield class="w-10 h-10 mb-2 opacity-30" />
                <p class="text-sm">
                  {{ searchQuery ? '无匹配结果' : '暂无可选模型' }}
                </p>
              </div>
            </template>
          </div>
        </div>
      </div>
    </template>

    <template #footer>
      <div class="flex items-center justify-between w-full">
        <p class="text-xs text-muted-foreground">
          {{ hasChanges ? '有未保存的更改' : '' }}
        </p>
        <div class="flex items-center gap-2">
          <Button
            :disabled="saving || !hasChanges"
            @click="handleSave"
          >
            {{ saving ? '保存中...' : '保存' }}
          </Button>
          <Button
            variant="outline"
            @click="handleCancel"
          >
            取消
          </Button>
        </div>
      </div>
    </template>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue'
import {
  Shield,
  Search,
  Loader2,
  Plus,
  Check,
  ChevronDown,
  RefreshCw,
  Zap
} from 'lucide-vue-next'
import { Dialog, Button, Input } from '@/components/ui'
import { useToast } from '@/composables/useToast'
import { useConfirm } from '@/composables/useConfirm'
import { parseApiError } from '@/utils/errorParser'
import {
  getProviderModels,
  updateProviderKey,
  type EndpointAPIKey,
  type AllowedModels,
} from '@/api/endpoints'
import { useUpstreamModelsCache } from '../composables/useUpstreamModelsCache'
import { formatApiFormatShort, type UpstreamModel } from '@/api/endpoints/types'

interface AvailableModel {
  name: string
  display_name: string
  global_model_name?: string
  global_model_display_name?: string
}

const props = defineProps<{
  open: boolean
  apiKey: EndpointAPIKey | null
  providerId: string
}>()

const emit = defineEmits<{
  close: []
  saved: []
}>()

const { success, error: showError, warning: showWarning } = useToast()
const { confirmWarning } = useConfirm()
const { fetchModels: fetchCachedModels } = useUpstreamModelsCache()

const isOpen = computed(() => props.open)
const saving = ref(false)
const loadingProviderModels = ref(false)
const fetchingUpstreamModels = ref(false)
const upstreamModelsLoaded = ref(false)

// 用于取消异步操作的标志
let loadingCancelled = false

// 搜索
const searchQuery = ref('')

// 可用模型列表（该 Provider 已关联的模型）
const allProviderModels = ref<AvailableModel[]>([])
// 上游模型列表（从 API 查询获取）
const upstreamModels = ref<UpstreamModel[]>([])

// 已选中的模型
const selectedModels = ref<string[]>([])
const initialSelectedModels = ref<string[]>([])

// 所有添加过的自定义模型（包括已取消勾选的，保存前不消失）
const allCustomModels = ref<string[]>([])

// 空状态判断
const showEmptyState = computed(() => {
  return filteredProviderModels.value.length === 0 &&
         filteredUpstreamModels.value.length === 0 &&
         customModels.value.length === 0
})

// 折叠状态
const collapsedGroups = ref<Set<string>>(new Set())

// 是否有更改
const hasChanges = computed(() => {
  // 检查选中模型是否有变化
  if (selectedModels.value.length !== initialSelectedModels.value.length) return true
  const sorted1 = [...selectedModels.value].sort()
  const sorted2 = [...initialSelectedModels.value].sort()
  if (sorted1.some((v, i) => v !== sorted2[i])) return true

  return false
})

// 所有已知模型的集合（提供商模型 + 上游模型）
const allKnownModels = computed(() => {
  const set = new Set<string>()
  allProviderModels.value.forEach(m => set.add(m.name))
  upstreamModels.value.forEach(m => set.add(m.id))
  return set
})

// 提供商模型名称集合（用于判断模型是否为“提供商模型”分组项）
const providerModelNamesSet = computed(() => {
  return new Set(allProviderModels.value.map(m => m.name))
})

// 上游模型列表（后端已按 id 聚合，包含 api_formats 数组）
// 这里只做排序
const upstreamModelList = computed(() => {
  return [...upstreamModels.value].sort((a, b) => a.id.localeCompare(b.id))
})

// 上游模型名称列表（用于计数和全选判断）
const upstreamModelNames = computed(() => upstreamModelList.value.map(m => m.id))

// 过滤后的上游模型
const filteredUpstreamModels = computed(() => {
  if (!searchQuery.value.trim()) return upstreamModelList.value
  const query = searchQuery.value.toLowerCase()
  return upstreamModelList.value.filter(m => m.id.toLowerCase().includes(query))
})

// 上游模型是否全选
const isAllUpstreamModelsSelected = computed(() => {
  if (filteredUpstreamModels.value.length === 0) return false
  return filteredUpstreamModels.value.every(m => selectedModels.value.includes(m.id))
})

// 全选/取消全选上游模型
function toggleAllUpstreamModels() {
  const allIds = filteredUpstreamModels.value.map(m => m.id)
  if (isAllUpstreamModelsSelected.value) {
    selectedModels.value = selectedModels.value.filter(id => !allIds.includes(id))
  } else {
    allIds.forEach(id => {
      if (!selectedModels.value.includes(id)) {
        selectedModels.value.push(id)
      }
    })
  }
}

// 处理上游模型点击。
function handleUpstreamModelClick(model: UpstreamModel) {
  toggleModel(model.id)
}

// 自定义模型列表（显示所有添加过的，不因取消勾选而消失）
const customModels = computed(() => {
  return allCustomModels.value
})

// 排序后的自定义模型（搜索命中的排前面）
const sortedCustomModels = computed(() => {
  const search = searchQuery.value.toLowerCase().trim()
  if (!search) return customModels.value

  const matched: string[] = []
  const unmatched: string[] = []
  for (const m of customModels.value) {
    if (m.toLowerCase().includes(search)) {
      matched.push(m)
    } else {
      unmatched.push(m)
    }
  }
  return [...matched, ...unmatched]
})

// 判断搜索内容是否可以作为自定义模型添加
const canAddAsCustom = computed(() => {
  const search = searchQuery.value.trim()
  if (!search) return false
  // 已经选中了就不显示
  if (selectedModels.value.includes(search)) return false
  // 已经在自定义模型列表中就不显示
  if (allCustomModels.value.includes(search)) return false
  // 精确匹配提供商模型就不显示
  if (allProviderModels.value.some(m => m.name === search)) return false
  // 精确匹配上游模型就不显示
  if (upstreamModelNames.value.includes(search)) return false
  return true
})

// 搜索过滤后的提供商模型
const filteredProviderModels = computed(() => {
  if (!searchQuery.value.trim()) return allProviderModels.value
  const query = searchQuery.value.toLowerCase()
  return allProviderModels.value.filter(m =>
    m.name.toLowerCase().includes(query) ||
    m.display_name.toLowerCase().includes(query) ||
    (m.global_model_name || '').toLowerCase().includes(query) ||
    (m.global_model_display_name || '').toLowerCase().includes(query)
  )
})

// 提供商模型是否全选
const isAllProviderModelsSelected = computed(() => {
  if (filteredProviderModels.value.length === 0) return false
  return filteredProviderModels.value.every(m => selectedModels.value.includes(m.name))
})

// 切换模型选中状态
function toggleModel(modelId: string) {
  const idx = selectedModels.value.indexOf(modelId)
  if (idx === -1) {
    selectedModels.value.push(modelId)
  } else {
    selectedModels.value.splice(idx, 1)
  }
}

// 添加自定义模型
function addCustomModel() {
  const model = searchQuery.value.trim()
  if (model && !selectedModels.value.includes(model)) {
    selectedModels.value.push(model)
    // 同时添加到自定义模型列表（如果不在已知模型中）
    if (!allKnownModels.value.has(model) && !allCustomModels.value.includes(model)) {
      allCustomModels.value.push(model)
    }
    searchQuery.value = ''
  }
}

// 全选/取消全选提供商模型
function toggleAllProviderModels() {
  const allNames = filteredProviderModels.value.map(m => m.name)
  if (isAllProviderModelsSelected.value) {
    // 取消全选
    selectedModels.value = selectedModels.value.filter(id => !allNames.includes(id))
  } else {
    // 全选
    allNames.forEach(name => {
      if (!selectedModels.value.includes(name)) {
        selectedModels.value.push(name)
      }
    })
  }
}

// 切换折叠状态
function toggleGroupCollapse(group: string) {
  if (collapsedGroups.value.has(group)) {
    collapsedGroups.value.delete(group)
  } else {
    collapsedGroups.value.add(group)
  }
  collapsedGroups.value = new Set(collapsedGroups.value)
}

// 加载该 Provider 已关联的模型
async function loadProviderModels() {
  loadingProviderModels.value = true
  try {
    const response = await getProviderModels(props.providerId, { limit: 1000 })
    if (loadingCancelled) return
    allProviderModels.value = response.map(m => ({
      name: m.provider_model_name,
      display_name: m.global_model_display_name || m.global_model_name || m.provider_model_name,
      global_model_name: m.global_model_name,
      global_model_display_name: m.global_model_display_name
    }))
  } catch {
    if (loadingCancelled) return
    showError('加载提供商模型失败', '错误')
  } finally {
    loadingProviderModels.value = false
  }
}

// 从提供商获取模型（使用缓存）
// 传 apiKeyId，获取当前 Key 自己能访问的上游模型
async function fetchUpstreamModels(forceRefresh = false) {
  if (!props.providerId || !props.apiKey) return
  try {
    fetchingUpstreamModels.value = true
    const result = await fetchCachedModels(props.providerId, props.apiKey.id, forceRefresh)
    if (loadingCancelled) return
    if (result.models.length > 0) {
      upstreamModels.value = result.models
      upstreamModelsLoaded.value = true
      // 获取上游模型后，从自定义模型列表中移除已变成已知的模型
      const upstreamIds = new Set(result.models.map((m: UpstreamModel) => m.id))
      allCustomModels.value = allCustomModels.value.filter(m => !upstreamIds.has(m))
      if (result.warning) {
        showWarning(result.warning, '部分格式获取失败')
      }
    } else if (result.error) {
      showError(result.error, '获取上游模型失败')
    }
  } finally {
    fetchingUpstreamModels.value = false
  }
}

// 手动刷新上游模型（强制跳过缓存）
async function refreshUpstreamModels() {
  await fetchUpstreamModels(true)
  if (upstreamModels.value.length > 0) {
    success('上游模型已刷新')
    // 获取成功后收缩所有分组
    collapsedGroups.value = new Set(['global', 'upstream', 'custom'])
  }
}

// 解析 allowed_models
function parseAllowedModels(allowed: AllowedModels): string[] {
  if (allowed === null || allowed === undefined) {
    return []
  }
  return [...allowed]
}

async function initializeDialogState(apiKey: EndpointAPIKey) {
  loadingCancelled = false

  const parsed = parseAllowedModels(apiKey.allowed_models ?? null)
  selectedModels.value = [...parsed]
  initialSelectedModels.value = [...parsed]

  searchQuery.value = ''
  upstreamModels.value = []
  upstreamModelsLoaded.value = false
  allCustomModels.value = []

  collapsedGroups.value = new Set()

  await loadProviderModels()

  const upstreamModelIdsSet = new Set(upstreamModels.value.map(m => m.id))
  allCustomModels.value = selectedModels.value.filter(m =>
    !providerModelNamesSet.value.has(m) && !upstreamModelIdsSet.has(m)
  )
}

// 监听对话框打开 / 当前 key 变化
watch([() => props.open, () => props.apiKey], async ([open, apiKey]) => {
  if (open && apiKey) {
    await initializeDialogState(apiKey)
  } else {
    loadingCancelled = true
  }
}, { immediate: true })

// 组件卸载时取消所有异步操作
onUnmounted(() => {
  loadingCancelled = true
})

async function handleDialogUpdate(value: boolean) {
  if (!value && hasChanges.value) {
    const confirmed = await confirmWarning('有未保存的更改，确定要关闭吗？', '放弃更改')
    if (!confirmed) return
  }
  if (!value) emit('close')
}

async function handleCancel() {
  if (hasChanges.value) {
    const confirmed = await confirmWarning('有未保存的更改，确定要关闭吗？', '放弃更改')
    if (!confirmed) return
  }
  emit('close')
}

async function handleSave() {
  if (!props.apiKey) return

  saving.value = true
  try {
    const newAllowed: AllowedModels = selectedModels.value.length > 0
      ? [...selectedModels.value]
      : null

    await updateProviderKey(props.apiKey.id, {
      allowed_models: newAllowed
    })
    success('模型权限已更新', '成功')
    emit('saved')
    emit('close')
  } catch (err: unknown) {
    showError(parseApiError(err, '保存失败'), '错误')
  } finally {
    saving.value = false
  }
}
</script>
