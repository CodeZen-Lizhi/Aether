async (page) => {
  await page.addInitScript(() => {
    const status = {
      phase: 'running', configured: true, port: 18087,
      gateway_url: 'http://127.0.0.1:18087',
      data_dir: '/isolated/settings-qa/long-directory-name/Aether/data',
      log_dir: '/isolated/settings-qa/long-directory-name/Aether/logs',
      autostart: false, pid: 100, error: null, version: '0.1.12',
    }
    window.__settingsQaCommands = []
    window.isTauri = true
    window.__AETHER_DESKTOP__ = {
      authenticate: async () => ({ access_token: localStorage.getItem('access_token') }),
    }
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args = {}) => {
        window.__settingsQaCommands.push({ command, args })
        if (command === 'desktop_logs') return ['Settings QA: isolated diagnostic sample.']
        if (command === 'desktop_set_port') {
          status.port = args.port
          status.gateway_url = 'http://127.0.0.1:' + args.port
        }
        if (command === 'desktop_set_autostart') status.autostart = args.enabled
        if (command === 'desktop_stop') { status.phase = 'stopped'; status.pid = null }
        if (command === 'desktop_start' || command === 'desktop_restart') { status.phase = 'running'; status.pid = 100 }
        return { ...status }
      },
    }
  })
  await page.reload()
  await page.getByRole('heading', { name: '连接与启动', exact: true }).waitFor()
  return { mode: 'Desktop presentation with simulated IPC; real isolated HTTP API', url: page.url() }
}
