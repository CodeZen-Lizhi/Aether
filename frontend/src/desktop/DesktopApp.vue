<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Activity, ArrowUpRight, Check, ChevronDown, CircleAlert, Copy, FileText, LoaderCircle, LogOut, Play, RotateCw, Square } from 'lucide-vue-next'
import Button from '@/components/ui/button.vue'
import LanguageSwitcher from '@/components/common/LanguageSwitcher.vue'
import ThemeModeButton from '@/components/common/ThemeModeButton.vue'
import { useClipboard } from '@/composables/useClipboard'
import { useI18n } from '@/i18n'
import DesktopSettings from './DesktopSettings.vue'
import { useDesktopGateway } from './useDesktopGateway'
import type { GatewayPhase } from './bridge'

const gateway = useDesktopGateway()
const { status, phase, loading, pendingAction, connectionError, errors, busy, available, canStart, canStop, logs, logsLoading, logsError } = gateway
const { copyToClipboard } = useClipboard()
const { legacyT } = useI18n()
const copied = ref(false)
const copyFailed = ref(false)
const copyBusy = ref(false)
const openAfterStart = ref(false)

const phaseLabels: Record<GatewayPhase, string> = {
  setup: '等待启动', starting: '正在启动', running: '运行中', stopping: '正在停止', stopped: '已停止', failed: '运行异常',
}
const phaseHeadings: Record<GatewayPhase, string> = {
  setup: '正在准备本机网关', starting: '正在启动网关', running: '网关正在运行',
  stopping: '正在安全停止网关', stopped: '网关已停止', failed: '网关需要处理',
}
const phaseDescriptions: Record<GatewayPhase, string> = {
  setup: '首次启动会自动准备本地数据。',
  starting: '正在准备本地服务，通常只需几秒。',
  running: '已准备好接收这台 Mac 上的请求。',
  stopping: '正在等待在途请求结束并保存数据。',
  stopped: '配置与数据已保留，随时可以再次启动。',
  failed: '查看下方错误信息，处理后可以重新启动。',
}
const statusLabel = computed(() => connectionError.value ? '状态不可用' : phase.value ? phaseLabels[phase.value] : '正在连接')
const statusHeading = computed(() => connectionError.value ? '无法确认网关状态' : phase.value ? phaseHeadings[phase.value] : '正在读取网关状态')
const statusDescription = computed(() => connectionError.value
  ? '请重试连接，或退出并重新打开 Aether。'
  : phase.value ? phaseDescriptions[phase.value] : '正在连接 Aether 的本地服务。')
const statusTone = computed(() => connectionError.value ? 'failed' : phase.value ?? 'loading')
const loadingPhase = computed(() => phase.value === 'starting' || phase.value === 'stopping' || (!status.value && loading.value))
const canInterruptStartup = computed(() => pendingAction.value === 'start' || pendingAction.value === 'restart')

watch(() => status.value?.gateway_url, () => {
  copied.value = false
  copyFailed.value = false
})
watch([() => status.value?.phase, pendingAction, openAfterStart], ([nextPhase, action, shouldOpen]) => {
  if (nextPhase === 'failed') openAfterStart.value = false
  if (shouldOpen && nextPhase === 'running' && !action) {
    openAfterStart.value = false
    void gateway.openDashboard()
  }
}, { flush: 'post' })

async function startGateway() {
  const result = await gateway.start()
  if (result && result.phase !== 'failed') {
    // The host opens the initial dashboard. Only explicit launcher starts open it here.
    openAfterStart.value = true
  }
}

async function copyAddress() {
  if (!status.value || copyBusy.value) return
  copyBusy.value = true
  const url = status.value.gateway_url
  const success = await copyToClipboard(url, false)
  if (status.value?.gateway_url === url) {
    copied.value = success
    copyFailed.value = !success
  }
  copyBusy.value = false
}

function toggleLogs(event: Event) {
  if ((event.currentTarget as HTMLElement).hasAttribute('open')) void gateway.refreshLogs()
}
</script>

