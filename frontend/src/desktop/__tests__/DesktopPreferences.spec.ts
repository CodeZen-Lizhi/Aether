import { createApp, h, nextTick, type App, type Component } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import { createMemoryHistory, createRouter, type Router } from 'vue-router'
import type { AxiosInstance, AxiosResponse, InternalAxiosRequestConfig } from 'axios'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createI18n, setI18nLocale } from '@/i18n'
import apiClient from '@/api/client'
import { useAuthStore } from '@/stores/auth'
import { useDarkMode } from '@/composables/useDarkMode'
import { cache } from '@/utils/cache'
import MainLayout from '@/layouts/MainLayout.vue'
import ProfileSettings from '@/views/admin/ProfileSettings.vue'
import SystemSettings from '@/views/admin/SystemSettings.vue'
import { apiResponse, localUser, sessionToken, setDesktopBridge } from './sessionFixtures'

let mounted: { app: App; root: HTMLElement; router: Router } | undefined
const raw = apiClient as unknown as { client: AxiosInstance }
const previousAdapter = raw.client.defaults.adapter
let requests: Array<{ url?: string; method?: string; data: unknown }>
let respondToPreferences: (config: InternalAxiosRequestConfig) => Promise<AxiosResponse>

async function settle() {
  for (let index = 0; index < 15; index += 1) {
    await Promise.resolve()
    await nextTick()
  }
}

async function mountComponent(component: Component, desktop: boolean, path = '/admin/system') {
  setDesktopBridge(desktop ? { authenticate: () => Promise.resolve({ access_token: sessionToken() }) } : undefined)
  apiClient.setToken(sessionToken())
  const pinia = createPinia()
  setActivePinia(pinia)
  useAuthStore().user = localUser
  const router = createRouter({ history: createMemoryHistory(), routes: [
    { path: '/admin/settings', component: { render: () => h('div') } },
    { path: '/:pathMatch(.*)*', component: { render: () => h('div') } },
  ] })
  await router.push(path)
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp(component).use(pinia).use(createI18n()).use(router)
  app.mount(root)
  mounted = { app, root, router }
  await settle()
  return root
}

async function editTimezone(root: HTMLElement, value: string) {
  const timezone = root.querySelector<HTMLInputElement>('#timezone')!
  timezone.value = value
  timezone.dispatchEvent(new Event('input', { bubbles: true }))
  await nextTick()
  timezone.dispatchEvent(new Event('change', { bubbles: true }))
  await settle()
  return timezone
}

function preferenceSaves() {
  return requests.filter(request => request.url === '/api/users/me/preferences' && request.method === 'put')
}

function saveButton(root: HTMLElement) {
  return root.querySelector<HTMLButtonElement>('#section-preferences button')!
}

beforeEach(() => {
  useDarkMode().setThemeMode('light')
  cache.clear()
  requests = []
  respondToPreferences = async config => apiResponse(config, { theme: 'light', language: 'zh-CN', timezone: 'Asia/Shanghai' })
  raw.client.defaults.adapter = async config => {
    requests.push({ url: config.url, method: config.method, data: config.data ? JSON.parse(config.data) : undefined })
    if (config.url === '/api/users/me/preferences') return respondToPreferences(config)
    if (config.url === '/api/users/me') return apiResponse(config, { ...localUser, auth_source: 'local', has_password: true })
    if (config.url === '/api/users/me/sessions') return apiResponse(config, [])
    if (config.url === '/api/admin/system/configs') return apiResponse(config, [])
    if (config.url === '/api/admin/system/version') return apiResponse(config, { version: 'test' })
    if (config.url === '/api/admin/system/cleanup/runs') return apiResponse(config, { items: [] })
    if (config.url === '/api/admin/proxy-nodes') return apiResponse(config, { items: [], total: 0, skip: 0, limit: 1000 })
    return apiResponse(config, {})
  }
})

afterEach(() => {
  mounted?.app.unmount()
  mounted?.router.options.history.destroy()
  mounted?.root.remove()
  mounted = undefined
  apiClient.clearAuth()
  raw.client.defaults.adapter = previousAdapter
  Reflect.deleteProperty(window, '__AETHER_DESKTOP__')
})

