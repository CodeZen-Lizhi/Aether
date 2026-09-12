<script setup lang="ts">
import { Loader2, Save } from 'lucide-vue-next'
import Button from '@/components/ui/button.vue'

withDefaults(defineProps<{
  hasChanges: boolean
  loading: boolean
  error?: string
  label?: string
}>(), { error: '', label: '保存' })

defineEmits<{ save: []; cancel: [] }>()
</script>

<template>
  <div
    v-if="hasChanges || loading || error"
    class="settings-save"
    :aria-busy="loading"
  >
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p>
    <div class="settings-actions">
      <Button
        variant="ghost"
        size="sm"
        :disabled="loading"
        @click="$emit('cancel')"
      >
        取消
      </Button>
      <Button
        size="sm"
        :disabled="loading || !hasChanges"
        @click="$emit('save')"
      >
        <Loader2
          v-if="loading"
          class="mr-2 h-4 w-4 animate-spin"
          aria-hidden="true"
        />
        <Save
          v-else
          class="mr-2 h-4 w-4"
          aria-hidden="true"
        />
        {{ loading ? '保存中...' : label }}
      </Button>
    </div>
  </div>
</template>
