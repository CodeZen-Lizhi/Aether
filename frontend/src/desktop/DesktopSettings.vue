<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { ChevronDown, FileText, FolderOpen, LoaderCircle, RotateCw, Save, Wrench } from 'lucide-vue-next'
import Button from '@/components/ui/button.vue'
import Input from '@/components/ui/input.vue'
import Switch from '@/components/ui/switch.vue'
import { useI18n } from '@/i18n'
import type { DesktopStatus } from './bridge'
import { validatePort } from './validation'

const props = defineProps<{
  status: DesktopStatus
  disabled: boolean
  canEditPort: boolean
  savingPort: boolean
  recovery?: boolean
  view?: 'connection' | 'diagnostics'
  logs?: readonly string[]
  logsLoading?: boolean
  logsError?: string
}>()
const emit = defineEmits<{
  setPort: [port: number]
  setAutostart: [enabled: boolean]
  openDataDir: []
  openLogDir: []
  refreshLogs: []
}>()
const { legacyT } = useI18n()

const port = ref(String(props.status.port))
const submitted = ref(false)
const portError = computed(() => submitted.value ? validatePort(port.value) : '')
const hasChanges = computed(() => port.value !== String(props.status.port))
const portErrorElement = ref<HTMLElement | null>(null)
const diagnosticLogs = computed(() => props.logs ?? [])

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

function toggleLogs(event: Event) {
  if ((event.currentTarget as HTMLElement).hasAttribute('open')) emit('refreshLogs')
}
</script>

<template>
  <section
    class="desktop-settings settings-group"
    :class="{ 'desktop-panel': recovery }"
  >
    <template v-if="recovery || view !== 'diagnostics'">
      <h2
        v-if="recovery"
        class="desktop-section-heading"
      >
        <Wrench
          class="h-4 w-4"
          aria-hidden="true"
        />
        恢复网关
      </h2>
      <form
        :class="recovery ? 'desktop-field' : 'desktop-port-row settings-row'"
        novalidate
        :aria-busy="savingPort"
        @submit.prevent="savePort"
      >
        <div>
          <label for="gateway-port">网关端口</label>
          <p
            id="gateway-port-hint"
            class="settings-description"
          >
            保存后会自动重启网关，仅允许本机访问。
          </p>
        </div>
        <div class="settings-row-control desktop-port-control">
          <Input
            id="gateway-port"
            v-model="port"
            type="number"
            inputmode="numeric"
            min="1024"
            max="65535"
            step="1"
            class="rounded-md"
            :disabled="disabled || !canEditPort"
            :aria-invalid="!!portError"
            aria-describedby="gateway-port-hint gateway-port-error"
          />
        </div>
        <p
          v-if="portError"
          id="gateway-port-error"
          ref="portErrorElement"
          class="desktop-field-error desktop-port-feedback"
          tabindex="-1"
          role="alert"
        >
          {{ portError }}
        </p>
        <div
          v-if="hasChanges || savingPort"
          class="settings-actions desktop-port-actions desktop-port-feedback"
        >
          <Button
            variant="ghost"
            size="sm"
            :disabled="disabled"
            @click="port = String(status.port); submitted = false"
          >
            取消
          </Button>
          <Button
            type="submit"
            size="sm"
            :disabled="disabled || !canEditPort || !hasChanges"
          >
            <LoaderCircle
              v-if="savingPort"
              class="desktop-spin h-4 w-4"
              aria-hidden="true"
            />
            <Save
              v-else
              class="h-4 w-4"
              aria-hidden="true"
            />
            {{ savingPort ? '保存中...' : recovery ? '保存端口' : '保存并重启' }}
          </Button>
        </div>
      </form>
      <div
        v-if="!recovery"
        class="desktop-autostart settings-row settings-row--toggle"
      >
        <div>
          <label
            id="autostart-label"
            for="desktop-autostart"
          >开机启动</label>
          <p
            id="autostart-hint"
            class="settings-description"
          >
            登录 Mac 时启动 Aether 和网关。
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
    </template>
    <template v-else>
      <div
        class="desktop-directories"
      >
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

      <details
        class="desktop-diagnostics group"
        @toggle="toggleLogs"
      >
        <summary>
          <FileText
            class="h-4 w-4 text-muted-foreground"
            aria-hidden="true"
          />
          <span>诊断日志</span>
          <ChevronDown
            class="ml-auto h-4 w-4 text-muted-foreground transition-transform group-open:rotate-180"
            aria-hidden="true"
          />
        </summary>
        <div class="desktop-diagnostics-content">
          <div class="desktop-diagnostics-toolbar">
            <p>最近 100 行，完整记录可在日志目录查看。</p>
            <div class="flex shrink-0 gap-2">
              <Button
                variant="ghost"
                size="sm"
                :disabled="disabled || !status.log_dir"
                @click="emit('openLogDir')"
              >
                打开日志目录
              </Button>
              <Button
                variant="outline"
                size="sm"
                class="gap-2"
                :disabled="logsLoading"
                @click="emit('refreshLogs')"
              >
                <RotateCw
                  class="h-3.5 w-3.5"
                  :class="{ 'desktop-spin': logsLoading }"
                  aria-hidden="true"
                />
                {{ logsLoading ? '读取中…' : '刷新日志' }}
              </Button>
            </div>
          </div>
          <p
            v-if="logsError"
            class="desktop-field-error"
            role="alert"
          >
            {{ logsError }}
          </p>
          <pre
            v-if="diagnosticLogs.length"
            class="desktop-log-output"
            tabindex="0"
            :aria-label="legacyT('最近的诊断日志')"
          >{{ diagnosticLogs.join('\n') }}</pre>
          <p
            v-else-if="!logsError"
            class="desktop-log-empty"
          >
            {{ logsLoading ? '正在读取日志…' : '暂无诊断日志。' }}
          </p>
        </div>
      </details>
    </template>
  </section>
