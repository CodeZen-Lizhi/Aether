import { createApp, nextTick, type App } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest'
import type { AxiosInstance, AxiosResponse, InternalAxiosRequestConfig } from 'axios'
import type { Router } from 'vue-router'
import { apiResponse, deferred, localUser, loginResponse, sessionToken, setDesktopBridge } from './sessionFixtures'

const { loginMounted, dashboardMounted, profileMounted, settingsMounted } = vi.hoisted(() => ({
  loginMounted: vi.fn(), dashboardMounted: vi.fn(), profileMounted: vi.fn(), settingsMounted: vi.fn(),
}))

vi.mock('@/router/routes', async () => {
  const { h } = await import('vue')
  const dashboard = { setup() { dashboardMounted(); return () => h('div', { 'data-page': 'dashboard' }, '管理页面') } }
  return { routes: [
    { path: '/', name: 'Login', meta: { requiresAuth: false }, component: {
      setup() { loginMounted(); return () => h('div', { 'data-page': 'login' }, '账号登录') },
    } },
    { path: '/admin/dashboard', meta: { requiresAuth: true, requiresAdmin: true }, component: dashboard },
    { path: '/admin/providers', meta: { requiresAuth: true, requiresAdmin: true }, component: dashboard },
    { path: '/admin/settings', meta: { requiresAuth: true }, component: {
      setup() { profileMounted(); return () => h('div', { 'data-page': 'profile' }, '个人设置') },
    } },
    { path: '/admin/system', meta: { requiresAuth: true, requiresAdmin: true }, component: {
      setup() { settingsMounted(); return () => h('div', { 'data-page': 'settings' }, '系统设置') },
    } },
  ] }
})

vi.mock('@/utils/logger', () => ({ log: { error: vi.fn(), warn: vi.fn(), info: vi.fn(), debug: vi.fn() } }))

let api: (typeof import('@/api/client'))['default']
let raw: { client: AxiosInstance; onStorageSync: EventListener }
let native: ReturnType<typeof vi.fn>
let respondToUser: (config: InternalAxiosRequestConfig) => Promise<AxiosResponse>
let requests: InternalAxiosRequestConfig[]
let mounted: { app: App; root: HTMLElement; router: Router } | undefined
let addedListeners: MockInstance<typeof window.addEventListener>

async function settle() {
  for (let index = 0; index < 30; index += 1) {
    await Promise.resolve()
    await nextTick()
  }
}

async function mountDashboard(path = '/admin/dashboard') {
  window.history.replaceState({}, '', path)
  const { default: Root } = await import('@/App.vue')
  const { default: router } = await import('@/router')
  const { useAuthStore } = await import('@/stores/auth')
  const { createI18n } = await import('@/i18n')
  const pinia = createPinia()
  setActivePinia(pinia)
  const auth = useAuthStore()
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp(Root).use(pinia).use(createI18n()).use(router)
  app.mount(root)
  mounted = { app, root, router }
  await settle()
  return { root, router, auth }
}

function reconnect(root: HTMLElement) {
  const button = Array.from(root.querySelectorAll('button')).find(element => element.textContent?.trim() === '重新连接')
  expect(button).toBeDefined()
  button!.click()
}

beforeEach(async () => {
  vi.resetModules()
  loginMounted.mockClear()
  dashboardMounted.mockClear()
  profileMounted.mockClear()
  settingsMounted.mockClear()
  addedListeners = vi.spyOn(window, 'addEventListener')
  native = vi.fn().mockResolvedValue(loginResponse())
  setDesktopBridge({ authenticate: native })
  api = (await import('@/api/client')).default
  raw = api as unknown as typeof raw
  requests = []
  respondToUser = async config => apiResponse(config, localUser)
  raw.client.defaults.adapter = async config => {
    requests.push(config)
    if (config.url === '/api/users/me') return respondToUser(config)
    return apiResponse(config, {})
  }
})

afterEach(() => {
  mounted?.app.unmount()
  mounted?.router.options.history.destroy()
  mounted?.root.remove()
  mounted = undefined
  api.clearAuth()
  window.removeEventListener('storage', raw.onStorageSync)
  for (const [type, listener, options] of addedListeners.mock.calls) {
    window.removeEventListener(type, listener, options)
  }
  Reflect.deleteProperty(window, '__AETHER_DESKTOP__')
  vi.restoreAllMocks()
})

