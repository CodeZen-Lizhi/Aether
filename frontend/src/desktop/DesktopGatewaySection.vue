<script setup lang="ts">
import DesktopSettings from './DesktopSettings.vue'
import { useDesktopGateway } from './useDesktopGateway'

const gateway = useDesktopGateway()
const { status, available, canEditPort, pendingAction, logs, logsLoading, logsError } = gateway
</script>

<template>
  <DesktopSettings
    v-if="status"
    :status="status"
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
</template>