<template>
  <div class="desktop-shell">
    <header class="desktop-header">
      <div class="desktop-brand">
        <img
          src="/aether_adaptive.svg"
          class="desktop-brand-mark"
          alt=""
          width="36"
          height="36"
        >
        <div>
          <h1>Aether</h1>
          <p>本机网关</p>
        </div>
      </div>
      <div class="desktop-header-actions">
        <span
          v-if="status?.version"
          class="desktop-version"
        >v{{ status.version }}</span>
        <LanguageSwitcher />
        <ThemeModeButton />
      </div>
    </header>

    <main class="desktop-main">
      <section
        class="desktop-panel desktop-status"
        :data-phase="statusTone"
        aria-labelledby="gateway-status-title"
      >
        <div class="desktop-status-meta">
          <span
            class="desktop-status-badge"
            role="status"
            aria-live="polite"
            aria-atomic="true"
          >
            <LoaderCircle
              v-if="loadingPhase"
              class="desktop-spin h-3.5 w-3.5"
              aria-hidden="true"
            />
            <span
              v-else
              class="desktop-status-dot"
              aria-hidden="true"
            />
            {{ statusLabel }}
          </span>
          <span class="desktop-local-label">仅本机访问</span>
        </div>
        <h2 id="gateway-status-title">
          {{ statusHeading }}
        </h2>
        <p class="desktop-status-description">
          {{ statusDescription }}
        </p>

        <template v-if="status">
          <div class="desktop-address">
            <div>
              <p>API 基础地址</p>
              <code>{{ status.gateway_url }}</code>
            </div>
            <Button
              variant="ghost"
              size="icon"
              :disabled="copyBusy"
              :aria-label="copied ? '地址已复制' : '复制 API 基础地址'"
              :title="copied ? '地址已复制' : '复制 API 基础地址'"
              @click="copyAddress"
            >
              <component
                :is="copied ? Check : Copy"
                class="h-4 w-4"
                aria-hidden="true"
              />
            </Button>
          </div>
          <p
            v-if="copied || copyFailed"
            class="desktop-copy-feedback"
            :class="{ 'desktop-field-error': copyFailed }"
            role="status"
          >
            {{ copyFailed ? '复制失败，请选中地址手动复制。' : '地址已复制' }}
          </p>

          <div class="desktop-gateway-actions">
            <Button
              v-if="phase === 'running'"
              class="gap-2"
              :disabled="!available"
              @click="gateway.openDashboard"
            >
              {{ pendingAction === 'dashboard' ? '正在打开…' : '打开管理界面' }}
              <ArrowUpRight
                class="h-4 w-4"
                aria-hidden="true"
              />
            </Button>
            <Button
              v-else
              class="gap-2"
              :disabled="!canStart"
              @click="startGateway"
            >
              <LoaderCircle
                v-if="loadingPhase"
                class="desktop-spin h-4 w-4"
                aria-hidden="true"
              />
              <Play
                v-else
                class="h-4 w-4"
                aria-hidden="true"
              />
              {{ phase === 'starting' ? '正在启动…' : phase === 'stopping' ? '正在停止…' : '启动网关' }}
            </Button>
            <Button
              variant="outline"
              class="gap-2"
              :disabled="!available || phase !== 'running'"
              @click="gateway.restart"
            >
              <RotateCw
                class="h-4 w-4"
                aria-hidden="true"
              />
              重启
            </Button>
            <Button
              variant="ghost"
              class="gap-2"
              :disabled="!canStop && !canInterruptStartup"
              @click="gateway.stop"
            >
              <Square
                class="h-3.5 w-3.5"
                aria-hidden="true"
              />
              停止网关
            </Button>
          </div>
        </template>
      </section>

      <div
        v-if="errors.length"
        class="desktop-error-panel"
        role="alert"
      >
        <CircleAlert
          class="mt-0.5 h-5 w-5 shrink-0"
          aria-hidden="true"
        />
        <div class="min-w-0 flex-1">
          <p class="font-semibold">
            需要处理
          </p>
          <p
            v-for="error in errors"
            :key="error"
            class="desktop-error-message"
          >
            {{ error }}
          </p>
          <Button
            v-if="connectionError"
            variant="outline"
            size="sm"
            class="mt-3 gap-2"
            :disabled="loading || busy"
            @click="gateway.refreshStatus"
          >
            <RotateCw
              class="h-3.5 w-3.5"
              aria-hidden="true"
            />
            重试连接
          </Button>
        </div>
      </div>

      <DesktopSettings
        v-if="status && phase === 'failed'"
        recovery
        :status="status"
        :disabled="!available"
        :can-edit-port="!!canStart"
        :saving-port="pendingAction === 'port'"
        @set-port="gateway.setPort"
      />

      <details
        v-if="status"
        class="desktop-panel desktop-diagnostics group"
        :open="phase === 'failed'"
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
                :disabled="busy || !status.log_dir"
                @click="gateway.openLogDir"
              >
                打开日志目录
              </Button>
              <Button
                variant="outline"
                size="sm"
                class="gap-2"
                :disabled="logsLoading"
                @click="gateway.refreshLogs"
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
            v-if="logs.length"
            class="desktop-log-output"
            tabindex="0"
            :aria-label="legacyT('最近的诊断日志')"
          >{{ logs.join('\n') }}</pre>
          <p
            v-else-if="!logsError"
            class="desktop-log-empty"
          >
            {{ logsLoading ? '正在读取日志…' : '暂无诊断日志。' }}
          </p>
        </div>
      </details>
    </main>

    <footer class="desktop-footer">
      <p>
        <Activity
          class="h-4 w-4 shrink-0"
          aria-hidden="true"
        />
        关闭窗口后，网关继续在菜单栏运行。
      </p>
      <Button
        variant="ghost"
        size="sm"
        class="shrink-0 gap-2"
        :disabled="busy || !status"
        title="退出并停止网关"
        @click="gateway.quit"
      >
        <LogOut
          class="h-3.5 w-3.5"
          aria-hidden="true"
        />
        退出 Aether
      </Button>
    </footer>
  </div>
</template>
