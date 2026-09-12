<script setup lang="ts">
import DesktopSettings from './DesktopSettings.vue'
import Button from '@/components/ui/button.vue'
import { useDesktopGateway } from './useDesktopGateway'

const props = defineProps<{
  gateway?: ReturnType<typeof useDesktopGateway>
  view?: 'connection' | 'diagnostics'
}>()
const gateway = props.gateway ?? useDesktopGateway()
const { status, available, canEditPort, pendingAction, logs, logsLoading, logsError, errors, loading } = gateway
</script>

<template>
  <div class="settings-group">
    <DesktopSettings
      v-if="status"
      :status="status"
      :view="view"
      :disabled="!available"
      :can-edit-port="!!canEditPort"
      :saving-port="pendingAction === 'port'"
      :logs="logs"
      :logs-loading="logsLoading"
      :logs-error="logsError"
      @set-port="gateway.setPort"
      @set-autostart="gateway.setAutostart"
      @open-data-dir="gateway.openDataDir"
      @open-log-dir="gateway.openLogDir"
      @refresh-logs="gateway.refreshLogs"
    />
    <div
      v-if="errors.length"
      class="settings-toolbar"
      role="alert"
    >
      <p class="settings-error">
        {{ errors.join('\n') }}
      </p>
      <Button
        variant="outline"
        size="sm"
        :disabled="loading"
        @click="gateway.refreshStatus"
      >
        重试
      </Button>
    </div>
  </div>
</template>
