<template>
  <div class="space-y-6 pb-8">
    <!-- 用户表格 -->
    <Card
      variant="default"
      class="overflow-hidden"
    >
      <UserManagementHeader
        :search-query="searchQuery"
        :filter-role="filterRole"
        :filter-group="filterGroup"
        :filter-status="filterStatus"
        :sort-option="sortOption"
        :user-groups="userGroups"
        :role-options="userRoleFilterOptions"
        :status-options="userStatusFilterOptions"
        :sort-options="userSortOptions"
        :loading="usersStore.loading"
        :can-operate-admin="authStore.canOperateAdmin"
        @update:search-query="searchQuery = $event"
        @update:filter-role="filterRole = $event"
        @update:filter-group="filterGroup = $event"
        @update:filter-status="filterStatus = $event"
        @update:sort-option="sortOption = $event"
        @open-groups="showUserGroupsDialog = true"
        @create-user="openCreateDialog"
        @refresh="handleManualRefresh"
      />

      <UserSelectionToolbar
        :is-all-filtered-selected="isAllFilteredSelected"
        :is-partially-filtered-selected="isPartiallyFilteredSelected"
        :filtered-user-count="filteredUserCount"
        :current-page-count="paginatedUsers.length"
        :selected-count="selectedCount"
        :is-current-page-fully-selected="isCurrentPageFullySelected"
        :can-clear-selection="canClearSelection"
        :select-all-filtered="selectAllFiltered"
        :loading="usersStore.loading"
        :can-operate-admin="authStore.canOperateAdmin"
        :group-count="userGroups.length"
        @toggle-select-filtered="toggleSelectFiltered"
        @toggle-select-current-page="toggleSelectCurrentPage"
        @clear-selection="clearSelection"
        @open-batch-dialog="openUserBatchDialog"
      />

      <UserManagementList
        :rows="userRows"
        :selected-id-set="selectedIdSet"
        :select-all-filtered="selectAllFiltered"
        :is-all-filtered-selected="isAllFilteredSelected"
        :is-partially-filtered-selected="isPartiallyFilteredSelected"
        :is-current-page-fully-selected="isCurrentPageFullySelected"
        :selection-disabled="selectAllFiltered || usersStore.loading"
        :loading="usersStore.loading"
        :can-operate-admin="authStore.canOperateAdmin"
        :has-filters="hasUserFilters"
        :sort-by="sortBy"
        :sort-order="sortOrder"
        @toggle-selected="toggleOne"
        @toggle-select-current-page="toggleSelectCurrentPage"
        @edit="editUser"
        @api-keys="manageApiKeys"
        @sessions="manageUserSessions"
        @toggle-status="toggleUserStatus"
        @delete="deleteUser"
        @sort="handleTableSort"
      />

      <!-- 分页控件 -->
      <Pagination
        :current="currentPage"
        :total="filteredUserCount"
        :page-size="pageSize"
        cache-key="users-page-size"
        @update:current="handlePageChange"
        @update:page-size="handlePageSizeChange"
      />
    </Card>

    <!-- 用户表单对话框（创建/编辑共用） -->
    <UserFormDialog
      ref="userFormDialogRef"
      :open="showUserFormDialog"
      :user="editingUser"
      :groups="userGroups"
      @close="closeUserFormDialog"
      @submit="handleUserFormSubmit"
    />

    <UserBatchActionDialog
      :open="showUserBatchDialog"
      :selected-ids="selectedIds"
      :select-all-filtered="selectAllFiltered"
      :selected-count="selectedCount"
      :filters="batchSelectionFilters"
      :groups="userGroups"
      @close="showUserBatchDialog = false"
      @completed="handleUserBatchCompleted"
    />

    <UserGroupsDialog
      :open="showUserGroupsDialog"
      :users-version="userOptionsVersion"
      @close="showUserGroupsDialog = false"
      @changed="handleUserGroupsChanged"
    />

    <UserApiKeysDialog
      :open="showApiKeysDialog"
      :api-keys="userApiKeys"
      :creating="creatingApiKey"
      :format-rate-limit="formatRateLimitSimple"
      :format-concurrent-limit="formatConcurrentLimitSimple"
      :format-ip-rules="formatIpRules"
      @close="closeApiKeysDialog"
      @create-key="openCreateUserApiKeyDialog"
      @edit-key="openEditUserApiKeyDialog"
      @toggle-lock="toggleLockApiKey"
      @delete-key="deleteApiKey"
      @copy-full-key="copyFullKey"
    />

    <UserApiKeyFormDialog
      :open="showUserApiKeyFormDialog"
      :form="userApiKeyForm"
      :is-editing="Boolean(editingUserApiKey)"
      :creating="creatingApiKey"
      @close="closeUserApiKeyFormDialog"
      @update:form="userApiKeyForm = $event"
      @submit="submitUserApiKeyForm"
    />

    <UserSessionsDialog
      :open="showUserSessionsDialog"
      :sessions="userSessions"
      :loading="loadingUserSessions"
      :action-loading="sessionDialogActionLoading"
      :format-date="formatDate"
      :format-session-meta="formatSessionMeta"
      @close="showUserSessionsDialog = false"
      @revoke-session="revokeSelectedUserSession"
      @revoke-all="revokeAllSelectedUserSessions"
    />

    <NewApiKeyDialog
      :open="showNewApiKeyDialog"
      :api-key="newApiKey"
      @close="closeNewApiKeyDialog"
      @copy="copyApiKey"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from 'vue'
