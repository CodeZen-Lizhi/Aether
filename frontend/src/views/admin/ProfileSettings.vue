<template>
  <div class="container mx-auto px-4 py-8">
    <h2 class="text-2xl font-bold text-foreground mb-6">
      账号设置
    </h2>

    <div class="max-w-3xl space-y-6">
      <!-- 基本信息与密码：一个表单一起保存 -->
      <Card
        v-if="!desktopMode"
        class="p-6"
      >
        <form
          class="space-y-4"
          @submit.prevent="saveAccount"
        >
          <div class="flex items-center justify-between">
            <h3 class="text-lg font-medium text-foreground">
              基本信息
            </h3>
            <Button
              type="submit"
              :disabled="saving || !hasChanges"
              class="shadow-none hover:shadow-none"
            >
              {{ saving ? '保存中...' : '保存' }}
            </Button>
          </div>

          <div>
            <Label for="username">用户名</Label>
            <Input
              id="username"
              v-model="profileForm.username"
              class="mt-1"
            />
          </div>

          <!-- 密码（LDAP 用户不显示） -->
          <template v-if="profile?.auth_source !== 'ldap'">
            <div class="space-y-4 border-t border-border/60 pt-4">
              <h3 class="text-base font-medium text-foreground">
                {{ profile?.has_password ? '修改密码' : '设置密码' }}
              </h3>
              <div v-if="profile?.has_password">
                <Label for="old-password">当前密码</Label>
                <Input
                  id="old-password"
                  v-model="passwordForm.old_password"
                  type="text"
                  masked
                  class="mt-1"
                />
              </div>
              <div>
                <Label for="new-password">{{ profile?.has_password ? '新密码' : '密码' }}</Label>
                <Input
                  id="new-password"
                  v-model="passwordForm.new_password"
                  type="text"
                  masked
                  :placeholder="getPasswordPolicyPlaceholder(passwordPolicyLevel)"
                  class="mt-1"
                />
                <p
                  v-if="passwordError"
                  class="mt-1 text-xs text-destructive"
                >
                  {{ passwordError }}
                </p>
                <p
                  v-else
                  class="mt-1 text-xs text-muted-foreground"
                >
                  {{ passwordPolicyHint }}
                </p>
              </div>
              <div>
                <Label for="confirm-password">确认{{ profile?.has_password ? '新' : '' }}密码</Label>
                <Input
                  id="confirm-password"
                  v-model="passwordForm.confirm_password"
                  type="text"
                  masked
                  placeholder="再次输入密码"
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
          </template>
        </form>
      </Card>

      <Card
        v-if="!desktopMode"
        class="p-6"
      >
        <div class="flex items-center justify-between mb-4">
          <div>
            <h3 class="text-lg font-medium text-foreground">
              登录设备
            </h3>
            <p class="text-sm text-muted-foreground mt-1">
              管理当前账号在各设备上的登录状态
            </p>
          </div>
          <Button
            variant="outline"
            :disabled="sessionsLoading || otherSessionCount === 0 || sessionActionLoading === 'others'"
            @click="handleRevokeOtherSessions"
          >
            {{ sessionActionLoading === 'others' ? '处理中...' : '退出其他设备' }}
          </Button>
        </div>

        <div
          v-if="sessionsLoading"
          class="text-sm text-muted-foreground"
        >
          正在加载设备列表...
        </div>
        <div
          v-else-if="userSessions.length === 0"
          class="text-sm text-muted-foreground"
        >
          暂无登录设备记录
        </div>
        <div
          v-else
          class="space-y-3"
        >
          <div
            v-for="session in userSessions"
            :key="session.id"
            class="flex items-start justify-between gap-4 rounded-lg border border-border/60 bg-muted/20 p-4"
          >
            <div class="min-w-0">
              <div class="flex items-center gap-2 flex-wrap">
                <template v-if="editingSessionId === session.id">
                  <Input
                    v-model="sessionLabelDraft"
                    size="sm"
                    class="h-8 w-56"
                    maxlength="120"
                    @keyup.enter="saveSessionLabel(session.id)"
                  />
                </template>
                <span
                  v-else
                  class="font-medium text-foreground"
                >{{ session.device_label }}</span>
                <Badge
                  v-if="session.is_current"
                  variant="secondary"
                >
                  当前设备
                </Badge>
              </div>
              <p class="mt-1 text-xs text-muted-foreground">
                {{ formatSessionMeta(session) }}
              </p>
              <p class="mt-1 text-xs text-muted-foreground">
                最近活跃 {{ formatDate(session.last_seen_at || session.created_at) }}
                <span v-if="session.ip_address"> · IP {{ session.ip_address }}</span>
              </p>
            </div>
            <div class="flex items-center gap-2">
              <template v-if="editingSessionId === session.id">
                <Button
                  size="sm"
                  :disabled="sessionActionLoading === session.id || !sessionLabelDraft.trim()"
                  @click="saveSessionLabel(session.id)"
                >
                  {{ sessionActionLoading === session.id ? '保存中...' : '保存' }}
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  :disabled="sessionActionLoading === session.id"
                  @click="cancelSessionLabelEdit"
                >
                  取消
                </Button>
              </template>
              <template v-else>
                <Button
                  variant="outline"
                  size="sm"
                  :disabled="sessionActionLoading !== null"
                  @click="startSessionLabelEdit(session)"
                >
                  重命名
                </Button>
                <Button
                  v-if="!session.is_current"
                  variant="outline"
                  size="sm"
                  :disabled="sessionActionLoading === session.id"
                  @click="handleRevokeSession(session.id)"
                >
                  {{ sessionActionLoading === session.id ? '处理中...' : '退出' }}
                </Button>
              </template>
            </div>
          </div>
        </div>
      </Card>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { meApi, type Profile } from '@/api/me'