describe('native dashboard entry and navigation', () => {
  it('redirects desktop personal-settings links to the merged preferences section', async () => {
    const { root, router } = await mountDashboard('/admin/settings?source=shortcut')
    await vi.waitFor(() => expect(router.currentRoute.value.fullPath).toBe('/admin/system?source=shortcut#section-preferences'))
    await nextTick()
    expect(root.querySelector('[data-page="settings"]')).not.toBeNull()
    expect(settingsMounted).toHaveBeenCalledTimes(1)
    expect(profileMounted).not.toHaveBeenCalled()
    expect(loginMounted).not.toHaveBeenCalled()
  })

  it('keeps the original personal-settings route for signed-in Web users', async () => {
    setDesktopBridge(undefined)
    api.setToken(sessionToken('web'))
    const { root, router } = await mountDashboard('/admin/settings')
    await vi.waitFor(() => expect(router.currentRoute.value.path).toBe('/admin/settings'))
    await nextTick()
    expect(root.querySelector('[data-page="profile"]')).not.toBeNull()
    expect(profileMounted).toHaveBeenCalledTimes(1)
    expect(settingsMounted).not.toHaveBeenCalled()
    expect(native).not.toHaveBeenCalled()
  })

  it('does not mount login or protected pages until the current local administrator is verified', async () => {
    const exchange = deferred<unknown>()
    const userResponse = deferred<void>()
    native.mockReturnValue(exchange.promise)
    respondToUser = async config => { await userResponse.promise; return apiResponse(config, localUser) }
    api.setToken(sessionToken('old-install'))
    const { root, router, auth } = await mountDashboard('/')
    expect(root.textContent).toContain('正在连接本机网关')
    expect(loginMounted).not.toHaveBeenCalled()
    expect(dashboardMounted).not.toHaveBeenCalled()
    expect(requests).toHaveLength(0)
    exchange.resolve(loginResponse())
    await settle()
    expect(native).toHaveBeenCalledTimes(1)
    expect(requests).toHaveLength(1)
    expect(requests[0].headers.Authorization).toBe(`Bearer ${sessionToken()}`)
    expect(requests[0].headers['X-Client-Device-Id']).toBe(native.mock.calls[0][0])
    expect(dashboardMounted).not.toHaveBeenCalled()
    userResponse.resolve()
    await vi.waitFor(() => expect(router.currentRoute.value.path).toBe('/admin/dashboard'))
    await nextTick()
    expect(auth.desktopReady).toBe(true)
    expect(root.querySelector('[data-page="dashboard"]')).not.toBeNull()
    expect(router.currentRoute.value.path).toBe('/admin/dashboard')
    expect(loginMounted).not.toHaveBeenCalled()
  })

  it('keeps a failed connection visible without automatic retries, and one retry restores the requested page', async () => {
    native.mockRejectedValueOnce(new Error('gateway stopped'))
    const { root, router } = await mountDashboard('/admin/providers?tab=keys')
    expect(root.textContent).toContain('无法连接本机网关')
    expect(dashboardMounted).not.toHaveBeenCalled()
    await router.push('/admin/dashboard')
    await router.push('/')
    await settle()
    expect(native).toHaveBeenCalledTimes(1)
    expect(loginMounted).not.toHaveBeenCalled()
    reconnect(root)
    await vi.waitFor(() => expect(router.currentRoute.value.fullPath).toBe('/admin/providers?tab=keys'))
    await nextTick()
    expect(native).toHaveBeenCalledTimes(2)
    expect(router.currentRoute.value.fullPath).toBe('/admin/providers?tab=keys')
    expect(root.querySelector('[data-page="dashboard"]')).not.toBeNull()
    expect(loginMounted).not.toHaveBeenCalled()
  })

  it.each([
    { ...localUser, role: 'user' },
    { ...localUser, is_active: false },
    null,
  ])('does not trust a returned token without a valid active administrator: %j', async user => {
    respondToUser = async config => apiResponse(config, user)
    const { root, router, auth } = await mountDashboard()
    expect(root.textContent).toContain('无法验证本机网关的管理权限')
    expect(auth.desktopReady).toBe(false)
    expect(api.getToken()).toBeNull()
    await router.push('/admin/providers')
    expect(native).toHaveBeenCalledTimes(1)
    expect(loginMounted).not.toHaveBeenCalled()
    expect(dashboardMounted).not.toHaveBeenCalled()
  })

  it('does not use the Web guard fallback to allow a page when user verification fails', async () => {
    respondToUser = async () => { throw new Error('unexpected verification failure') }
    const { root, router } = await mountDashboard()
    expect(root.textContent).toContain('无法连接本机网关')
    await router.push('/admin/providers')
    expect(loginMounted).not.toHaveBeenCalled()
    expect(dashboardMounted).not.toHaveBeenCalled()
    expect(native).toHaveBeenCalledTimes(1)
  })

  it('synchronizes a refreshed token in the store without reconnecting or unmounting the page', async () => {
    const { root, auth } = await mountDashboard()
    api.setToken(sessionToken('rotated'))
    await settle()
    expect(auth.token).toBe(sessionToken('rotated'))
    expect(auth.desktopReady).toBe(true)
    expect(root.querySelector('[data-page="dashboard"]')).not.toBeNull()
    expect(dashboardMounted).toHaveBeenCalledTimes(1)
    expect(native).toHaveBeenCalledTimes(1)
    expect(requests).toHaveLength(1)
  })

  it('shows manual reconnect after authentication is cleared and retains the current route', async () => {
    const { root, router, auth } = await mountDashboard('/admin/providers')
    api.clearAuth()
    await settle()
    expect(auth.desktopReady).toBe(false)
    expect(root.textContent).toContain('无法连接本机网关')
    expect(router.currentRoute.value.path).toBe('/admin/providers')
    expect(native).toHaveBeenCalledTimes(1)
    reconnect(root)
    await settle()
    expect(auth.desktopReady).toBe(true)
    expect(router.currentRoute.value.path).toBe('/admin/providers')
    expect(loginMounted).not.toHaveBeenCalled()
  })

  it('does not restore stale user information after a connection was invalidated', async () => {
    const userResponse = deferred<void>()
    respondToUser = async config => { await userResponse.promise; return apiResponse(config, localUser) }
    const { root, auth } = await mountDashboard()
    api.clearAuth()
    userResponse.resolve()
    await settle()
    expect(auth.desktopReady).toBe(false)
    expect(auth.user).toBeNull()
    expect(root.textContent).toContain('无法连接本机网关')
    expect(dashboardMounted).not.toHaveBeenCalled()
  })

  it('keeps ordinary Web login when only a boolean desktop marker is present', async () => {
    setDesktopBridge(true)
    const { root, router } = await mountDashboard('/')
    expect(root.querySelector('[data-page="login"]')).not.toBeNull()
    expect(router.currentRoute.value.path).toBe('/')
    expect(native).not.toHaveBeenCalled()
    expect(dashboardMounted).not.toHaveBeenCalled()
  })
})
