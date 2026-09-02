<template>
  <CardSection
    title="网络代理"
    description="配置提供商出站请求的代理，仅影响大模型 API、余额查询、OAuth 等提供商请求"
    :collapsible="collapsible"
    :default-open="defaultOpen"
  >
    <template #actions>
      <Button
        size="sm"
        :disabled="loading || !hasChanges"
        :title="hasChanges ? undefined : '暂无改动'"
        @click="$emit('save')"
      >
        {{ loading ? '保存中...' : '保存默认代理' }}
      </Button>
    </template>

    <div class="max-w-md space-y-5">
      <!-- 区块一：默认代理 -->
      <div class="space-y-1.5">
        <Label class="block text-sm font-medium">默认代理</Label>
        <Select
          :model-value="proxyNodeId || '__direct__'"
          @update:model-value="(v: string) => $emit('update:proxyNodeId', v === '__direct__' ? null : v)"
        >
          <SelectTrigger>
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
        <p class="text-xs text-muted-foreground">
          对未单独配置代理的提供商生效，覆盖大模型 API 请求、余额查询、OAuth 刷新等。不影响系统内部接口。
        </p>
      </div>

      <div class="border-t border-border" />

      <!-- 区块二：代理节点管理 -->
      <div class="space-y-3">
        <div class="flex items-center justify-between gap-2">
          <div class="flex items-center gap-2">
            <Label class="text-sm font-medium">代理节点</Label>
            <span
              v-if="nodes.length"
              class="text-xs text-muted-foreground"
            >({{ nodes.length }})</span>
          </div>
          <Button
            variant="outline"
            size="sm"
            @click="openAddDialog"
          >
            添加节点
          </Button>
        </div>

        <div
          v-if="nodes.length"
          class="space-y-2"
        >
          <div
            v-for="node in nodes"
            :key="node.id"
            class="flex items-center justify-between gap-3 rounded-md border border-border/60 px-3 py-2"
          >
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <span class="truncate text-sm font-medium text-foreground">{{ node.name }}</span>
                <span
                  v-if="node.region"
                  class="truncate text-xs text-muted-foreground"
                >{{ node.region }}</span>
                <Badge
                  :variant="node.status === 'online' ? 'success' : 'secondary'"
                  class="shrink-0"
                >
                  {{ node.status === 'online' ? '在线' : '离线' }}
                </Badge>
              </div>
              <p class="mt-0.5 truncate text-xs text-muted-foreground">
                {{ nodeAddress(node) }}
              </p>
            </div>
            <Button
              variant="outline"
              size="sm"
              class="shrink-0"
              @click="openEditDialog(node)"
            >
              编辑
            </Button>
          </div>
        </div>

        <div
          v-else
          class="rounded-md border border-dashed border-border/60 px-4 py-6 text-center"
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
  </CardSection>
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
import { CardSection } from '@/components/layout'
import { useProxyNodesStore } from '@/stores/proxy-nodes'
import { clearModelsDevCache } from '@/api/models-dev'
import ProxyNodeEditDialog, { type ProxyNodeDeletedPayload } from './ProxyNodeEditDialog.vue'
import type { ProxyNode } from '@/api/proxy-nodes'

const props = defineProps<{
  proxyNodeId: string | null
  loading: boolean
  hasChanges: boolean
  collapsible?: boolean
  defaultOpen?: boolean
}>()

const emit = defineEmits<{
  save: []
  'update:proxyNodeId': [value: string | null]
}>()

const store = useProxyNodesStore()

const nodes = computed(() => store.nodes)

const dialogOpen = ref(false)
const editingNode = ref<ProxyNode | null>(null)

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
  void store.ensureLoaded()
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

// 删除节点的副作用留在父级处理，弹窗只上报结果
function handleNodeDeleted(payload: ProxyNodeDeletedPayload) {
  if (payload.clearedExternalModelsProxy) {
    clearModelsDevCache()
  }
  if (payload.clearedSystemProxy || props.proxyNodeId === payload.nodeId) {
    emit('update:proxyNodeId', null)
  }
}
</script>