import { useUsersStore } from '@/stores/users'
import { useAuthStore } from '@/stores/auth'
import {
  usersApi,
  type User,
  type ApiKey,
  type UserSession,
  type UserBatchActionResponse,
  type UserBatchSelectionFilters,
  type UserGroup,
  type AdminUserSortBy,
  type AdminUserSortOrder,
} from '@/api/users'
import { formatSessionMeta } from '@/types/session'
import { useToast } from '@/composables/useToast'
import { useConfirm } from '@/composables/useConfirm'
import { useClipboard } from '@/composables/useClipboard'
import { adminApi } from '@/api/admin'

// UI 组件
import {
  Card,
  Pagination,
} from '@/components/ui'

// 功能组件
import NewApiKeyDialog from '@/features/users/components/NewApiKeyDialog.vue'
import UserApiKeyFormDialog, { type UserApiKeyFormState } from '@/features/users/components/UserApiKeyFormDialog.vue'
import UserApiKeysDialog from '@/features/users/components/UserApiKeysDialog.vue'
import UserFormDialog, { type UserFormData } from '@/features/users/components/UserFormDialog.vue'
import UserBatchActionDialog from '@/features/users/components/UserBatchActionDialog.vue'
import UserGroupsDialog from '@/features/users/components/UserGroupsDialog.vue'
import UserManagementHeader from '@/features/users/components/UserManagementHeader.vue'
import UserManagementList from '@/features/users/components/UserManagementList.vue'
import UserSelectionToolbar from '@/features/users/components/UserSelectionToolbar.vue'
import UserSessionsDialog from '@/features/users/components/UserSessionsDialog.vue'
import type { UserManagementRow } from '@/features/users/components/user-management-types'
import {
  USER_ROLE_FILTER_OPTIONS,
  USER_SORT_OPTIONS,
  USER_STATUS_FILTER_OPTIONS,
  formatUserRoleLabel,
  userRoleBadgeVariant,
} from '@/features/users/components/user-management-config'
import { parseApiError } from '@/utils/errorParser'
import { formatTokens, formatRateLimitInheritable, formatRateLimitSimple, isRateLimitInherited, isRateLimitUnlimited } from '@/utils/format'
import { log } from '@/utils/logger'
import { useBatchSelection } from '@/composables/useBatchSelection'
import { useI18n } from '@/i18n'

const { success, error } = useToast()
const { confirmDanger } = useConfirm()
const { copyToClipboard } = useClipboard()
const { legacyT, locale } = useI18n()
const usersStore = useUsersStore()
const authStore = useAuthStore()

function localizedApiError(err: unknown, fallback: string): string {
  return legacyT(parseApiError(err, fallback))
}

// 用户表单对话框状态
const showUserFormDialog = ref(false)
const editingUser = ref<UserFormData | null>(null)
const userFormDialogRef = ref<InstanceType<typeof UserFormDialog>>()

