import { spawn } from 'node:child_process'
import { randomBytes } from 'node:crypto'
import { mkdtempSync, openSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const repo = path.resolve(here, '../..')
const stateFile = path.join(here, 'settings-stable-state.json')
const gatewayPort = 18089
const previewPort = 5175

if (!process.argv.includes('--serve')) {
  const log = openSync(path.join(here, 'settings-stable-server.log'), 'a')
  const child = spawn(process.execPath, [fileURLToPath(import.meta.url), '--serve'], {
    cwd: repo, detached: true, stdio: ['ignore', log, log],
  })
  child.unref()
  console.log(JSON.stringify({ serverPid: child.pid, url: `http://127.0.0.1:${previewPort}/admin/system`, stateFile }))
} else {
  const runtime = mkdtempSync(path.join(here, 'settings-stable-runtime-'))
  const gatewayLog = openSync(path.join(runtime, 'gateway.log'), 'a')
  const gateway = spawn(path.join(repo, 'target/release/aether-gateway'), [
    '--app-host', '127.0.0.1', '--app-port', String(gatewayPort), '--listener-shards', '1',
    '--auto-prepare-database', '--exit-on-stdin-close',
  ], {
    cwd: runtime, stdio: ['pipe', gatewayLog, gatewayLog],
    env: {
      PATH: process.env.PATH, ENVIRONMENT: 'development', AETHER_DATABASE_DRIVER: 'sqlite',
      AETHER_DATABASE_URL: `sqlite://${path.join(runtime, 'test.db')}?mode=rwc`,
      AETHER_RUNTIME_BACKEND: 'memory', AETHER_LOG_DESTINATION: 'stdout', RUST_LOG: 'aether_gateway=info',
      JWT_SECRET_KEY: randomBytes(48).toString('base64url'), ENCRYPTION_KEY: randomBytes(32).toString('base64url'),
      ADMIN_USERNAME: 'settings-layout-qa', ADMIN_EMAIL: 'settings-layout-qa@example.test',
      ADMIN_PASSWORD: 'Settings-Layout-QA-Only-2026!',
    },
  })
  const { createServer } = await import(path.join(repo, 'frontend/node_modules/vite/dist/node/index.js'))
  process.chdir(path.join(repo, 'frontend'))
  const target = `http://127.0.0.1:${gatewayPort}`
  const server = await createServer({
    root: path.join(repo, 'frontend'), configFile: path.join(repo, 'frontend/vite.config.ts'),
    server: {
      host: '127.0.0.1', port: previewPort, strictPort: true,
      proxy: Object.fromEntries(['/api/', '/v1/', '/health', '/_gateway/'].map(prefix => [prefix, { target, changeOrigin: true }])),
    },
  })
  await server.listen()
  writeFileSync(stateFile, JSON.stringify({ runtime, serverPid: process.pid, gatewayPid: gateway.pid, gatewayPort, previewPort }, null, 2))
  console.log(`Settings layout preview: http://127.0.0.1:${previewPort}`)
  async function stop() { gateway.stdin.end(); await server.close(); }
  process.once('SIGTERM', stop)
  process.once('SIGINT', stop)
}