import { type UserSession, formatSessionMeta } from '@/types/session'
import {
  getPasswordPolicyHint,
  getPasswordPolicyPlaceholder,
  validatePasswordByPolicy,
  type PasswordPolicyLevel,
} from '@/utils/passwordPolicy'
import Card from '@/components/ui/card.vue'
import Button from '@/components/ui/button.vue'
import Badge from '@/components/ui/badge.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import { useToast } from '@/composables/useToast'
import { log } from '@/utils/logger'
import { getErrorMessage } from '@/types/api-error'
import { hasDesktopSession } from '@/desktop/session'

const authStore = useAuthStore()
const router = useRouter()
const { success, error: showError } = useToast()
const desktopMode = hasDesktopSession()

const profile = ref<Profile | null>(null)
const userSessions = ref<UserSession[]>([])

const profileForm = ref({
  username: ''
})

const passwordForm = ref({
  old_password: '',
  new_password: '',
  confirm_password: ''
})

const saving = ref(false)
const sessionsLoading = ref(false)
const sessionActionLoading = ref<string | null>(null)
const editingSessionId = ref<string | null>(null)
const sessionLabelDraft = ref('')
const passwordPolicyLevel = ref<PasswordPolicyLevel>('weak')

// 原始用户名，用于检测是否有修改
const originalUsername = ref('')

const usernameChanged = computed(() => profileForm.value.username !== originalUsername.value)

const passwordTouched = computed(() =>
  !!(passwordForm.value.old_password || passwordForm.value.new_password || passwordForm.value.confirm_password)
)

const hasChanges = computed(() => usernameChanged.value || passwordTouched.value)

const passwordPolicyHint = computed(() => getPasswordPolicyHint(passwordPolicyLevel.value))
const passwordError = computed(() =>
  validatePasswordByPolicy(passwordForm.value.new_password, passwordPolicyLevel.value)
)

const otherSessionCount = computed(() => userSessions.value.filter((session) => !session.is_current).length)

onMounted(async () => {
  if (desktopMode) return
  await Promise.all([
    loadProfile(),
    loadSessions(),
  ])
})

async function loadProfile() {
  try {
    profile.value = await meApi.getProfile()
    profileForm.value = { username: profile.value.username }
    originalUsername.value = profile.value.username
  } catch (error) {
    log.error('加载个人信息失败:', error)
    showError('加载个人信息失败')
  }
}

