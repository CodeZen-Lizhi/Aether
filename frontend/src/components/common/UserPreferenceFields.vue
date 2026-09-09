<script setup lang="ts">
import { ref } from 'vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import Select from '@/components/ui/select.vue'
import SelectTrigger from '@/components/ui/select-trigger.vue'
import SelectValue from '@/components/ui/select-value.vue'
import SelectContent from '@/components/ui/select-content.vue'
import SelectItem from '@/components/ui/select-item.vue'

defineProps<{ theme: string; language: string; timezone: string; disabled?: boolean }>()
const emit = defineEmits<{
  'update:theme': [value: string]
  'update:language': [value: string]
  'update:timezone': [value: string]
  timezoneChange: []
}>()
const themeSelectOpen = ref(false)
const languageSelectOpen = ref(false)

function selectTheme(value: string) {
  themeSelectOpen.value = false
  emit('update:theme', value)
}

function selectLanguage(value: string) {
  languageSelectOpen.value = false
  emit('update:language', value)
}
</script>

<template>
  <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
    <div>
      <Label for="theme">主题</Label>
      <Select
        v-model:open="themeSelectOpen"
        :model-value="theme"
        :disabled="disabled"
        @update:model-value="selectTheme"
      >
        <SelectTrigger
          id="theme"
          class="mt-1"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="light">
            浅色
          </SelectItem>
          <SelectItem value="dark">
            深色
          </SelectItem>
          <SelectItem value="system">
            跟随系统
          </SelectItem>
        </SelectContent>
      </Select>
    </div>
    <div>
      <Label for="language">语言</Label>
      <Select
        v-model:open="languageSelectOpen"
        :model-value="language"
        :disabled="disabled"
        @update:model-value="selectLanguage"
      >
        <SelectTrigger
          id="language"
          class="mt-1"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="zh-CN">
            简体中文
          </SelectItem>
          <SelectItem value="en">
            English
          </SelectItem>
        </SelectContent>
      </Select>
    </div>
    <div>
      <Label for="timezone">时区</Label>
      <Input
        id="timezone"
        :model-value="timezone"
        :disabled="disabled"
        placeholder="Asia/Shanghai"
        class="mt-1"
        @update:model-value="emit('update:timezone', $event)"
        @change="emit('timezoneChange')"
      />
    </div>
  </div>
</template>