// API Keys 对话框状态
const showApiKeysDialog = ref(false)
const showUserSessionsDialog = ref(false)
const showNewApiKeyDialog = ref(false)
const showUserApiKeyFormDialog = ref(false)
const selectedUser = ref<User | null>(null)
const userApiKeys = ref<ApiKey[]>([])
const userSessions = ref<UserSession[]>([])
const newApiKey = ref('')
const creatingApiKey = ref(false)
const loadingUserSessions = ref(false)
const sessionDialogActionLoading = ref<string | null>(null)
const editingUserApiKey = ref<ApiKey | null>(null)
const userApiKeyForm = ref<UserApiKeyFormState>({
  name: '',
  rate_limit: undefined,
  concurrent_limit: undefined,
  ip_rules_text: '',
})

const showUserBatchDialog = ref(false)
const showUserGroupsDialog = ref(false)
const userOptionsVersion = ref(0)

const searchQuery = ref('')
const filterRole = ref<'all' | User['role']>('all')
const filterStatus = ref<'all' | 'active' | 'inactive'>('all')
const filterGroup = ref('all')
const sortOption = ref<'default' | 'created_at_desc' | 'created_at_asc'>('default')
const userGroups = ref<UserGroup[]>([])
const userRoleFilterOptions = USER_ROLE_FILTER_OPTIONS
const userStatusFilterOptions = USER_STATUS_FILTER_OPTIONS
const userSortOptions = USER_SORT_OPTIONS
const sortBy = computed<AdminUserSortBy | null>(() =>
  sortOption.value === 'default' ? null : 'created_at'
)
const sortOrder = computed<AdminUserSortOrder>(() =>
  sortOption.value === 'created_at_asc' ? 'asc' : 'desc'
)

const currentPage = ref(1)
const pageSize = ref(20)
const USERS_PAGE_CACHE_TTL_MS = 10 * 1000
const USERS_SEARCH_DEBOUNCE_MS = 300
let userApiKeysRequestId = 0
let userApiKeyMutationRequestId = 0
let usersSearchDebounceTimer: ReturnType<typeof setTimeout> | null = null

const filteredUsers = computed(() => usersStore.users)

const paginatedUsers = computed(() => filteredUsers.value)

const filteredUserCount = computed(() => usersStore.total)
const {
  selectedIds,
  selectAllFiltered,
  selectedIdSet,
  selectedCount,
  isAllFilteredSelected,
  isPartiallyFilteredSelected,
  isCurrentPageFullySelected,
  canClearSelection,
  rememberItems: rememberBatchPageUsers,
  resetSelection: resetBatchSelection,
  toggleOne,
  toggleSelectFiltered,
  toggleSelectCurrentPage,
  clearSelection,
} = useBatchSelection<User>({
  pageItems: paginatedUsers,
  filteredTotal: filteredUserCount,
  getItemId: (user) => user.id,
})

const batchSelectionFilters = computed<UserBatchSelectionFilters>(() => {
  const filters: UserBatchSelectionFilters = {}
  const search = searchQuery.value.trim()
  if (search) filters.search = search
  if (filterRole.value === 'admin' || filterRole.value === 'audit_admin' || filterRole.value === 'user') filters.role = filterRole.value
  if (filterStatus.value === 'active') filters.is_active = true
  if (filterStatus.value === 'inactive') filters.is_active = false
  if (filterGroup.value !== 'all') filters.group_id = filterGroup.value
  return filters
})

const hasUserFilters = computed(() =>
  Boolean(searchQuery.value.trim())
  || filterRole.value !== 'all'
  || filterStatus.value !== 'all'
  || filterGroup.value !== 'all'
)

const userRows = computed<UserManagementRow[]>(() =>
  paginatedUsers.value.map((user) => {
    return {
      user,
      roleLabel: legacyT(formatUserRoleLabel(user.role)),
      roleBadgeVariant: userRoleBadgeVariant(user.role),
      isUnlimited: isUserUnlimited(user),
      requestCountLabel: formatNumber(user.request_count),
      tokensLabel: formatTokens(user.total_tokens ?? 0),
      rateLimitLabel: formatRateLimitInheritable(user.rate_limit),
      rateLimitSource: formatUserEffectiveRateLimitSource(user),
      rateLimitAsBadge: isRateLimitInherited(user.rate_limit) || isRateLimitUnlimited(user.rate_limit),
      createdAtLabel: formatDate(user.created_at),
      statusLabel: legacyT(user.is_active ? '活跃' : '禁用'),
      statusVariant: user.is_active ? 'success' : 'destructive',
    }
  })
)