</template>

<style scoped>
.desktop-panel { min-width: 0; border: 1px solid var(--border); border-radius: 8px; background: var(--card); padding: 20px 24px; }
.desktop-settings { min-width: 0; }
.desktop-section-heading { display: flex; align-items: center; gap: 10px; font-size: 15px; font-weight: 600; }
.desktop-field { display: flex; min-width: 0; flex-direction: column; gap: 8px; padding: 0 0 16px; }
.desktop-panel .desktop-field { padding-top: 20px; }
.desktop-field label,
.desktop-autostart label { font-size: 13px; font-weight: 500; }
.desktop-field > p { font-size: 12px; color: var(--muted-foreground); }
.desktop-field .settings-description { margin-top: 4px; font-size: 12px; line-height: 1.5; color: var(--muted-foreground); }
.desktop-port-actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 8px; }
.desktop-port-actions > button { gap: 8px; }
.desktop-port-row .desktop-port-feedback { grid-column: 1 / -1; margin: 0; font-size: 12px; }
.desktop-port-row .desktop-port-actions { justify-content: end; }
.desktop-port-control { display: flex; justify-content: end; }
.desktop-port-control > input { width: 128px; }
.desktop-panel .desktop-port-control { justify-content: start; }
.desktop-directory { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 12px 0; border-bottom: 1px solid var(--border); }
.desktop-directory > div { min-width: 0; }
.desktop-directory > button { flex-shrink: 0; border-radius: 6px; }
.desktop-directory-label { font-size: 12px; color: var(--muted-foreground); }
.desktop-directory code { display: block; overflow-wrap: anywhere; font-size: 12px; user-select: text; margin-top: 4px; }
.desktop-field .desktop-field-error,
.desktop-field-error { color: var(--destructive); }
.desktop-diagnostics { margin-top: 16px; }
.desktop-diagnostics > summary { display: flex; cursor: pointer; align-items: center; gap: 10px; list-style: none; font-size: 13px; font-weight: 500; min-height: 40px; }
.desktop-diagnostics > summary::-webkit-details-marker { display: none; }
.desktop-diagnostics-content { margin-top: 12px; }
.desktop-diagnostics-toolbar { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 12px; }
.desktop-diagnostics-toolbar p { min-width: 0; font-size: 12px; color: var(--muted-foreground); }
.desktop-diagnostics-toolbar > div { flex-wrap: wrap; }
.desktop-log-output { max-height: 280px; overflow: auto; margin-top: 12px; border: 1px solid var(--border); border-radius: 6px; background: var(--muted); padding: 10px; white-space: pre-wrap; overflow-wrap: anywhere; font: 11px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace; user-select: text; }
.desktop-log-empty { margin-top: 12px; font-size: 12px; color: var(--muted-foreground); }
</style>
