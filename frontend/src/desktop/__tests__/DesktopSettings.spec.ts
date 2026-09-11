import { afterEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, type App } from 'vue'
import DesktopSettings from '../DesktopSettings.vue'
import type { DesktopStatus } from '../bridge'

const mounted: Array<{ app: App; root: HTMLElement }> = []
const status: DesktopStatus = {
  phase: 'running', configured: true, port: 8084, gateway_url: 'http://127.0.0.1:8084',
  data_dir: '/test/data', log_dir: '/test/logs', autostart: false, pid: 42, error: null, version: '1.0.0',
}

function mountSettings(overrides: Record<string, unknown> = {}) {
  const root = document.createElement('div')
  document.body.appendChild(root)
  const onSetPort = vi.fn()
  const onSetAutostart = vi.fn()
  const app = createApp(DesktopSettings, {
    status,
    disabled: false,
    canEditPort: true,
    savingPort: false,
    onSetPort,
    onSetAutostart,
    onOpenDataDir: vi.fn(),
    onOpenLogDir: vi.fn(),
    ...overrides,
  })
  app.mount(root)
  mounted.push({ app, root })
  return { root, onSetPort, onSetAutostart }
}

afterEach(() => {
  for (const { app, root } of mounted.splice(0)) {
    app.unmount()
    root.remove()
  }
})

describe('desktop settings section', () => {
  it('is expanded by default and collapses through its accessible heading button', async () => {
    const { root } = mountSettings()
    const toggle = root.querySelector<HTMLButtonElement>('button[aria-controls="desktop-settings-content"]')!

    expect(toggle.tagName).toBe('BUTTON')
    expect(toggle.type).toBe('button')
    expect(toggle.getAttribute('aria-expanded')).toBe('true')
    expect(toggle.getAttribute('aria-label')).toBe('桌面应用')
    expect(root.querySelector('#gateway-port')).not.toBeNull()
    expect(root.querySelector('[role="switch"]')).not.toBeNull()
    expect(root.querySelector('details')).not.toBeNull()

    toggle.focus()
    expect(document.activeElement).toBe(toggle)
    toggle.click()
    await nextTick()

    expect(toggle.getAttribute('aria-expanded')).toBe('false')
    expect(toggle.getAttribute('aria-label')).toBe('桌面应用')
    const content = root.querySelector<HTMLElement>('#desktop-settings-content')!
    expect(content.style.display).toBe('none')
    expect(content.querySelector('#gateway-port')).not.toBeNull()
    expect(content.querySelector('[role="switch"]')).not.toBeNull()
    expect(content.querySelector('code')).not.toBeNull()
    expect(content.querySelector('details')).not.toBeNull()

    toggle.click()
    await nextTick()
    expect(toggle.getAttribute('aria-expanded')).toBe('true')
    expect(root.querySelector('#gateway-port')).not.toBeNull()
  })

  it('keeps recovery controls directly expanded without a collapse trigger', () => {
    const { root } = mountSettings({ recovery: true })

    expect(root.textContent).toContain('恢复网关')
    expect(root.querySelector('button[aria-controls="desktop-settings-content"]')).toBeNull()
    expect(root.querySelector('#gateway-port')).not.toBeNull()
    expect(root.querySelector('[role="switch"]')).toBeNull()
    expect(root.querySelector('code')).toBeNull()
    expect(root.querySelector('details')).toBeNull()
  })

  it('lives as a desktop app section and restarts after a valid port save', async () => {
    const { root, onSetPort } = mountSettings()
    expect(root.textContent).toContain('桌面应用')
    expect(root.textContent).not.toContain('客户端设置')
    expect(root.textContent).toContain('保存后会自动重启网关')

    const input = root.querySelector<HTMLInputElement>('#gateway-port')!
    input.value = '8085'
    input.dispatchEvent(new Event('input', { bubbles: true }))
    await nextTick()
    root.querySelector<HTMLButtonElement>('button[type="submit"]')!.click()
    expect(onSetPort).toHaveBeenCalledWith(8085)
  })

  it('keeps invalid ports in the form and exposes the inline error', async () => {
    const { root, onSetPort } = mountSettings()
    const input = root.querySelector<HTMLInputElement>('#gateway-port')!
    input.value = '80'
    input.dispatchEvent(new Event('input', { bubbles: true }))
    await nextTick()
    root.querySelector<HTMLButtonElement>('button[type="submit"]')!.click()
    await nextTick()
    expect(onSetPort).not.toHaveBeenCalled()
    expect(root.querySelector('#gateway-port-error')).toBe(document.activeElement)
  })

  it('updates login startup from the switch', async () => {
    const { root, onSetAutostart } = mountSettings()
    root.querySelector<HTMLButtonElement>('[role="switch"]')!.click()
    await nextTick()
    expect(onSetAutostart).toHaveBeenCalledWith(true)
  })

  it('keeps diagnostics in system settings and refreshes them on demand', async () => {
    const onRefreshLogs = vi.fn()
    const { root } = mountSettings({
      logs: ['[desktop] gateway ready'],
      onRefreshLogs,
    })
    const diagnostics = root.querySelector<HTMLDetailsElement>('details')!
    expect(diagnostics).not.toBeNull()
    expect(root.textContent).toContain('[desktop] gateway ready')
    diagnostics.open = true
    diagnostics.dispatchEvent(new Event('toggle'))
    expect(onRefreshLogs).toHaveBeenCalledOnce()
  })
})
