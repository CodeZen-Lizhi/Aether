<template>
  <Card class="p-6">
    <div class="space-y-6">
      <div>
        <h3 class="text-lg font-medium text-foreground">
          管理员账号
        </h3>
        <p class="mt-1 text-sm text-muted-foreground">
          本系统为单用户模式，这里管理你自己的登录账号与密码。
        </p>
      </div>

      <!-- 基本信息 -->
      <form
        class="space-y-4"
        @submit.prevent="saveProfile"
      >
        <div class="grid gap-4 sm:grid-cols-2">
          <div>
            <Label for="account-username">登录用户名</Label>
            <Input
              id="account-username"
              v-model="profileForm.username"
              class="mt-1"
              autocomplete="username"
            />
          </div>
          <div>
            <Label for="account-email">邮箱（可选）</Label>
            <Input
              id="account-email"
              v-model="profileForm.email"
              type="email"
              class="mt-1"
              placeholder="admin@example.com"
            />
          </div>
        </div>
        <div class="flex items-center gap-3">
          <Button
            type="submit"
            :disabled="savingProfile || !hasProfileChanges"
          >
            {{ savingProfile ? '保存中...' : '保存账号信息' }}
          </Button>
          <span
            v-if="profileMessage"
            :class="profileError ? 'text-sm text-destructive' : 'text-sm text-emerald-600'"
          >
            {{ profileMessage }}
          </span>
        </div>
      </form>

      <div class="border-t border-border/60" />

      <!-- 修改密码 -->
      <form
        class="space-y-4"
        @submit.prevent="submitPassword"
      >
        <div class="flex items-center justify-between">
          <h3 class="text-base font-medium text-foreground">修改密码</h3>
          <Button
            type="submit"
            :disabled="changingPassword || !hasPasswordChanges"
            variant="secondary"
          >
            {{ changingPassword ? '保存中...' : '更新密码' }}
          </Button>
        </div>
        <div class="grid gap-4 sm:grid-cols-3">
          <div>
            <Label for="account-old-password">当前密码</Label>
            <Input
              id="account-old-password"
              v-model="passwordForm.old_password"
              type="password"
              autocomplete="current-password"
              class="mt-1"
            />
          </div>
          <div>
            <Label for="account-new-password">新密码</Label>
            <Input
              id="account-new-password"
              v-model="passwordForm.new_password"
              type="password"
              autocomplete="new-password"
              class="mt-1"
            />
          </div>
          <div>
            <Label for="account-confirm-password">确认新密码</Label>
            <Input
              id="account-confirm-password"
              v-model="passwordForm.confirm_password"
              type="password"
              autocomplete="new-password"
              class="mt-1"
            />
            <p
              v-if="passwordForm.confirm_password && passwordForm.new_password !== passwordForm.confirm_password"
              class="mt-1 text-xs text-destructive"
            >
              两次输入的密码不一致
            </p>
          </div>
        </div>
        <p
          v-if="passwordMessage"
          :class="passwordError ? 'text-sm text-destructive' : 'text-sm text-emerald-600'"
        >
          {{ passwordMessage }}
        </p>
      </form>
    </div>
  </Card>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { Card, Button, Input, Label } from '@/components/ui'
import { meApi } from '@/api/me'
import { log } from '@/utils/logger'

const profileForm = reactive({ username: '', email: '' })
const passwordForm = reactive({ old_password: '', new_password: '', confirm_password: '' })

const savingProfile = ref(false)
const changingPassword = ref(false)
const profileMessage = ref('')
const profileError = ref(false)
const passwordMessage = ref('')
const passwordError = ref(false)
const hasPassword = ref(true)

const hasProfileChanges = computed(
  () => profileForm.username.trim().length > 0,
)
const hasPasswordChanges = computed(
  () => passwordForm.new_password.length > 0
    && passwordForm.new_password === passwordForm.confirm_password,
)

async function loadProfile() {
  try {
    const profile = await meApi.getProfile()
    profileForm.username = profile.username
    profileForm.email = profile.email ?? ''
    hasPassword.value = profile.has_password
  } catch (error) {
    log.error('加载管理员账号信息失败:', error)
  }
}

async function saveProfile() {
  profileMessage.value = ''
  profileError.value = false
  savingProfile.value = true
  try {
    const payload: { username?: string; email?: string } = {}
    if (profileForm.username.trim()) {
      payload.username = profileForm.username.trim()
    }
    payload.email = profileForm.email.trim() || undefined
    const result = await meApi.updateProfile(payload)
    profileMessage.value = result.message || '账号信息已更新'
  } catch (error) {
    profileError.value = true
    profileMessage.value = error instanceof Error ? error.message : '账号信息保存失败'
  } finally {
    savingProfile.value = false
  }
}

async function submitPassword() {
  passwordMessage.value = ''
  passwordError.value = false
  if (passwordForm.new_password !== passwordForm.confirm_password) {
    passwordError.value = true
    passwordMessage.value = '两次输入的密码不一致'
    return
  }
  changingPassword.value = true
  try {
    const result = await meApi.changePassword({
      old_password: passwordForm.old_password || undefined,
      new_password: passwordForm.new_password,
    })
    passwordMessage.value = result.message || '密码已更新'
    passwordForm.old_password = ''
    passwordForm.new_password = ''
    passwordForm.confirm_password = ''
  } catch (error) {
    passwordError.value = true
    passwordMessage.value = error instanceof Error ? error.message : '密码更新失败'
  } finally {
    changingPassword.value = false
  }
}

onMounted(loadProfile)
</script>
