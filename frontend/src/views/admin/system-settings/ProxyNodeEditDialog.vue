<script lang="ts">
export interface ProxyNodeDeletedPayload {
  nodeId: string
  clearedSystemProxy: boolean
  clearedExternalModelsProxy: boolean
}
</script>

<template>
  <Dialog
    :open="open"
    :title="dialogTitle"
    @update:open="$emit('update:open', $event)"
  >
    <div class="space-y-4">
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
        <p
          v-if="proxyUrlError"
          class="text-xs text-destructive"
        >
          {{ proxyUrlError }}
        </p>
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
            :placeholder="isEdit ? '留空保持不变' : '可选'"
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

      <div
        v-if="testResult"
        class="rounded-md border px-3 py-2 text-xs"
        :class="testResult.kind === 'success'
          ? 'border-[#bfdbfe] bg-[#f5f9ff] text-[#1d4ed8] dark:border-[#1e40af]/40 dark:bg-[#1e3a8a]/25 dark:text-[#93c5fd]'
          : 'border-destructive/30 bg-destructive/10 text-destructive'"
      >
        {{ testResult.text }}
      </div>
    </div>

    <template #footer>
      <div class="flex w-full flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <Button
          v-if="isEdit"
          variant="outline"
          class="w-full text-destructive hover:text-destructive sm:w-auto"
          :disabled="deletingNode"
          @click="handleDeleteNode"
        >
          {{ deletingNode ? '删除中...' : '删除' }}
        </Button>
        <div class="flex w-full flex-col gap-2 sm:ml-auto sm:w-auto sm:flex-row sm:gap-3">
          <Button
            variant="outline"
            class="w-full sm:w-auto"
            :disabled="testingUrl || !proxyUrlValid"
            @click="handleTestUrl"
          >
            {{ testingUrl ? '测试中...' : '测试连接' }}
          </Button>
          <Button
            class="w-full sm:w-auto"
            :disabled="savingNode || !canSave"
            @click="handleSaveNode"
          >
            {{ savingNode ? '保存中...' : '保存' }}
          </Button>
        </div>
      </div>
    </template>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import Button from '@/components/ui/button.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import { Dialog } from '@/components/ui'
import { useToast } from '@/composables/useToast'
import { useConfirm } from '@/composables/useConfirm'
import { useProxyNodesStore } from '@/stores/proxy-nodes'
import { proxyNodesApi, type ProxyNode } from '@/api/proxy-nodes'
import { parseApiError } from '@/utils/errorParser'

const props = defineProps<{
  open: boolean
  node: ProxyNode | null // null = 添加模式
}>()

const emit = defineEmits<{
  'update:open': [value: boolean]
  saved: [] // 创建/更新成功（store 已刷新列表）
  deleted: [payload: ProxyNodeDeletedPayload]
}>()

// 与后端口径一致：scheme 前缀校验（http/https/socks5/socks5h），不做更深的 URL 语法校验
const PROXY_URL_PREFIX_PATTERN = /^(https?|socks5h?):\/\//i

const { success, error: toastError } = useToast()
const { confirmDanger } = useConfirm()
const store = useProxyNodesStore()

const isEdit = computed(() => props.node !== null)

function emptyForm() {
  return { name: '', proxyUrl: '', username: '', password: '', region: '' }
}

const form = ref(emptyForm())
const savingNode = ref(false)
const deletingNode = ref(false)
const testingUrl = ref(false)
const testResult = ref<{ kind: 'success' | 'error'; text: string } | null>(null)

const proxyUrlValid = computed(() => PROXY_URL_PREFIX_PATTERN.test(form.value.proxyUrl.trim()))
const proxyUrlError = computed(() =>
  form.value.proxyUrl.trim() && !proxyUrlValid.value
    ? '代理地址必须以 http://、https:// 或 socks5:// 开头'
    : ''
)
const canSave = computed(() => !!form.value.name.trim() && proxyUrlValid.value)
const dialogTitle = computed(() => (isEdit.value ? '编辑节点' : '添加节点'))

// 打开时重置表单，避免上一次编辑残留；编辑模式回填详情接口的明文密码（与现状一致）
watch(() => props.open, (open) => {
  if (open) {
    void initializeForm()
  }
}, { immediate: true })

// 测试结果针对当时填写的地址/凭据，字段变化后旧结果即失效（重开弹窗时由 initializeForm 重置）
watch(
  () => [form.value.proxyUrl, form.value.username, form.value.password],
  () => {
    testResult.value = null
  }
)

async function initializeForm() {
  form.value = emptyForm()
  testResult.value = null
  const node = props.node
  if (!node) return
  try {
    const { node: detail } = await proxyNodesApi.getNode(node.id)
    if (!props.open || props.node?.id !== node.id) return
    form.value = {
      name: detail.name,
      proxyUrl: detail.proxy_url || '',
      username: detail.proxy_username || '',
      password: detail.proxy_password || '',
      region: detail.region || '',
    }
  } catch (err: unknown) {
    toastError(parseApiError(err, '读取代理节点详情失败'))
  }
}

function closeDialog() {
  emit('update:open', false)
}

function toRequestPayload() {
  return {
    name: form.value.name.trim(),
    proxy_url: form.value.proxyUrl.trim(),
    username: form.value.username.trim() || undefined,
    // 编辑时密码留空表示保持原密码不变（后端空=不改）
    password: form.value.password || undefined,
    region: form.value.region.trim() || undefined,
  }
}

async function handleSaveNode() {
  if (savingNode.value || !canSave.value) return
  savingNode.value = true
  try {
    if (isEdit.value && props.node) {
      await proxyNodesApi.updateManualNode(props.node.id, toRequestPayload())
      await store.fetchNodes()
      success('代理节点已更新')
    } else {
      await store.createManualNode(toRequestPayload())
      success('代理节点已添加')
    }
    emit('saved')
    closeDialog()
  } catch (err: unknown) {
    toastError(parseApiError(err, isEdit.value ? '更新失败' : '添加失败'))
  } finally {
    savingNode.value = false
  }
}

async function handleDeleteNode() {
  const node = props.node
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
    await store.fetchNodes()
    success('代理节点已删除')
    emit('deleted', {
      nodeId: node.id,
      clearedSystemProxy: result.cleared_system_proxy,
      clearedExternalModelsProxy: result.cleared_external_models_proxy,
    })
    closeDialog()
  } catch (err: unknown) {
    toastError(parseApiError(err, '删除失败'))
  } finally {
    deletingNode.value = false
  }
}

function formatTestSuccessText(result: { latency_ms: number | null; exit_ip: string | null }): string {
  const parts: string[] = []
  if (result.latency_ms != null) parts.push(`延迟 ${result.latency_ms}ms`)
  if (result.exit_ip) parts.push(`出口 IP ${result.exit_ip}`)
  return parts.length ? `测试通过：${parts.join(' · ')}` : '测试通过'
}

async function handleTestUrl() {
  if (testingUrl.value || !proxyUrlValid.value) return
  testingUrl.value = true
  try {
    const result = await proxyNodesApi.testProxyUrl({
      proxy_url: form.value.proxyUrl.trim(),
      username: form.value.username.trim() || undefined,
      password: form.value.password || undefined,
    })
    testResult.value = result.success
      ? { kind: 'success', text: formatTestSuccessText(result) }
      : { kind: 'error', text: `测试失败: ${result.error || '未知错误'}` }
  } catch (err: unknown) {
    testResult.value = { kind: 'error', text: parseApiError(err, '测试请求失败') }
  } finally {
    testingUrl.value = false
  }
}
</script>
