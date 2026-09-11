<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { ChevronDown, FileText, FolderOpen, LoaderCircle, RotateCw, Settings2, Wrench } from 'lucide-vue-next'
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
const expanded = ref(true)
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
    class="desktop-panel desktop-settings"
    aria-labelledby="settings-title"
  >
    <button
      v-if="!recovery"
      type="button"
      class="desktop-section-heading desktop-section-toggle"
      aria-controls="desktop-settings-content"
      :aria-expanded="expanded"
      :aria-label="legacyT('桌面应用')"
      @click="expanded = !expanded"
    >
      <Settings2
        class="h-4 w-4 text-muted-foreground"
        aria-hidden="true"
      />
      <h2 id="settings-title">
        桌面应用
      </h2>
      <ChevronDown
        class="ml-auto h-4 w-4 text-muted-foreground transition-transform"
        :class="{ 'rotate-180': expanded }"
        aria-hidden="true"
      />
    </button>
    <div
      v-else
      class="desktop-section-heading"
    >
      <Wrench
        class="h-4 w-4 text-muted-foreground"
        aria-hidden="true"
      />
      <h2 id="settings-title">
        恢复网关
      </h2>
    </div>

    <div
      v-show="recovery || expanded"
      id="desktop-settings-content"
      class="desktop-settings-content"
    >
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
            保存后会自动重启网关，仅允许本机访问。
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

        <div
          v-if="!recovery"
          class="desktop-autostart"
        >
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

      <div
        v-if="!recovery"
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
        v-if="!recovery"
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
    </div>
  </section>
</template>

<style scoped>
.desktop-panel { min-width: 0; border: 1px solid var(--border); border-radius: 8px; background: var(--card); }
.desktop-settings { padding: 20px 24px; }
.desktop-section-heading { display: flex; align-items: center; gap: 10px; }
.desktop-section-heading h2 { font-size: 15px; font-weight: 600; }
.desktop-section-toggle { width: 100%; border: 0; padding: 0; background: transparent; color: inherit; cursor: pointer; text-align: left; }
.desktop-section-toggle:focus-visible { outline: 2px solid var(--ring); outline-offset: 4px; border-radius: 4px; }
.desktop-settings-content { min-width: 0; }
.desktop-settings-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 28px; margin-top: 20px; }
.desktop-field { display: flex; min-width: 0; flex-direction: column; gap: 7px; }
.desktop-field label,
.desktop-autostart label { font-size: 13px; font-weight: 500; }
.desktop-field > p,
.desktop-autostart p { font-size: 12px; color: var(--muted-foreground); }
.desktop-port-input { display: flex; align-items: center; gap: 8px; }
.desktop-port-input input { min-width: 0; }
.desktop-autostart { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.desktop-autostart p { margin-top: 4px; }
.desktop-autostart > button { flex-shrink: 0; margin-top: 2px; }
.desktop-directories { margin-top: 20px; border-top: 1px solid var(--border); padding-top: 12px; }
.desktop-directory { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 4px 0; }
.desktop-directory > div { min-width: 0; }
.desktop-directory > button { flex-shrink: 0; }
.desktop-directory-label { font-size: 12px; color: var(--muted-foreground); }
.desktop-directory code { display: block; overflow-wrap: anywhere; font-size: 12px; user-select: text; }
.desktop-field-error { color: var(--destructive); }
.desktop-diagnostics { margin-top: 20px; border-top: 1px solid var(--border); padding-top: 12px; }
.desktop-diagnostics > summary { display: flex; cursor: pointer; align-items: center; gap: 10px; list-style: none; font-size: 13px; font-weight: 500; }
.desktop-diagnostics > summary::-webkit-details-marker { display: none; }
.desktop-diagnostics-content { margin-top: 12px; }
.desktop-diagnostics-toolbar { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 12px; }
.desktop-diagnostics-toolbar p { min-width: 0; font-size: 12px; color: var(--muted-foreground); }
.desktop-log-output { max-height: 280px; overflow: auto; margin-top: 12px; border: 1px solid var(--border); border-radius: 6px; background: var(--muted); padding: 10px; white-space: pre-wrap; overflow-wrap: anywhere; font: 11px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace; user-select: text; }
.desktop-log-empty { margin-top: 12px; font-size: 12px; color: var(--muted-foreground); }

@media (max-width: 640px) {
  .desktop-settings { padding: 20px; }
  .desktop-settings-grid { grid-template-columns: 1fr; gap: 20px; }
}
</style>
