<template>
  <section class="settings-group">
    <h3 class="settings-heading">
      默认出站代理
    </h3>
    <div>
      <!-- 区块一：默认代理 -->
      <div class="settings-row">
        <div>
          <Label for="default-proxy">默认代理</Label>
          <p class="settings-description">
            用于未单独配置代理的提供商请求，包括模型调用、余额查询和 OAuth。
          </p>
        </div>
        <div class="settings-row-control">
          <Select
            :model-value="proxyNodeId || '__direct__'"
            @update:model-value="(v: string) => $emit('update:proxyNodeId', v === '__direct__' ? null : v)"
          >
            <SelectTrigger id="default-proxy">
              <SelectValue placeholder="直连（不使用代理）" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="__direct__">
                直连（不使用代理）
              </SelectItem>
              <SelectItem
                v-for="node in selectableNodes"
                :key="node.id"
                :value="node.id"
              >
                {{ node.name }}{{ node.region ? ` · ${node.region}` : '' }} ({{ node.ip }}:{{ node.port }})
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      <SettingsSaveActions
        :loading="loading"
        :has-changes="hasChanges"
        :error="error"
        label="保存默认代理"
        @save="$emit('save')"
        @cancel="$emit('cancel')"
      />

      <!-- 区块二：代理节点管理 -->
      <div class="settings-group mt-7">
        <div class="settings-toolbar">
          <div class="flex items-center gap-2">
            <Label class="text-sm font-medium">代理节点</Label>
            <span
              v-if="nodes.length"
              class="text-xs text-muted-foreground"
            >({{ nodes.length }})</span>
          </div>
          <div class="flex items-center gap-2">
            <Button
              variant="ghost"
              size="icon"
              class="settings-icon-button"
              title="刷新节点"
              aria-label="刷新节点"
              :disabled="store.loading"
              @click="store.fetchNodes()"
            >
              <RefreshCw
                class="h-4 w-4"
                :class="{ 'animate-spin': store.loading }"
              />
            </Button>
            <Button
              variant="outline"
              size="sm"
              @click="openAddDialog"
            >
              <Plus class="mr-1.5 h-4 w-4" />
              添加节点
            </Button>
          </div>
        </div>

        <p
          v-if="store.loading"
          role="status"
          class="text-sm text-muted-foreground"
        >
          正在加载代理节点...
        </p>
        <div
          v-else-if="store.error"
          role="alert"
          class="space-y-2 text-sm text-destructive"
        >
          <p>{{ store.error }}</p>
          <Button
            variant="outline"
            size="sm"
            @click="store.fetchNodes()"
          >
            重试
          </Button>
        </div>
        <div
          v-else-if="nodes.length"
          class="space-y-2"
        >
          <div
            v-for="node in nodes"
            :key="node.id"
            class="settings-proxy-row"
            data-proxy-node
          >
            <div class="min-w-0 flex-1">
              <div class="flex flex-wrap items-center gap-2">
                <samp class="settings-node-name text-sm font-medium font-sans text-foreground">{{ node.name }}</samp>
                <span
                  v-if="node.region"
                  class="text-xs text-muted-foreground [overflow-wrap:anywhere]"
                >{{ node.region }}</span>
                <Badge
                  :variant="node.status === 'online' ? 'success' : 'secondary'"
                  class="shrink-0"
                >
                  {{ node.status === 'online' ? '在线' : '离线' }}
                </Badge>
              </div>
              <p class="settings-node-address">
                {{ nodeAddress(node) }}
              </p>
            </div>
            <div class="flex shrink-0 items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                :disabled="isNodeTesting(node.id)"
                @click="handleTestNode(node.id)"
              >
                {{ isNodeTesting(node.id) ? '测试中...' : '测试' }}
              </Button>
              <Button
                variant="ghost"
                size="icon"
                class="settings-icon-button"
                title="编辑"
                aria-label="编辑"
                @click="openEditDialog(node)"
              >
                <Pencil class="h-4 w-4" />
              </Button>
            </div>
          </div>
        </div>

        <div
          v-else
          class="settings-empty"
        >
          <p class="text-sm text-muted-foreground">
            暂无代理节点
          </p>
          <Button
            variant="outline"
            size="sm"
            class="mt-3"
            @click="openAddDialog"
          >
            添加节点
          </Button>
        </div>
      </div>

      <ProxyNodeEditDialog
        v-model:open="dialogOpen"
        :node="editingNode"
        @deleted="handleNodeDeleted"
      />
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import Button from '@/components/ui/button.vue'
import Badge from '@/components/ui/badge.vue'
import Label from '@/components/ui/label.vue'
import Select from '@/components/ui/select.vue'
import SelectTrigger from '@/components/ui/select-trigger.vue'
import SelectValue from '@/components/ui/select-value.vue'
import SelectContent from '@/components/ui/select-content.vue'
import SelectItem from '@/components/ui/select-item.vue'
import { Pencil, Plus, RefreshCw } from 'lucide-vue-next'
import SettingsSaveActions from './SettingsSaveActions.vue'
import { useProxyNodesStore } from '@/stores/proxy-nodes'
import { clearModelsDevCache } from '@/api/models-dev'
import { useToast } from '@/composables/useToast'
import { parseApiError } from '@/utils/errorParser'
import ProxyNodeEditDialog, { type ProxyNodeDeletedPayload } from './ProxyNodeEditDialog.vue'
import { proxyNodesApi, type ProxyNode } from '@/api/proxy-nodes'
import { formatProxyTestSuccessText } from './proxyTest'

