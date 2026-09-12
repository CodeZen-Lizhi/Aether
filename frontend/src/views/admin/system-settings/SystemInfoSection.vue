<template>
  <section class="settings-group">
    <h3 class="settings-heading">
      诊断
    </h3>
    <div class="settings-row">
      <span class="settings-label">当前版本</span>
      <samp class="text-sm [overflow-wrap:anywhere]">{{ systemVersion || '-' }}</samp>
    </div>
  </section>
</template>

<script setup lang="ts">
defineProps<{ systemVersion: string }>()
</script>
