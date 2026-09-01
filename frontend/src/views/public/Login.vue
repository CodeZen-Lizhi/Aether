<template>
  <div class="min-h-screen flex items-center justify-center bg-background px-4">
    <div class="w-full max-w-sm">
      <div class="flex flex-col items-center mb-8">
        <img src="/aether_adaptive.svg" alt="Aether" class="w-16 h-16 mb-4" />
        <h1 class="text-2xl font-semibold text-foreground">{{ siteName }}</h1>
        <p class="text-sm text-muted-foreground mt-1">{{ siteSubtitle }}</p>
      </div>

      <form ref="loginFormEl" class="space-y-4" @submit.prevent="handleLogin">
        <div class="space-y-2">
          <Label for="username">{{ t('auth.login.usernameEmail') }}</Label>
          <Input
            id="username"
            v-model="form.email"
            name="username"
            type="text"
            autocomplete="username"
            :placeholder="t('auth.login.usernameEmail')"
            required
          />
        </div>
        <div class="space-y-2">
          <Label for="password">{{ t('auth.login.password') }}</Label>
          <Input
            id="password"
            v-model="form.password"
            name="password"
            type="password"
            autocomplete="current-password"
            :placeholder="t('auth.login.password')"
            required
          />
        </div>
        <Button type="submit" class="w-full" :disabled="authStore.loading">
          {{ authStore.loading ? t('auth.login.submitting') : t('auth.login.submit') }}
        </Button>
      </form>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import Button from '@/components/ui/button.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import { useAuthStore } from '@/stores/auth'
import { useToast } from '@/composables/useToast'
import { useSiteInfo } from '@/composables/useSiteInfo'
import { navigateAfterLogin } from '@/features/auth/utils/loginRedirect'
import { useI18n } from '@/i18n'

const router = useRouter()
const authStore = useAuthStore()
const { success: showSuccess, warning: showWarning, error: showError } = useToast()
const { siteName, siteSubtitle } = useSiteInfo()
const { t } = useI18n()

const loginFormEl = ref<HTMLFormElement | null>(null)
const form = ref({ email: '', password: '' })

function readCurrentLoginCredentials(event?: Event): { email: string; password: string } {
  const formElement = event?.currentTarget instanceof HTMLFormElement
    ? event.currentTarget
    : loginFormEl.value

  const emailInput = formElement?.elements.namedItem('username')
  const passwordInput = formElement?.elements.namedItem('password')

  const email = emailInput instanceof HTMLInputElement
    ? emailInput.value.trim()
    : form.value.email.trim()
  const password = passwordInput instanceof HTMLInputElement
    ? passwordInput.value
    : form.value.password

  form.value.email = email
  form.value.password = password
  return { email, password }
}

async function handleLogin(event?: Event) {
  const { email, password } = readCurrentLoginCredentials(event)

  if (!email || !password) {
    showWarning(t('auth.login.required'))
    return
  }

  const success = await authStore.login(email, password)
  if (success) {
    const redirectPath = sessionStorage.getItem('redirectPath')
    sessionStorage.removeItem('redirectPath')
    const targetPath = redirectPath && redirectPath !== '/'
      ? redirectPath
      : '/admin/dashboard'

    await navigateAfterLogin(router, targetPath)
    showSuccess(t('auth.login.successRedirecting'))
  } else {
    showError(authStore.error || t('auth.login.failed'))
  }
}
</script>
