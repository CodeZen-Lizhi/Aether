<script setup lang="ts">
import DesktopSettings from './DesktopSettings.vue'
import { useDesktopGateway } from './useDesktopGateway'

const gateway = useDesktopGateway()
const { status, available, canEditPort, pendingAction } = gateway
</script>

<template>
  <DesktopSettings
    v-if="status"
    :status="status"
    :disabled="!available"
    :can-edit-port="!!canEditPort"
    :saving-port="pendingAction === 'port'"
    @set-port="gateway.setPort"
    @set-autostart="gateway.setAutostart"
    @open-data-dir="gateway.openDataDir"
    @open-log-dir="gateway.openLogDir"
  />
</template>
