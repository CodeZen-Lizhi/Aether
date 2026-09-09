<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { meApi } from '@/api/me'
import { CardSection } from '@/components/layout'
import Button from '@/components/ui/button.vue'
import UserPreferenceFields from '@/components/common/UserPreferenceFields.vue'
import { useDarkMode, type ThemeMode } from '@/composables/useDarkMode'
import { useToast } from '@/composables/useToast'
import { useI18n } from '@/i18n'

const { themeMode, setThemeMode } = useDarkMode()
const { locale, setLocale } = useI18n()
const { success, error: showError } = useToast()
const preferences = ref({ theme: themeMode.value, language: locale.value === 'en-US' ? 'en' : 'zh-CN', timezone: 'Asia/Shanghai' })
const original = ref({ ...preferences.value })
const loading = ref(true)
const saving = ref(false)
const loadError = ref(false)
const hasChanges = computed(() => preferences.value.theme !== original.value.theme
  || preferences.value.language !== original.value.language
  || preferences.value.timezone !== original.value.timezone)

async function loadPreferences() {
  loading.value = true
  loadError.value = false
  try {
    const stored = await meApi.getPreferences()
    preferences.value = {
      theme: themeMode.value,
      language: locale.value === 'en-US' ? 'en' : 'zh-CN',
      timezone: stored.timezone || 'Asia/Shanghai',
    }
    original.value = { ...preferences.value }
  } catch {
    loadError.value = true
  } finally {
    loading.value = false
  }
}

async function savePreferences() {
  if (loading.value || saving.value || loadError.value || !hasChanges.value) return
  saving.value = true
  const next = { ...preferences.value }
  try {
    await meApi.updatePreferences({ ...next, timezone: next.timezone || undefined })
    setThemeMode(next.theme)
    setLocale(next.language === 'en' ? 'en-US' : 'zh-CN')
    original.value = next
    success('设置已保存')
  } catch {
    showError('保存设置失败')
  } finally {
    saving.value = false
  }
}

// Header shortcuts also change these local preferences. Keep untouched fields in sync.
watch(themeMode, next => {
  if (preferences.value.theme === original.value.theme) {
    preferences.value.theme = next
    original.value.theme = next
  }
})
watch(locale, next => {
  if (preferences.value.language === original.value.language) {
    preferences.value.language = next === 'en-US' ? 'en' : 'zh-CN'
    original.value.language = preferences.value.language
  }
})

onMounted(loadPreferences)
</script>

<template>
  <CardSection
    title="偏好设置"
    description="主题、语言和时区"
    :aria-busy="loading || saving"
  >
    <template #actions>
      <Button
        size="sm"
        class="bg-blue-600 hover:bg-blue-700"
        :disabled="loading || saving || loadError || !hasChanges"
        @click="savePreferences"
      >
        {{ saving ? '保存中...' : '保存' }}
      </Button>
    </template>
    <p
      v-if="loading"
      class="text-sm text-muted-foreground"
      role="status"
    >
      正在加载偏好设置…
    </p>
    <div
      v-else-if="loadError"
      class="flex items-center justify-between gap-4"
    >
      <p
        class="text-sm text-destructive"
        role="alert"
      >
        加载偏好设置失败
      </p>
      <Button
        variant="outline"
        size="sm"
        @click="loadPreferences"
      >
        重试
      </Button>
    </div>
    <UserPreferenceFields
      v-else
      v-model:language="preferences.language"
      v-model:timezone="preferences.timezone"
      :theme="preferences.theme"
      :disabled="saving"
      @update:theme="preferences.theme = $event as ThemeMode"
    />
  </CardSection>
</template>