function resetUserListForFilterChange() {
  currentPage.value = 1
  resetBatchSelection()
}

function clearUsersSearchDebounce() {
  if (usersSearchDebounceTimer) {
    clearTimeout(usersSearchDebounceTimer)
    usersSearchDebounceTimer = null
  }
}

watch(searchQuery, () => {
  resetUserListForFilterChange()
  clearUsersSearchDebounce()
  usersSearchDebounceTimer = setTimeout(() => {
    usersSearchDebounceTimer = null
    void refreshUsers()
  }, USERS_SEARCH_DEBOUNCE_MS)
})

watch([filterRole, filterStatus, filterGroup, sortOption], () => {
  resetUserListForFilterChange()
  clearUsersSearchDebounce()
  void refreshUsers()
})

watch(paginatedUsers, (users) => rememberBatchPageUsers(users), { immediate: true })

onMounted(() => {
  void refreshUsers({ preferCache: true })
  void loadUserGroups()
})

onBeforeUnmount(() => {
  clearUsersSearchDebounce()
  userApiKeysRequestId += 1
  userApiKeyMutationRequestId += 1
})

async function refreshUsers(options: { preferCache?: boolean } = {}) {
  const cacheTtlMs = options.preferCache ? USERS_PAGE_CACHE_TTL_MS : 0
  const search = searchQuery.value.trim()
  await usersStore.fetchUsers({
    cacheTtlMs,
    search: search || undefined,
    role: filterRole.value === 'all' ? undefined : filterRole.value,
    is_active: filterStatus.value === 'all' ? undefined : filterStatus.value === 'active',
    group_id: filterGroup.value === 'all' ? undefined : filterGroup.value,
    sort_by: sortBy.value ?? undefined,
    sort_order: sortBy.value ? sortOrder.value : undefined,
    skip: (currentPage.value - 1) * pageSize.value,
    limit: pageSize.value,
  })
}

async function handleManualRefresh() {
  clearUsersSearchDebounce()
  await Promise.all([
    refreshUsers(),
    loadUserGroups(),
  ])
}

function handleTableSort(payload: { key: string, direction: AdminUserSortOrder }): void {
  if (payload.key !== 'created_at') return
  sortOption.value = payload.direction === 'asc' ? 'created_at_asc' : 'created_at_desc'
}

function handlePageChange(page: number): void {
  currentPage.value = page
  void refreshUsers({ preferCache: true })
}

function handlePageSizeChange(size: number): void {
  pageSize.value = size
  currentPage.value = 1
  resetBatchSelection()
  void refreshUsers()
}

async function loadUserGroups(): Promise<void> {
  try {
    const response = await usersStore.listUserGroups()
    userGroups.value = response.items
    if (filterGroup.value !== 'all' && !userGroups.value.some((group) => group.id === filterGroup.value)) {
      filterGroup.value = 'all'
    }
  } catch (err) {
    log.error('加载用户分组失败:', err)
  }
}

async function handleUserGroupsChanged(): Promise<void> {
  await Promise.all([refreshUsers(), loadUserGroups()])
}

function openUserBatchDialog(): void {
  if (selectedCount.value === 0 && userGroups.value.length === 0) return
  showUserBatchDialog.value = true
}

async function handleUserBatchCompleted(_result: UserBatchActionResponse): Promise<void> {
  await refreshUsers()
  resetBatchSelection(true)
}

function invalidateUserOptions(): void {
  userOptionsVersion.value += 1
}

function formatDate(dateString: string) {
  return new Date(dateString).toLocaleDateString(locale.value)
}