describe('desktop preferences and account controls', () => {
  it('merges one set of preferences into system settings and saves only after confirmation', async () => {
    const root = await mountComponent(SystemSettings, true)
    await vi.waitFor(() => expect(root.querySelector('#timezone')).not.toBeNull())
    expect(root.querySelector('#section-preferences')?.textContent).toContain('偏好设置')
    expect(root.querySelector('nav')?.textContent).toContain('偏好设置')
    expect(root.textContent).not.toMatch(/账号设置|用户名|密码|登录设备|退出其他设备/)
    for (const id of ['theme', 'language', 'timezone']) expect(root.querySelectorAll(`#${id}`)).toHaveLength(1)
    for (const id of ['section-site-info', 'section-data-mgmt', 'section-proxy', 'section-basic', 'section-request-log', 'section-cleanup', 'section-sysinfo']) {
      expect(root.querySelector(`#${id}`)).not.toBeNull()
    }
    expect(requests.filter(request => request.url?.startsWith('/api/users/')).map(request => request.url)).toEqual(['/api/users/me/preferences'])
    expect(saveButton(root).disabled).toBe(true)
    await editTimezone(root, 'UTC')
    expect(preferenceSaves()).toHaveLength(0)
    expect(saveButton(root).disabled).toBe(false)
    saveButton(root).click()
    await vi.waitFor(() => expect(saveButton(root).disabled).toBe(true))
    expect(preferenceSaves()).toEqual([{ url: '/api/users/me/preferences', method: 'put', data: {
      theme: 'light', language: 'zh-CN', timezone: 'UTC',
    } }])
  })

  it('retains edited preferences after a failed save and allows retry', async () => {
    let failNextSave = true
    respondToPreferences = async config => {
      if (config.method === 'put' && failNextSave) {
        failNextSave = false
        return apiResponse(config, { detail: 'Save failed' }, 400)
      }
      return apiResponse(config, { theme: 'light', language: 'zh-CN', timezone: 'Asia/Shanghai' })
    }
    const root = await mountComponent(SystemSettings, true)
    const timezone = await editTimezone(root, 'UTC')
    saveButton(root).click()
    await vi.waitFor(() => {
      expect(preferenceSaves()).toHaveLength(1)
      expect(saveButton(root).disabled).toBe(false)
    })
    expect(timezone.value).toBe('UTC')
    saveButton(root).click()
    await vi.waitFor(() => {
      expect(preferenceSaves()).toHaveLength(2)
      expect(saveButton(root).disabled).toBe(true)
    })
    expect(preferenceSaves()[1].data).toEqual({ theme: 'light', language: 'zh-CN', timezone: 'UTC' })
  })

  it('offers a retry when loading preferences fails instead of saving default values', async () => {
    let failNextLoad = true
    respondToPreferences = async config => {
      if (failNextLoad) {
        failNextLoad = false
        return apiResponse(config, { detail: 'Load failed' }, 503)
      }
      return apiResponse(config, { theme: 'light', language: 'zh-CN', timezone: 'UTC' })
    }
    const root = await mountComponent(SystemSettings, true)
    expect(root.querySelector('#section-preferences [role="alert"]')?.textContent).toContain('加载偏好设置失败')
    expect(root.querySelector('#timezone')).toBeNull()
    expect(saveButton(root).disabled).toBe(true)
    const retry = Array.from(root.querySelectorAll<HTMLButtonElement>('#section-preferences button')).find(button => button.textContent?.trim() === '重试')!
    retry.click()
    await vi.waitFor(() => expect(root.querySelector<HTMLInputElement>('#timezone')?.value).toBe('UTC'))
    expect(preferenceSaves()).toHaveLength(0)
    expect(saveButton(root).disabled).toBe(true)
  })

  it('keeps untouched theme and language fields in sync with header shortcuts', async () => {
    const root = await mountComponent(SystemSettings, true)
    useDarkMode().setThemeMode('dark')
    setI18nLocale('en-US')
    await settle()
    expect(root.querySelector('#theme')?.textContent).toContain('Dark')
    expect(root.querySelector('#language')?.textContent).toContain('English')
    expect(saveButton(root).disabled).toBe(true)
    await editTimezone(root, 'UTC')
    saveButton(root).click()
    await vi.waitFor(() => expect(preferenceSaves()).toHaveLength(1))
    expect(preferenceSaves()[0].data).toEqual({ theme: 'dark', language: 'en', timezone: 'UTC' })
  })

  it('retains Web account settings and immediate preference saving', async () => {
    const root = await mountComponent(ProfileSettings, false, '/admin/settings')
    expect(root.textContent).toContain('账号设置')
    expect(root.querySelector('#username')).not.toBeNull()
    expect(root.querySelector('#old-password')).not.toBeNull()
    expect(root.querySelector('#new-password')).not.toBeNull()
    expect(root.textContent).toContain('登录设备')
    expect(requests.map(request => request.url).sort()).toEqual([
      '/api/users/me', '/api/users/me/preferences', '/api/users/me/sessions',
    ])
    await editTimezone(root, 'UTC')
    await vi.waitFor(() => expect(preferenceSaves()).toHaveLength(1))
    expect(preferenceSaves()[0].data).toMatchObject({ theme: 'light', language: 'zh-CN', timezone: 'UTC' })
  })

  it('keeps Web system settings separate from personal preferences', async () => {
    const root = await mountComponent(SystemSettings, false)
    expect(root.querySelector('#section-site-info')).not.toBeNull()
    expect(root.querySelector('#section-preferences')).toBeNull()
    expect(root.querySelector('#timezone')).toBeNull()
    expect(root.querySelector('nav')?.textContent).not.toContain('偏好设置')
    expect(requests.some(request => request.url?.startsWith('/api/users/'))).toBe(false)
  })

  it('removes the desktop identity and personal-settings area from both navigation layouts', async () => {
    const root = await mountComponent(MainLayout, true)
    expect(root.textContent).not.toContain('本机网关')
    expect(root.textContent).not.toContain(localUser.username)
    expect(root.querySelector('a[href="/admin/settings"]')).toBeNull()
    expect(root.querySelectorAll('a[href="/admin/system"]')).toHaveLength(1)
    expect(root.querySelector('[title="退出登录"]')).toBeNull()
    const menuButton = root.querySelector<HTMLButtonElement>('button[aria-label="打开导航菜单"]')!
    menuButton.click()
    await settle()
    expect(menuButton.getAttribute('aria-expanded')).toBe('true')
    expect(root.querySelector('a[href="/admin/settings"]')).toBeNull()
    expect(root.querySelectorAll('a[href="/admin/system"]')).toHaveLength(2)
    expect(root.textContent).not.toContain('本机网关')
    expect(root.textContent).not.toContain(localUser.username)
    expect(root.querySelector('[title="退出登录"]')).toBeNull()
  })

  it('keeps the native minimum-width window on the left sidebar layout', async () => {
    const root = await mountComponent(MainLayout, true)
    const sidebar = root.querySelector<HTMLElement>('.app-shell__sidebar')!
    const mobileHeader = root.querySelector<HTMLButtonElement>('button[aria-label="打开导航菜单"]')!.closest('header')!
    const desktopHeader = Array.from(root.querySelectorAll('header')).find(header => header.classList.contains('hidden'))!
    const main = root.querySelector<HTMLElement>('.app-shell__main')!

    expect(root.querySelector('.app-shell')?.getAttribute('data-desktop-mode')).toBe('true')
    expect(sidebar.classList).toContain('md:flex')
    expect(sidebar.classList).not.toContain('lg:flex')
    expect(sidebar.classList).toContain('w-[184px]')
    expect(sidebar.classList).toContain('min-[1101px]:w-[224px]')
    expect(root.querySelector('.sidebar-nav')?.getAttribute('data-compact')).toBe('true')
    expect(mobileHeader.classList).toContain('md:hidden')
    expect(desktopHeader.classList).toContain('md:flex')
    expect(main.classList).toContain('md:pt-6')
  })

  it('retains Web account identity and logout controls', async () => {
    const root = await mountComponent(MainLayout, false)
    expect(root.textContent).toContain(localUser.username)
    expect(root.querySelector('[title="退出登录"]')).not.toBeNull()
    expect(root.querySelector('a[aria-label="个人设置"]')).not.toBeNull()
    expect(root.querySelector('.app-shell__sidebar')?.classList).toContain('lg:flex')
    expect(root.querySelector('.app-shell__sidebar')?.classList).not.toContain('md:flex')
    expect(root.querySelector('.app-shell__sidebar')?.classList).toContain('w-[224px]')
    expect(root.querySelector('.sidebar-nav')?.getAttribute('data-compact')).toBe('false')
  })
})
