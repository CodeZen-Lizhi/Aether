import { computed, onMounted, onUnmounted, readonly, ref, shallowRef } from 'vue'
import { desktopApi, describeDesktopError, type DesktopStatus } from './bridge'

type DesktopAction = 'start' | 'stop' | 'restart' | 'port' | 'autostart' | 'dashboard' | 'data' | 'logs' | 'quit'

export function useDesktopGateway() {
  const status = shallowRef<DesktopStatus | null>(null)
  const loading = ref(true)
  const pendingAction = ref<DesktopAction | null>(null)
  const connectionError = ref('')
  const operationError = ref('')
  const logs = ref<string[]>([])
  const logsLoading = ref(false)
  const logsError = ref('')
  let disposed = false
  let timer: ReturnType<typeof setTimeout> | undefined
  let inFlight: Promise<void> | null = null
  let revision = 0

  const phase = computed(() => {
    if (pendingAction.value === 'start' || pendingAction.value === 'restart') return 'starting'
    if (pendingAction.value === 'stop' || pendingAction.value === 'quit') return 'stopping'
    return status.value?.phase ?? null
  })
  const transitioning = computed(() => phase.value === 'starting' || phase.value === 'stopping')
  const busy = computed(() => !!pendingAction.value || transitioning.value)
  const available = computed(() => !!status.value && !connectionError.value && !busy.value)
  const canStart = computed(() => available.value
    && (phase.value === 'setup' || phase.value === 'stopped' || phase.value === 'failed') && status.value?.pid === null)
  const canStop = computed(() => available.value && (phase.value === 'running' || status.value?.pid != null))
  const canEditPort = computed(() => available.value
    && (phase.value === 'setup' || phase.value === 'stopped' || phase.value === 'failed') && status.value?.pid === null)
  const errors = computed(() => [...new Set([
    connectionError.value, operationError.value, status.value?.error,
  ].filter((error): error is string => !!error))])

  function clearTimer() {
    if (timer !== undefined) clearTimeout(timer)
    timer = undefined
  }

  function schedule(delay = transitioning.value ? 1000 : 3000) {
    clearTimer()
    if (!disposed) timer = setTimeout(() => { void refreshStatus() }, delay)
  }

  async function refreshStatus(): Promise<void> {
    if (disposed) return
    if (inFlight) return inFlight
    if (pendingAction.value) {
      schedule()
      return
    }
    clearTimer()
    const requestRevision = revision
    loading.value = true
    inFlight = (async () => {
      try {
        const next = await desktopApi.status()
        if (!disposed && requestRevision === revision) {
          status.value = next
          connectionError.value = ''
        }
      } catch (error) {
        if (!disposed && requestRevision === revision) connectionError.value = describeDesktopError(error)
      } finally {
        inFlight = null
        if (!disposed) {
          loading.value = false
          schedule()
        }
      }
    })()
    return inFlight
  }

  async function perform<T>(action: DesktopAction, operation: () => Promise<T>, apply?: (value: T) => void): Promise<T | undefined> {
    if (disposed || pendingAction.value) return undefined
    clearTimer()
    revision += 1 // A poll started before a user action must never overwrite its result.
    pendingAction.value = action
    operationError.value = ''
    try {
      const result = await operation()
      if (!disposed) {
        apply?.(result)
        return result
      }
    } catch (error) {
      if (!disposed) operationError.value = describeDesktopError(error)
    } finally {
      if (!disposed) {
        pendingAction.value = null
        schedule(0)
      }
    }
    return undefined
  }

  function updateStatus(next: DesktopStatus) {
    status.value = next
    connectionError.value = ''
  }

  async function refreshLogs() {
    if (disposed || logsLoading.value) return
    logsLoading.value = true
    logsError.value = ''
    try {
      const next = await desktopApi.logs()
      if (!disposed) logs.value = next
    } catch (error) {
      if (!disposed) logsError.value = describeDesktopError(error)
    } finally {
      if (!disposed) logsLoading.value = false
    }
  }

  onMounted(() => { void refreshStatus() })
  onUnmounted(() => {
    disposed = true
    revision += 1
    clearTimer()
  })

  return {
    status: readonly(status), loading: readonly(loading), pendingAction: readonly(pendingAction),
    connectionError: readonly(connectionError), errors, phase, busy, available, canStart, canStop, canEditPort,
    logs: readonly(logs), logsLoading: readonly(logsLoading), logsError: readonly(logsError),
    refreshStatus, refreshLogs,
    start: () => perform('start', desktopApi.start, updateStatus),
    stop: () => perform('stop', desktopApi.stop, updateStatus),
    restart: () => perform('restart', desktopApi.restart, updateStatus),
    setPort: (port: number) => perform('port', () => desktopApi.setPort(port), updateStatus),
    setAutostart: (enabled: boolean) => perform('autostart', () => desktopApi.setAutostart(enabled), updateStatus),
    openDashboard: () => perform('dashboard', desktopApi.openDashboard),
    openDataDir: () => perform('data', desktopApi.openDataDir),
    openLogDir: () => perform('logs', desktopApi.openLogDir),
    quit: () => perform('quit', desktopApi.quit),
  }
}
