<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { FolderOpen, LoaderCircle, Settings2 } from 'lucide-vue-next'
import Button from '@/components/ui/button.vue'
import Input from '@/components/ui/input.vue'
import Switch from '@/components/ui/switch.vue'
import type { DesktopStatus } from './bridge'
import { validatePort } from './validation'

const props = defineProps<{
  status: DesktopStatus
  disabled: boolean
  canEditPort: boolean
  savingPort: boolean
}>()
const emit = defineEmits<{
  setPort: [port: number]
  setAutostart: [enabled: boolean]
  openDataDir: []
  openLogDir: []
}>()

const port = ref(String(props.status.port))
const submitted = ref(false)
const portError = computed(() => submitted.value ? validatePort(port.value) : '')
const hasChanges = computed(() => port.value !== String(props.status.port))
const portErrorElement = ref<HTMLElement | null>(null)

watch(() => props.status.port, next => {
  port.value = String(next)
  submitted.value = false
})

async function savePort() {
  if (props.disabled || !props.canEditPort || !hasChanges.value) return
  submitted.value = true
  if (portError.value) {
    await nextTick()
    portErrorElement.value?.focus()
    return
  }
  emit('setPort', Number(port.value))
}
</script>

<template>
  <section
    class="desktop-panel desktop-settings"
    aria-labelledby="settings-title"
  >
    <div class="desktop-section-heading">
      <Settings2
        class="h-4 w-4 text-muted-foreground"
        aria-hidden="true"
      />
      <h2 id="settings-title">
        客户端设置
      </h2>
    </div>

    <div class="desktop-settings-grid">
      <form
        class="desktop-field"
        novalidate
        :aria-busy="savingPort"
        @submit.prevent="savePort"
      >
        <label for="gateway-port">网关端口</label>
        <div class="desktop-port-input">
          <Input
            id="gateway-port"
            v-model="port"
            type="number"
            inputmode="numeric"
            min="1024"
            max="65535"
            step="1"
            :disabled="disabled || !canEditPort"
            :aria-invalid="!!portError"
            aria-describedby="gateway-port-hint gateway-port-error"
          />
          <Button
            type="submit"
            variant="outline"
            class="shrink-0 gap-2"
            :disabled="disabled || !canEditPort || !hasChanges"
          >
            <LoaderCircle
              v-if="savingPort"
              class="desktop-spin h-4 w-4"
              aria-hidden="true"
            />
            保存端口
          </Button>
        </div>
        <p id="gateway-port-hint">
          停止网关后可修改，仅允许本机访问。
        </p>
        <p
          v-if="portError"
          id="gateway-port-error"
          ref="portErrorElement"
          class="desktop-field-error"
          tabindex="-1"
          role="alert"
        >
          {{ portError }}
        </p>
      </form>

      <div class="desktop-autostart">
        <div>
          <label
            id="autostart-label"
            for="desktop-autostart"
          >登录 Mac 时自动启动</label>
          <p id="autostart-hint">
            启动 Aether 并运行本机网关。
          </p>
        </div>
        <Switch
          id="desktop-autostart"
          :model-value="status.autostart"
          :disabled="disabled"
          aria-labelledby="autostart-label"
          aria-describedby="autostart-hint"
          @update:model-value="emit('setAutostart', $event)"
        />
      </div>
    </div>

    <div class="desktop-directories">
      <div class="desktop-directory">
        <div>
          <p class="desktop-directory-label">
            数据目录
          </p>
          <code>{{ status.data_dir || '—' }}</code>
        </div>
        <Button
          variant="ghost"
          size="icon"
          :disabled="disabled || !status.data_dir"
          aria-label="打开数据目录"
          title="打开数据目录"
          @click="emit('openDataDir')"
        >
          <FolderOpen
            class="h-4 w-4"
            aria-hidden="true"
          />
        </Button>
      </div>
      <div class="desktop-directory">
        <div>
          <p class="desktop-directory-label">
            日志目录
          </p>
          <code>{{ status.log_dir || '—' }}</code>
        </div>
        <Button
          variant="ghost"
          size="icon"
          :disabled="disabled || !status.log_dir"
          aria-label="打开日志目录"
          title="打开日志目录"
          @click="emit('openLogDir')"
        >
          <FolderOpen
            class="h-4 w-4"
            aria-hidden="true"
          />
        </Button>
      </div>
    </div>
  </section>
</template>