const props = defineProps<{
  proxyNodeId: string | null
  loading: boolean
  hasChanges: boolean
  error?: string
}>()

const emit = defineEmits<{
  save: []
  cancel: []
  proxyCleared: []
  'update:proxyNodeId': [value: string | null]
}>()

const store = useProxyNodesStore()
const { success, error: toastError } = useToast()

const nodes = computed(() => store.nodes)

const dialogOpen = ref(false)
const editingNode = ref<ProxyNode | null>(null)
const testingNodeIds = ref(new Set<string>())

const selectableNodes = computed(() => {
  if (!props.proxyNodeId) {
    return onlineNodes()
  }
  const exists = onlineNodes().some(node => node.id === props.proxyNodeId)
  if (exists) {
    return onlineNodes()
  }
  const selected = nodes.value.find(node => node.id === props.proxyNodeId)
  return selected ? [selected, ...onlineNodes()] : onlineNodes()
})

function onlineNodes() {
  return nodes.value.filter(node =>
    node.status === 'online'
    && node.remote_config?.scheduling_state !== 'draining'
    && node.remote_config?.scheduling_state !== 'cordoned'
  )
}

onMounted(() => {
  void store.fetchNodes()
})

function nodeAddress(node: ProxyNode) {
  return node.tunnel_mode ? node.ip : `${node.ip}:${node.port}`
}

function openAddDialog() {
  editingNode.value = null
  dialogOpen.value = true
}

function openEditDialog(node: ProxyNode) {
  editingNode.value = node
  dialogOpen.value = true
}

function isNodeTesting(nodeId: string) {
  return testingNodeIds.value.has(nodeId)
}

async function handleTestNode(nodeId: string) {
  if (isNodeTesting(nodeId)) return
  testingNodeIds.value.add(nodeId)
  try {
    const result = await proxyNodesApi.testProxyNode(nodeId)
    if (result.success) {
      success(formatProxyTestSuccessText(result))
    } else {
      toastError(`测试失败: ${result.error || '未知错误'}`)
    }
  } catch (err: unknown) {
    toastError(parseApiError(err, '测试请求失败'))
  } finally {
    testingNodeIds.value.delete(nodeId)
  }
}

// 删除节点的副作用留在父级处理，弹窗只上报结果
function handleNodeDeleted(payload: ProxyNodeDeletedPayload) {
  if (payload.clearedExternalModelsProxy) {
    clearModelsDevCache()
  }
  if (payload.clearedSystemProxy) emit('proxyCleared')
  if (props.proxyNodeId === payload.nodeId || (payload.clearedSystemProxy && !props.proxyNodeId)) {
    emit('update:proxyNodeId', null)
  }
}
</script>
