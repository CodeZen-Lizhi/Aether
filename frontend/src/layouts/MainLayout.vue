<template>
  <AppShell
    :show-notice="showAuthError"
    :main-class="mainClasses"
    :sidebar-class="sidebarClasses"
    :content-class="contentClasses"
  >
    <template #notice>
      <div class="flex w-full max-w-3xl items-center justify-between rounded-3xl bg-orange-500 px-6 py-3 text-white shadow-2xl ring-1 ring-white/30">
        <div class="flex items-center gap-3">
          <AlertTriangle class="h-5 w-5" />
          <span>{{ t('auth.expired') }}</span>
        </div>
        <Button
          variant="outline"
          size="sm"
          class="border-white/60 text-white hover:bg-white/10"
          @click="handleRelogin"
        >
          {{ t('auth.relogin') }}
        </Button>
      </div>
    </template>

    <template #sidebar>
      <div class="flex h-full w-full min-w-0 flex-col overflow-hidden">
        <!-- HEADER (Brand) -->
        <div
          class="group/sidebar-brand relative flex shrink-0 items-center transition-[height,padding] [transition-duration:240ms] [transition-timing-function:cubic-bezier(0.4,0,0.2,1)] motion-reduce:transition-none"
          :class="sidebarCollapsed ? 'h-16 px-4' : 'h-20 px-6'"
        >
          <Transition
            name="sidebar-mode"
            mode="out-in"
          >
            <RouterLink
              v-if="!sidebarCollapsed"
              key="expanded-brand"
              to="/"
              class="group flex w-full min-w-0 items-center gap-3 pr-10 transition-opacity hover:opacity-80"
            >
              <HeaderLogo
                size="h-9 w-9"
                class-name="shrink-0 text-[#191919] dark:text-white"
              />
              <div class="flex min-w-0 flex-col justify-center">
                <h1 class="truncate text-lg font-bold leading-none text-[#191919] dark:text-white">
                  {{ siteName }}
                </h1>
                <span class="mt-1.5 truncate text-[10px] font-medium leading-none tracking-wide text-[#91918d] dark:text-muted-foreground">{{ siteSubtitle }}</span>
              </div>
            </RouterLink>

            <div
              v-else
              key="collapsed-brand"
              aria-hidden="true"
              class="flex h-8 w-8 transform-gpu items-center justify-center transition-[opacity,transform] duration-200 ease-out will-change-[opacity,transform] group-hover/sidebar-brand:scale-90 group-hover/sidebar-brand:opacity-0 motion-reduce:transition-none"
            >
              <HeaderLogo
                size="h-8 w-8"
                class-name="shrink-0 text-[#191919] dark:text-white"
              />
            </div>
          </Transition>

          <button
            type="button"
            class="absolute top-1/2 z-10 flex h-8 w-8 shrink-0 -translate-y-1/2 transform-gpu items-center justify-center rounded-md text-muted-foreground transition-[right,color,background-color,opacity,transform] [transition-duration:240ms] [transition-timing-function:cubic-bezier(0.4,0,0.2,1)] hover:bg-muted/50 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary motion-reduce:transition-none"
            :class="sidebarCollapsed ? 'right-[15px] scale-90 opacity-0 will-change-[right,opacity,transform] group-hover/sidebar-brand:scale-100 group-hover/sidebar-brand:opacity-100 focus-visible:scale-100 focus-visible:bg-[#ffffff] focus-visible:opacity-100 dark:focus-visible:bg-[#0b1220]' : 'right-3 opacity-100'"
            :aria-label="sidebarCollapsed ? t('nav.expandSidebar') : t('nav.collapseSidebar')"
            :aria-expanded="!sidebarCollapsed"
            :title="sidebarCollapsed ? t('nav.expandSidebar') : t('nav.collapseSidebar')"
            @click="sidebarCollapsed = !sidebarCollapsed"
          >
            <Transition
              name="sidebar-icon"
              mode="out-in"
            >
              <PanelLeftOpen
                v-if="sidebarCollapsed"
                key="open"
                class="h-4 w-4"
              />
              <PanelLeftClose
                v-else
                key="close"
                class="h-4 w-4"
              />
            </Transition>
          </button>
        </div>

        <!-- NAVIGATION -->
        <div
          class="flex-1 overflow-y-auto transition-[padding] [transition-duration:240ms] [transition-timing-function:cubic-bezier(0.4,0,0.2,1)] scrollbar-none motion-reduce:transition-none"
          :class="sidebarCollapsed ? 'pb-2 pt-0' : 'py-2'"
        >
          <Transition
            name="sidebar-mode"
            mode="out-in"
          >
            <div
              :key="sidebarCollapsed ? 'collapsed-nav' : 'expanded-nav'"
              class="w-full"
              :class="sidebarCollapsed ? 'max-w-16' : ''"
            >
              <SidebarNav
                :items="navigation"
                :is-active="isNavActive"
                :collapsed="sidebarCollapsed"
                @prefetch="prefetchNavigationItem"
              />
            </div>
          </Transition>
        </div>

        <!-- FOOTER (Profile) -->
        <Transition
          name="sidebar-mode"
          mode="out-in"
        >
          <div
            :key="sidebarCollapsed ? 'collapsed-footer' : 'expanded-footer'"
            class="border-t border-[#0f172a]/5 dark:border-white/5"
            :class="sidebarCollapsed ? 'max-w-16 p-2' : 'p-4'"
          >
            <div
              class="flex items-center"
              :class="sidebarCollapsed ? 'flex-col gap-2 rounded-lg p-1' : 'justify-between rounded-xl p-2'"
            >
              <div
                class="flex min-w-0 items-center"
                :class="sidebarCollapsed ? 'justify-center' : 'gap-3'"
              >
                <div
                  class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-black/5 bg-[#f0f0eb] text-xs font-bold text-[#0f172a] dark:bg-white/10 dark:text-[#93c5fd]"
                  :title="sidebarCollapsed ? authStore.user?.username : undefined"
                >
                  {{ authStore.user?.username?.substring(0, 2).toUpperCase() }}
                </div>
                <div
                  v-if="!sidebarCollapsed"
                  class="flex min-w-0 flex-col"
                >
                  <span class="truncate text-xs font-semibold leading-none text-foreground opacity-90">{{ authStore.user?.username }}</span>
                  <span class="mt-1.5 text-[10px] leading-none text-muted-foreground opacity-50">{{ currentRoleLabel }}</span>
                </div>
              </div>

              <div
                class="flex items-center gap-1"
                :class="sidebarCollapsed ? 'flex-col' : ''"
              >
                <RouterLink
                  to="/admin/settings"
                  class="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-muted/50 hover:text-foreground"
                  :aria-label="sidebarCollapsed ? t('common.settings') : undefined"
                  :title="t('common.settings')"
                >
                  <Settings class="h-4 w-4" />
                </RouterLink>
                <button
                  class="rounded-md p-1.5 text-muted-foreground transition-colors hover:text-red-500"
                  :aria-label="sidebarCollapsed ? t('common.logout') : undefined"
                  :title="t('common.logout')"
                  @click="handleLogout"
                >
                  <LogOut class="h-4 w-4" />
                </button>
              </div>
            </div>
          </div>
        </Transition>
      </div>
    </template>

    <template #header>
      <!-- Mobile Header -->
      <header class="lg:hidden fixed top-0 left-0 right-0 z-50 border-b border-[var(--shell-border)] bg-[var(--shell-glass)] backdrop-blur-xl transition-all">
        <div class="mx-auto max-w-7xl px-6 py-4">
          <div class="flex items-center justify-between">
            <RouterLink
              to="/"
              class="flex items-center gap-3 group"
            >
              <HeaderLogo
                size="h-9 w-9"
                class-name="text-[#191919] dark:text-white"
              />
              <div class="flex flex-col justify-center">
                <h1 class="text-lg font-bold text-[#191919] dark:text-white leading-none">
                  {{ siteName }}
                </h1>
                <span class="text-[10px] text-[#91918d] dark:text-muted-foreground leading-none mt-1.5 font-medium tracking-wide">{{ siteSubtitle }}</span>
              </div>
            </RouterLink>

            <div class="flex items-center gap-3">
              <LanguageSwitcher />
              <ThemeModeButton />
              <button
                class="flex h-9 w-9 items-center justify-center rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted/50 transition"
                @click="mobileMenuOpen = !mobileMenuOpen"
              >
                <div class="relative w-5 h-5">
                  <Transition
                    enter-active-class="transition-all duration-200 ease-out"
                    enter-from-class="opacity-0 rotate-90 scale-75"
                    enter-to-class="opacity-100 rotate-0 scale-100"
                    leave-active-class="transition-all duration-150 ease-in absolute inset-0"
                    leave-from-class="opacity-100 rotate-0 scale-75"
                    leave-to-class="opacity-0 -rotate-90 scale-75"
                    mode="out-in"
                  >
                    <Menu
                      v-if="!mobileMenuOpen"
                      class="h-5 w-5"
                    />
                    <X
                      v-else
                      class="h-5 w-5"
                    />
                  </Transition>
                </div>
              </button>
            </div>
          </div>
        </div>

        <!-- Mobile Dropdown Menu -->
        <Transition
          enter-active-class="transition-all duration-300 ease-out"
          enter-from-class="opacity-0 -translate-y-2"
          enter-to-class="opacity-100 translate-y-0"
          leave-active-class="transition-all duration-200 ease-in"
          leave-from-class="opacity-0 translate-y-0"
          leave-to-class="opacity-0 -translate-y-2"
        >
          <div
            v-if="mobileMenuOpen"
            class="absolute inset-x-0 top-full max-h-[calc(100dvh-73px)] overflow-y-auto overscroll-contain border-t border-[var(--shell-border)] bg-background shadow-xl [-webkit-overflow-scrolling:touch] touch-pan-y"
          >
            <div class="mx-auto max-w-7xl px-6 py-4 pb-28">
              <div class="space-y-4">
                <div
                  v-for="group in navigation"
                  :key="group.title"
                >
                  <div
                    v-if="group.title"
                    class="text-[10px] font-semibold text-[#91918d] dark:text-muted-foreground uppercase tracking-wider mb-2"
                  >
                    {{ group.title }}
                  </div>
                  <div class="grid grid-cols-2 gap-2">
                    <RouterLink
                      v-for="item in group.items"
                      :key="item.href"
                      :to="item.href"
                      class="flex items-center gap-2.5 px-3 py-2.5 rounded-xl text-sm font-medium transition-all"
                      :class="isNavActive(item.href)
                        ? 'bg-[#cc785c]/10 dark:bg-[#cc785c]/20 text-[#cc785c] dark:text-[#93c5fd]'
                        : 'text-[#666663] dark:text-muted-foreground hover:bg-black/5 dark:hover:bg-white/5 hover:text-[#191919] dark:hover:text-white'"
                      @pointerenter="prefetchNavigationItem(item.href)"
                      @pointerdown="prefetchNavigationItem(item.href)"
                      @focus="prefetchNavigationItem(item.href)"
                      @click="mobileMenuOpen = false"
                    >
                      <component
                        :is="item.icon"
                        class="h-4 w-4 shrink-0"
                      />
                      <span class="truncate">{{ item.name }}</span>
                    </RouterLink>
                  </div>
                </div>
              </div>

              <!-- User Section -->
              <div class="mt-4 pt-4 border-t border-[#cc785c]/10 dark:border-[rgba(227,224,211,0.12)]">
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-3 min-w-0">
                    <div class="w-8 h-8 rounded-full bg-[#f0f0eb] dark:bg-white/10 border border-black/5 flex items-center justify-center text-xs font-bold text-[#0f172a] dark:text-[#93c5fd] shrink-0">
                      {{ authStore.user?.username?.substring(0, 2).toUpperCase() }}
                    </div>
                    <div class="flex flex-col min-w-0">
                      <span class="text-sm font-semibold leading-none truncate text-[#191919] dark:text-white">{{ authStore.user?.username }}</span>
                      <span class="text-[10px] text-[#91918d] dark:text-muted-foreground leading-none mt-1">{{ currentRoleLabel }}</span>
                    </div>
                  </div>
                  <div class="flex items-center gap-1">
                    <RouterLink
                      to="/admin/settings"
                      class="p-2 hover:bg-muted/50 rounded-lg text-muted-foreground hover:text-foreground transition-colors"
                      :title="t('common.settings')"
                      @click="mobileMenuOpen = false"
                    >
                      <Settings class="w-4 h-4" />
                    </RouterLink>
                    <button
                      class="p-2 rounded-lg text-muted-foreground hover:text-red-500 transition-colors"
                      :title="t('common.logout')"
                      @click="handleLogout"
                    >
                      <LogOut class="w-4 h-4" />
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </Transition>
      </header>

      <!-- Desktop Page Header -->
      <header class="hidden lg:flex h-16 px-8 items-center justify-between shrink-0 border-b border-[#0f172a]/5 dark:border-white/5 sticky top-0 z-40 backdrop-blur-md bg-[#ffffff]/90 dark:bg-[#0b1220]/90">
        <div class="flex flex-col gap-0.5">
          <div class="flex items-center gap-2 text-sm text-muted-foreground">
            <template
              v-for="(crumb, index) in breadcrumbs"
              :key="index"
            >
              <template v-if="index > 0">
                <ChevronRight class="w-3 h-3 opacity-50" />
              </template>
              <RouterLink
                v-if="crumb.href && index < breadcrumbs.length - 1"
                :to="crumb.href"
                class="hover:text-foreground transition-colors"
              >
                {{ crumb.label }}
              </RouterLink>
              <span
                v-else
                :class="index === breadcrumbs.length - 1 ? 'text-foreground font-medium' : ''"
              >
                {{ crumb.label }}
              </span>
            </template>
            <!-- 页面级操作插入点 -->
            <div id="breadcrumb-actions" />
          </div>
        </div>

        <div class="flex items-center gap-2">
          <!-- Page-level header actions (right side) -->
          <div
            id="header-actions-right"
            class="flex items-center"
          />
          <LanguageSwitcher />
          <ThemeModeButton />
          <a
            href="https://github.com/fawney19/Aether"
            target="_blank"
            rel="noopener noreferrer"
            class="flex h-9 w-9 items-center justify-center rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted/50 transition"
            :title="t('common.githubRepository')"
          >
            <GithubIcon class="h-4 w-4" />
          </a>
        </div>
      </header>
    </template>

    <RouterView />
  </AppShell>
</template>

<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted } from 'vue'
import { useLocalStorage } from '@vueuse/core'
import { useRoute, useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { useSiteInfo } from '@/composables/useSiteInfo'
import Button from '@/components/ui/button.vue'
import AppShell from '@/components/layout/AppShell.vue'
import SidebarNav from '@/components/layout/SidebarNav.vue'
import HeaderLogo from '@/components/HeaderLogo.vue'
import LanguageSwitcher from '@/components/common/LanguageSwitcher.vue'
import ThemeModeButton from '@/components/common/ThemeModeButton.vue'
import {
  Settings,
  AlertTriangle,
  LogOut,
  ChevronRight,
  Menu,
  X,
  PanelLeftClose,
  PanelLeftOpen,
} from 'lucide-vue-next'

import GithubIcon from '@/components/icons/GithubIcon.vue'
import { prefetchNavigationTarget } from '@/utils/adminNavigationPrefetch'
import { useI18n } from '@/i18n'
import { buildBreadcrumbs, buildNavigation } from './main-layout/navigation'

const router = useRouter()
const route = useRoute()
const authStore = useAuthStore()
const { siteName, siteSubtitle } = useSiteInfo()
const { t } = useI18n()

const showAuthError = ref(false)
const mobileMenuOpen = ref(false)
const sidebarCollapsed = useLocalStorage('aether-sidebar-collapsed', false)

function syncAuthNotice() {
  authStore.syncToken()
  showAuthError.value = !!authStore.user && !authStore.token
}

function handleStorageChange(event: StorageEvent) {
  if (event.key === null || event.key === 'access_token') {
    syncAuthNotice()
  }
}

function handleVisibilityChange() {
  if (!document.hidden) {
    syncAuthNotice()
  }
}

watch(
  () => [authStore.user, authStore.token] as const,
  () => {
    showAuthError.value = !!authStore.user && !authStore.token
  },
  { immediate: true }
)

onMounted(() => {
  window.addEventListener('storage', handleStorageChange)
  document.addEventListener('visibilitychange', handleVisibilityChange)
  syncAuthNotice()
})

onUnmounted(() => {
  window.removeEventListener('storage', handleStorageChange)
  document.removeEventListener('visibilitychange', handleVisibilityChange)
})

async function handleRelogin() {
  showAuthError.value = false
  await authStore.logout()
  await router.push('/')
}

async function handleLogout() {
  await authStore.logout()
  await router.push('/')
}

function isNavActive(href: string) {
  if (href === '/admin/dashboard') {
    return route.path === href
  }
  return route.path === href || route.path.startsWith(`${href}/`)
}

function prefetchNavigationItem(href: string) {
  prefetchNavigationTarget(router, href)
}

const navigation = computed(() => {
  return buildNavigation({ t })
})

const currentRoleLabel = computed(() => t('auth.role.admin'))

const breadcrumbs = computed(() => buildBreadcrumbs({
  route,
  navigation: navigation.value,
  isNavActive,
  t,
}))

// Styling Classes (Editorial)
const sidebarClasses = computed(() => {
    const widthClass = sidebarCollapsed.value ? 'w-16' : 'w-[260px]'
    return `${widthClass} flex-col hidden lg:flex border-r border-[#0f172a]/5 dark:border-white/5 bg-[#ffffff] dark:bg-[#0b1220] h-screen sticky top-0 transition-[width] [transition-duration:240ms] [transition-timing-function:cubic-bezier(0.4,0,0.2,1)] motion-reduce:transition-none`
})

const contentClasses = computed(() => {
    return `flex-1 min-w-0 bg-[#ffffff] dark:bg-[#0b1220] text-[#0f172a] dark:text-[#93c5fd]`
})

const mainClasses = computed(() => {
    // 移动端需要 pt-24 来避开固定头部（约69px）+ 额外间距
    // 桌面端内容在 sticky header 下方，但需要一些内边距让内容不紧贴
    return `pt-24 lg:pt-6`
})

</script>

<style scoped>
.sidebar-mode-enter-active {
  transition: opacity 120ms ease-out;
}

.sidebar-mode-leave-active {
  transition: opacity 60ms ease-in;
}

.sidebar-mode-enter-from,
.sidebar-mode-leave-to {
  opacity: 0;
}

.sidebar-icon-enter-active,
.sidebar-icon-leave-active {
  transition: opacity 60ms ease;
}

.sidebar-icon-enter-from,
.sidebar-icon-leave-to {
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .sidebar-mode-enter-active,
  .sidebar-mode-leave-active,
  .sidebar-icon-enter-active,
  .sidebar-icon-leave-active {
    transition: none;
  }
}

.scrollbar-none::-webkit-scrollbar { display: none; }
.scrollbar-none { -ms-overflow-style: none; scrollbar-width: none; }
</style>
