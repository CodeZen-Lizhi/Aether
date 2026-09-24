async (page) => {
  await page.addInitScript(() => {
    const stateKey = 'settings-stable-desktop-state'
    const defaults = {
      phase: 'running', configured: true, port: 8084,
      gateway_url: 'http://127.0.0.1:8084',
      data_dir: '/isolated/settings-layout-qa/data', log_dir: '/isolated/settings-layout-qa/logs',
      autostart: true, pid: 100, error: null, version: '0.1.18',
    }
    const status = JSON.parse(localStorage.getItem(stateKey) || 'null') || defaults
    window.__settingsStableCommands = []
    window.isTauri = true
    window.__AETHER_DESKTOP__ = {
      authenticate: async () => ({ access_token: localStorage.getItem('access_token') }),
    }
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args = {}) => {
        window.__settingsStableCommands.push({ command, args })
        if (window.__settingsStableFailNext === command) {
          window.__settingsStableFailNext = null
          throw new Error('Isolated QA: simulated desktop command failure')
        }
        if (command === 'desktop_logs') return ['Isolated settings layout QA log sample.']
        if (command === 'desktop_set_port') {
          status.port = args.port
          status.gateway_url = `http://127.0.0.1:${args.port}`
        }
        if (command === 'desktop_set_autostart') status.autostart = args.enabled
        localStorage.setItem(stateKey, JSON.stringify(status))
        return { ...status }
      },
    }
  })
  await page.goto('http://127.0.0.1:5175/admin/system')
  await page.locator('.system-settings').waitFor()
  await page.setViewportSize({ width: 900, height: 640 })
  return { mode: 'Real isolated HTTP API, simulated desktop IPC', url: page.url() }
}