function formatDateTime(value?: string | null): string {
  if (!value) return '-'
  return new Date(value).toLocaleString(locale.value, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function formatNumber(value?: number | null): string {
  const numericValue = typeof value === 'number' && Number.isFinite(value) ? value : 0
  return numericValue.toLocaleString()
}

function isUserUnlimited(user: User): boolean {
  return Boolean(user.unlimited)
}

function formatConcurrentLimitSimple(concurrentLimit?: number | null): string {
  if (concurrentLimit == null || concurrentLimit === 0) {
    return legacyT('不限并发')
  }
  return locale.value === 'en-US' ? `${concurrentLimit} concurrent` : `${concurrentLimit} 并发`
}

function formatIpRules(ipRules?: string[] | null): string {
  return ipRules && ipRules.length > 0 ? ipRules.join(', ') : legacyT('不限制')
}

function parseIpRulesInput(value: string): string[] | null {
  const items = value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean)
  return items.length > 0 ? items : null
}

function formatUserEffectiveRateLimitSource(user: User): string {
  const source = user.effective_policy?.rate_limit
  if (!source) return ''
  if (source.source === 'group' && source.group_name) {
    return `${legacyT('继承自分组：')}${source.group_name}`
  }
  if (source.source === 'combined') {
    const groupNames = Array.isArray(source.group_names) ? source.group_names.join(locale.value === 'en-US' ? ', ' : '、') : ''
    return groupNames ? `${legacyT('用户额外限制与分组叠加：')}${groupNames}` : legacyT('用户额外限制与分组叠加')
  }
  if (source.source === 'user') {
    return legacyT('用户单独配置')
  }
  return legacyT('系统默认')
}

async function toggleUserStatus(user: User) {
  const action = user.is_active ? '禁用' : '启用'
  const localizedAction = legacyT(action)
  const confirmed = await confirmDanger(
    locale.value === 'en-US'
      ? `${localizedAction} user ${user.username}?`
      : `确定要${action}用户 ${user.username} 吗？`,
    locale.value === 'en-US' ? `${localizedAction} user` : `${action}用户`,
    localizedAction
  )

  if (!confirmed) return

  try {
    await usersStore.updateUser(user.id, { is_active: !user.is_active })
    invalidateUserOptions()
    await refreshUsers()
    success(legacyT(`用户已${action}`))
  } catch (err: unknown) {
    error(localizedApiError(err, '未知错误'), legacyT(`${action}用户失败`))
  }
}

// ========== 用户表单对话框方法 ==========

function openCreateDialog() {
  editingUser.value = null
  showUserFormDialog.value = true
}

function editUser(user: User) {
  // 创建数组副本，避免与 store 数据共享引用
  editingUser.value = {
    id: user.id,
    username: user.username,
    email: user.email,
    unlimited: user.unlimited,
    role: user.role,
    is_active: user.is_active,
    group_ids: (user.groups || []).map(group => group.id),
    feature_settings: user.feature_settings ?? null,
  }
  showUserFormDialog.value = true
}

function closeUserFormDialog() {
  showUserFormDialog.value = false
  editingUser.value = null
}

async function handleUserFormSubmit(data: UserFormData & { password?: string; unlimited?: boolean }) {
  userFormDialogRef.value?.setSaving(true)
  try {
    if (data.id) {
      // 更新用户
      const updateData: Record<string, unknown> = {
        username: data.username,
        email: data.email || undefined,
        unlimited: data.unlimited,
        role: data.role,
        group_ids: data.group_ids ?? [],
        feature_settings: data.feature_settings ?? null,
      }
      if (data.password) {
        updateData.password = data.password
      }
      await usersStore.updateUser(data.id, updateData)
      invalidateUserOptions()
      success(legacyT('用户信息已更新'))
    } else {
      // 创建用户
      const newUser = await usersStore.createUser({
        username: data.username,
        password: data.password ?? '',
        email: data.email || undefined,
        unlimited: data.unlimited,
        role: data.role,
        group_ids: data.group_ids ?? [],
        feature_settings: data.feature_settings ?? null,
      })
      // 如果创建时指定为禁用，则更新状态
      if (data.is_active === false && newUser) {
        await usersStore.updateUser(newUser.id, { is_active: false })
      }
      invalidateUserOptions()
      success(legacyT('用户创建成功'))
    }
    closeUserFormDialog()
    await refreshUsers()
  } catch (err: unknown) {
    const title = data.id ? '更新用户失败' : '创建用户失败'
    error(localizedApiError(err, '未知错误'), legacyT(title))
  } finally {
    userFormDialogRef.value?.setSaving(false)
  }
}

async function manageApiKeys(user: User) {
  userApiKeyMutationRequestId += 1
  creatingApiKey.value = false
  selectedUser.value = user
  userApiKeys.value = []
  showApiKeysDialog.value = true
  await loadUserApiKeys(user.id)
}

function closeApiKeysDialog() {
  userApiKeyMutationRequestId += 1
  creatingApiKey.value = false
  showApiKeysDialog.value = false
  userApiKeys.value = []
  userApiKeysRequestId += 1
}

async function manageUserSessions(user: User) {
  selectedUser.value = user
  showUserSessionsDialog.value = true
  loadingUserSessions.value = true
  try {
    userSessions.value = await usersStore.getUserSessions(user.id)
  } catch (err) {
    error(localizedApiError(err, '加载用户设备会话失败'), legacyT('加载用户设备会话失败'))
  } finally {
    loadingUserSessions.value = false
  }
}

async function loadUserApiKeys(userId: string) {
  const requestId = ++userApiKeysRequestId
  try {
    const apiKeys = await usersStore.getUserApiKeys(userId)
    if (
      requestId !== userApiKeysRequestId
      || selectedUser.value?.id !== userId
      || !showApiKeysDialog.value
    ) return
    userApiKeys.value = apiKeys
  } catch (err) {
    if (
      requestId !== userApiKeysRequestId
      || selectedUser.value?.id !== userId
      || !showApiKeysDialog.value
    ) return
    log.error('加载API Keys失败:', err)
    userApiKeys.value = []
  }
}

function openCreateUserApiKeyDialog() {
  userApiKeyForm.value = {
    name: `Key-${new Date().toISOString().split('T')[0]}`,
    rate_limit: undefined,
    concurrent_limit: undefined,
    ip_rules_text: '',
  }
  editingUserApiKey.value = null
  showUserApiKeyFormDialog.value = true
}

function openEditUserApiKeyDialog(apiKey: ApiKey) {
  editingUserApiKey.value = apiKey
  userApiKeyForm.value = {
    name: apiKey.name || '',
    rate_limit: apiKey.rate_limit ?? undefined,
    concurrent_limit: apiKey.concurrent_limit ?? undefined,
    ip_rules_text: apiKey.ip_rules?.join(', ') ?? '',
  }
  showUserApiKeyFormDialog.value = true
}

function closeUserApiKeyFormDialog() {
  if (creatingApiKey.value) {
    userApiKeyMutationRequestId += 1
    creatingApiKey.value = false
  }
  showUserApiKeyFormDialog.value = false
  editingUserApiKey.value = null
  userApiKeyForm.value = {
    name: '',
    rate_limit: undefined,
    concurrent_limit: undefined,
    ip_rules_text: '',
  }
}

async function submitUserApiKeyForm() {
  if (!selectedUser.value) return
  if (!userApiKeyForm.value.name.trim()) {
    error(legacyT('请输入密钥名称'), legacyT(editingUserApiKey.value ? '更新 API Key 失败' : '创建 API Key 失败'))
    return
  }

  const targetUserId = selectedUser.value.id
  const editingApiKey = editingUserApiKey.value
  const form = { ...userApiKeyForm.value }
  const mutationRequestId = ++userApiKeyMutationRequestId
  const mutationIsCurrent = () => (
    mutationRequestId === userApiKeyMutationRequestId
    && selectedUser.value?.id === targetUserId
    && showApiKeysDialog.value
  )

  creatingApiKey.value = true
  try {
    const ipRules = parseIpRulesInput(form.ip_rules_text)
    if (editingApiKey) {
      await usersStore.updateApiKey(targetUserId, editingApiKey.id, {
        name: form.name,
        rate_limit: form.rate_limit ?? 0,
        concurrent_limit: form.concurrent_limit,
        ip_rules: ipRules,
      })
      if (!mutationIsCurrent()) return
      success(legacyT('API Key已更新'))
    } else {
      const response = await usersStore.createApiKey(targetUserId, {
        name: form.name,
        rate_limit: form.rate_limit ?? 0,
        concurrent_limit: form.concurrent_limit,
        ip_rules: ipRules,
      })
      if (!mutationIsCurrent()) return
      newApiKey.value = response.key || ''
      showNewApiKeyDialog.value = true
      success(legacyT('API Key创建成功'))
    }
    await loadUserApiKeys(targetUserId)
    if (!mutationIsCurrent()) return
    closeUserApiKeyFormDialog()
  } catch (err: unknown) {
    if (mutationIsCurrent()) {
      error(localizedApiError(err, '未知错误'), legacyT(editingApiKey ? '更新 API Key 失败' : '创建 API Key 失败'))
    }
  } finally {
    if (mutationRequestId === userApiKeyMutationRequestId) {
      creatingApiKey.value = false
    }
  }
}

async function revokeSelectedUserSession(sessionId: string) {
  if (!selectedUser.value) return
  sessionDialogActionLoading.value = sessionId
  try {
    await usersStore.revokeUserSession(selectedUser.value.id, sessionId)
    userSessions.value = userSessions.value.filter((session) => session.id !== sessionId)
    success(legacyT('设备已强制下线'))
  } catch (err) {
    error(localizedApiError(err, '强制下线失败'), legacyT('强制下线失败'))
  } finally {
    sessionDialogActionLoading.value = null
  }
}

async function revokeAllSelectedUserSessions() {
  if (!selectedUser.value) return
  sessionDialogActionLoading.value = 'all'
  try {
    const result = await usersStore.revokeAllUserSessions(selectedUser.value.id)
    userSessions.value = []
    success(result.revoked_count > 0
      ? legacyT(`已强制下线 ${result.revoked_count} 个设备`)
      : legacyT('没有可下线的设备'))
  } catch (err) {
    error(localizedApiError(err, '强制下线全部设备失败'), legacyT('强制下线全部设备失败'))
  } finally {
    sessionDialogActionLoading.value = null
  }
}

async function copyApiKey() {
  await copyToClipboard(newApiKey.value)
}

async function closeNewApiKeyDialog() {
  showNewApiKeyDialog.value = false
  newApiKey.value = ''
}

async function deleteApiKey(apiKey: ApiKey) {
  const confirmed = await confirmDanger(
    locale.value === 'en-US'
      ? `Delete this API key?\n\n${apiKey.key_display || '****'}\n\nThis action cannot be undone.`
      : `确定要删除这个API Key吗？\n\n${apiKey.key_display || '****'}\n\n此操作无法撤销。`,
    legacyT('删除 API Key')
  )

  if (!confirmed) return

  try {
    await usersStore.deleteApiKey(selectedUser.value.id, apiKey.id)
    await loadUserApiKeys(selectedUser.value.id)
    success(legacyT('API Key已删除'))
  } catch (err: unknown) {
    error(localizedApiError(err, '未知错误'), legacyT('删除 API Key 失败'))
  }
}

async function toggleLockApiKey(apiKey: ApiKey) {
  if (!selectedUser.value) return
  try {
    const response = await adminApi.toggleUserApiKeyLock(selectedUser.value.id, apiKey.id)
    // 更新本地状态
    const index = userApiKeys.value.findIndex(k => k.id === apiKey.id)
    if (index !== -1) {
      userApiKeys.value[index].is_locked = response.is_locked
    }
    success(legacyT(response.message))
  } catch (err: unknown) {
    log.error('切换密钥锁定状态失败:', err)
    error(localizedApiError(err, '操作失败'), legacyT('锁定/解锁失败'))
  }
}

async function copyFullKey(apiKey: ApiKey) {
  if (!selectedUser.value) return
  try {
    const response = await usersStore.getFullApiKey(selectedUser.value.id, apiKey.id)
    await copyToClipboard(response.key)
  } catch (err: unknown) {
    log.error('复制密钥失败:', err)
    error(localizedApiError(err, '未知错误'), legacyT('复制密钥失败'))
  }
}

async function deleteUser(user: User) {
  const confirmed = await confirmDanger(
    locale.value === 'en-US'
      ? `Delete user ${user.username}?\n\nThis will delete:\n- User account\n- All API keys\n- All usage records\n\nThis action cannot be undone.`
      : `确定要删除用户 ${user.username} 吗？\n\n此操作将删除：\n• 用户账户\n• 所有API密钥\n• 所有使用记录\n\n此操作无法撤销！`,
    legacyT('删除用户')
  )

  if (!confirmed) return

  try {
    await usersStore.deleteUser(user.id)
    invalidateUserOptions()
    if (usersStore.users.length === 0 && currentPage.value > 1) {
      currentPage.value -= 1
    }
    await refreshUsers()
    success(legacyT('用户已删除'))
  } catch (err: unknown) {
    error(localizedApiError(err, '未知错误'), legacyT('删除用户失败'))
  }
}
</script>
