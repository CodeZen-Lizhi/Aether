<template>
  <CardSection
    title="网络代理"
    description="配置提供商出站请求的代理，仅影响大模型 API、余额查询、OAuth 等提供商请求"
  >
    <template #actions>
      <Button
        size="sm"
        :disabled="loading || !hasChanges"
        @click="$emit('save')"
      >
        {{ loading ? '保存中...' : '保存' }}
      </Button>
    </template>

    <div class="max-w-md space-y-6">
      <!-- 代理节点：单节点手动录入 -->
      <div class="space-y-3">
        <div class="flex items-center justify-between gap-2">
          <div class="flex items-center gap-2">
            <Label class="text-sm font-medium">代理节点</Label>
            <Badge
              v-if="managedNode"
              :variant="managedNode.status === 'online' ? 'success' : 'secondary'"
            >
              {{ managedNode.status === 'online' ? '在线' : '离线' }}
            </Badge>
          </div>
          <Select
            v-if="nodes.length > 1"
            :model-value="managedNodeId ?? ''"
            @update:model-value="handleSelectManagedNode"
          >
            <SelectTrigger class="h-8 w-44 text-xs">
              <SelectValue placeholder="选择要管理的节点" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="node in nodes"
                :key="node.id"
                :value="node.id"
              >
                {{ node.name }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div class="space-y-1.5">
          <Label>名称 *</Label>
          <Input
            v-model="form.name"
            placeholder="例如: 美西 VPN 代理"
          />
        </div>
        <div class="space-y-1.5">
          <Label>代理地址 *</Label>
          <Input
            v-model="form.proxyUrl"
            placeholder="http://proxy:port 或 socks5://proxy:port"
          />
        </div>
        <div class="grid grid-cols-2 gap-3">
          <div class="space-y-1.5">
            <Label>用户名</Label>
            <Input
              v-model="form.username"
              placeholder="可选"
              autocomplete="off"
              data-form-type="other"
              data-lpignore="true"
              data-1p-ignore="true"
            />
          </div>
          <div class="space-y-1.5">
            <Label>密码</Label>
            <Input
              v-model="form.password"
              type="text"
              masked
              placeholder="可选"
              autocomplete="new-password"
              data-form-type="other"
              data-lpignore="true"
              data-1p-ignore="true"
            />
          </div>
        </div>
        <div class="space-y-1.5">
          <Label>区域</Label>
          <Input
            v-model="form.region"
            placeholder="可选，例如: US-West"
          />
        </div>

        <div class="flex items-center gap-2 pt-1">
          <Button
            variant="outline"
            size="sm"
            :disabled="!form.proxyUrl.trim() || testingUrl"
            @click="handleTestUrl"
          >
            {{ testingUrl ? '测试中...' : '测试连接' }}
          </Button>
          <template v-if="managedNodeId">
            <Button
              size="sm"
              :disabled="savingNode || !form.name.trim() || !form.proxyUrl.trim()"
              @click="handleSaveNode"
            >
              {{ savingNode ? '保存中...' : '保存节点' }}
            </Button>
            <Button
              variant="outline"
              size="sm"
              class="text-destructive hover:text-destructive"
              :disabled="deletingNode"
              @click="handleDeleteNode"
            >
              {{ deletingNode ? '删除中...' : '删除' }}
            </Button>
          </template>
          <Button
            v-else
            size="sm"
            :disabled="savingNode || !form.name.trim() || !form.proxyUrl.trim()"
            @click="handleCreateNode"
          >
            {{ savingNode ? '添加中...' : '添加节点' }}
          </Button>
        </div>
        <p class="text-xs text-muted-foreground">
          {{ managedNodeId ? '编辑时密码留空表示保持原密码不变。' : '支持 HTTP/SOCKS5 代理，用户名、密码、区域均可选。' }}
        </p>
      </div>

      <!-- 默认代理 -->
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
    </div>
  </CardSection>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import Button from '@/components/ui/button.vue'
import Badge from '@/components/ui/badge.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import Select from '@/components/ui/select.vue'
import SelectTrigger from '@/components/ui/select-trigger.vue'
import SelectValue from '@/components/ui/select-value.vue'
import SelectContent from '@/components/ui/select-content.vue'
import SelectItem from '@/components/ui/select-item.vue'
import { CardSection } from '@/components/layout'
import { useToast } from '@/composables/useToast'
import { useConfirm } from '@/composables/useConfirm'
import { useProxyNodesStore } from '@/stores/proxy-nodes'
import { proxyNodesApi, type ProxyNode } from '@/api/proxy-nodes'
import { clearModelsDevCache } from '@/api/models-dev'
import { parseApiError } from '@/utils/errorParser'

const props = defineProps<{
  proxyNodeId: string | null
  loading: boolean
  hasChanges: boolean
}>()

const emit = defineEmits<{
  save: []
  'update:proxyNodeId': [value: string | null]
}>()

const { success, error: toastError } = useToast()
const { confirmDanger } = useConfirm()
const store = useProxyNodesStore()

const nodes = computed(() => store.nodes)

const managedNodeId = ref<string | null>(null)
const form = ref({ name: '', proxyUrl: '', username: '', password: '', region: '' })
const savingNode = ref(false)
const deletingNode = ref(false)
const testingUrl = ref(false)

const managedNode = computed(() =>
  nodes.value.find(node => node.id === managedNodeId.value) ?? null
)

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

// 节点列表变化时，保证始终有一个被管理的节点（列表为空则回到“添加”模式）
watch(nodes, (list) => {
  if (!list.length) {
    managedNodeId.value = null
    return
  }
  if (!managedNodeId.value || !list.some(node => node.id === managedNodeId.value)) {
    void applyManagedNode(list[0].id)
  }
}, { immediate: true })

async function applyManagedNode(nodeId: string) {
  managedNodeId.value = nodeId
  try {
    const { node } = await proxyNodesApi.getNode(nodeId)
    if (managedNodeId.value !== nodeId) return
    form.value = {
      name: node.name,
      proxyUrl: node.proxy_url || '',
      username: node.proxy_username || '',
      password: node.proxy_password || '',
      region: node.region || '',
    }
  } catch (err: unknown) {
    toastError(parseApiError(err, '读取代理节点详情失败'))
  }
}

function handleSelectManagedNode(nodeId: string) {
  if (nodeId && nodeId !== managedNodeId.value) {
    void applyManagedNode(nodeId)
  }
}

function toRequestPayload() {
  return {
    name: form.value.name.trim(),
    proxy_url: form.value.proxyUrl.trim(),
    username: form.value.username.trim() || undefined,
    password: form.value.password || undefined,
    region: form.value.region.trim() || undefined,
  }
}

async function handleCreateNode() {
  if (savingNode.value) return
  savingNode.value = true
  try {
    const result = await store.createManualNode(toRequestPayload())
    await applyManagedNode(result.node_id)
    success('代理节点已添加')
  } catch (err: unknown) {
    toastError(parseApiError(err, '添加失败'))
  } finally {
    savingNode.value = false
  }
}

async function handleSaveNode() {
  if (!managedNodeId.value || savingNode.value) return
  savingNode.value = true
  try {
    await proxyNodesApi.updateManualNode(managedNodeId.value, toRequestPayload())
    await store.fetchNodes()
    success('代理节点已更新')
  } catch (err: unknown) {
    toastError(parseApiError(err, '更新失败'))
  } finally {
    savingNode.value = false
  }
}

async function handleDeleteNode() {
  const node = managedNode.value
  if (!node || deletingNode.value) return
  const address = node.tunnel_mode ? node.ip : `${node.ip}:${node.port}`
  const confirmed = await confirmDanger(
    `确定要删除代理节点 "${node.name}" (${address}) 吗？`,
    '删除节点'
  )
  if (!confirmed) return

  deletingNode.value = true
  try {
    const result = await proxyNodesApi.deleteProxyNode(node.id)
    if (result.cleared_external_models_proxy) {
      clearModelsDevCache()
    }
    if (result.cleared_system_proxy || props.proxyNodeId === node.id) {
      emit('update:proxyNodeId', null)
    }
    form.value = { name: '', proxyUrl: '', username: '', password: '', region: '' }
    managedNodeId.value = null
    await store.fetchNodes()
    success('代理节点已删除')
  } catch (err: unknown) {
    toastError(parseApiError(err, '删除失败'))
  } finally {
    deletingNode.value = false
  }
}

function formatTestResultMessage(result: { latency_ms: number | null; exit_ip: string | null }): string {
  const parts: string[] = []
  if (result.latency_ms != null) parts.push(`延迟 ${result.latency_ms}ms`)
  if (result.exit_ip) parts.push(`出口 IP ${result.exit_ip}`)
  return parts.length ? `，${parts.join('，')}` : ''
}

async function handleTestUrl() {
  if (!form.value.proxyUrl.trim() || testingUrl.value) return
  testingUrl.value = true
  try {
    const result = await proxyNodesApi.testProxyUrl({
      proxy_url: form.value.proxyUrl.trim(),
      username: form.value.username.trim() || undefined,
      password: form.value.password || undefined,
    })
    if (result.success) {
      success(`连通性测试通过${formatTestResultMessage(result)}`)
    } else {
      toastError(`连通性测试失败: ${result.error || '未知错误'}`)
    }
  } catch (err: unknown) {
    toastError(parseApiError(err, '测试请求失败'))
  } finally {
    testingUrl.value = false
  }
}
</script>