async function loadSessions() {
  sessionsLoading.value = true
  try {
    userSessions.value = await meApi.listSessions()
    if (editingSessionId.value) {
      const currentEditing = userSessions.value.find((session) => session.id === editingSessionId.value)
      if (!currentEditing) {
        cancelSessionLabelEdit()
      }
    }
  } catch (error) {
    log.error('加载登录设备失败:', error)
  } finally {
    sessionsLoading.value = false
  }
}

async function saveAccount() {
  if (desktopMode) return
  const hasPassword = profile.value?.has_password ?? false
  const wantsPassword = hasPassword
    ? !!(passwordForm.value.old_password && passwordForm.value.new_password && passwordForm.value.confirm_password)
    : !!(passwordForm.value.new_password && passwordForm.value.confirm_password)

  if (passwordTouched.value && !wantsPassword) {
    showError('密码字段未填写完整', '保存失败')
    return
  }
  if (passwordForm.value.new_password !== passwordForm.value.confirm_password) {
    showError('两次输入的密码不一致', '保存失败')
    return
  }
  if (wantsPassword && passwordError.value) {
    showError(passwordError.value, '保存失败')
    return
  }

  saving.value = true
  try {
    if (usernameChanged.value) {
      await meApi.updateProfile({ username: profileForm.value.username })
      originalUsername.value = profileForm.value.username
      authStore.fetchCurrentUser()
    }

    if (wantsPassword) {
      try {
        await meApi.changePassword({
          old_password: hasPassword ? passwordForm.value.old_password : undefined,
          new_password: passwordForm.value.new_password
        })
      } catch (err) {
        log.error('修改密码失败:', err)
        const title = hasPassword ? '密码修改失败' : '密码设置失败'
        const defaultMsg = hasPassword ? '请检查当前密码是否正确' : '请稍后重试'
        showError(getErrorMessage(err, defaultMsg), title)
        return
      }
      success('保存成功，请重新登录')
      await authStore.logout()
      await router.replace('/')
      return
    }

    success('个人信息已更新')
  } catch (err) {
    log.error('更新个人信息失败:', err)
    showError(getErrorMessage(err), '更新个人信息失败')
  } finally {
    saving.value = false
  }
}

function startSessionLabelEdit(session: UserSession) {
  editingSessionId.value = session.id
  sessionLabelDraft.value = session.device_label
}

function cancelSessionLabelEdit() {
  editingSessionId.value = null
  sessionLabelDraft.value = ''
}

async function saveSessionLabel(sessionId: string) {
  const nextLabel = sessionLabelDraft.value.trim()
  if (!nextLabel) {
    showError('设备名称不能为空')
    return
  }

  sessionActionLoading.value = sessionId
  try {
    const updated = await meApi.updateSessionLabel(sessionId, nextLabel)
    userSessions.value = userSessions.value.map((session) =>
      session.id === sessionId ? updated : session
    )
    cancelSessionLabelEdit()
    success('设备名称已更新')
  } catch (error) {
    log.error('更新设备名称失败:', error)
    showError(getErrorMessage(error, '更新设备名称失败'))
  } finally {
    sessionActionLoading.value = null
  }
}

async function handleRevokeSession(sessionId: string) {
  sessionActionLoading.value = sessionId
  try {
    await meApi.revokeSession(sessionId)
    if (editingSessionId.value === sessionId) {
      cancelSessionLabelEdit()
    }
    success('设备已退出登录')
    await loadSessions()
  } catch (error) {
    log.error('退出设备失败:', error)
    showError(getErrorMessage(error, '退出设备失败'))
  } finally {
    sessionActionLoading.value = null
  }
}

async function handleRevokeOtherSessions() {
  sessionActionLoading.value = 'others'
  try {
    const result = await meApi.revokeOtherSessions()
    success(result.revoked_count > 0 ? `已退出 ${result.revoked_count} 个其他设备` : '没有其他在线设备')
    await loadSessions()
  } catch (error) {
    log.error('退出其他设备失败:', error)
    showError(getErrorMessage(error, '退出其他设备失败'))
  } finally {
    sessionActionLoading.value = null
  }
}

function formatDate(dateString?: string): string {
  if (!dateString) return '未知'
  return new Date(dateString).toLocaleDateString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  })
}
</script>

<style scoped>
</style>
