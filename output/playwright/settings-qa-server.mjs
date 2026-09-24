import { spawn } from 'node:child_process'
import { randomBytes } from 'node:crypto'
import { mkdirSync, mkdtempSync, openSync, writeFileSync } from 'node:fs'
import path from 'node:path'

const repo = path.resolve(import.meta.dirname, '../..')
const runtime = mkdtempSync(path.join(import.meta.dirname, 'settings-runtime-'))
const gatewayLog = openSync(path.join(runtime, 'gateway.log'), 'a')
const gateway = spawn(path.join(repo, 'target/release/aether-gateway'), [
  '--app-host', '127.0.0.1', '--app-port', '18087', '--listener-shards', '1',
  '--auto-prepare-database',
], {
  cwd: runtime,
  detached: true,
  stdio: ['ignore', gatewayLog, gatewayLog],
  env: {
    PATH: process.env.PATH,
    ENVIRONMENT: 'development',
    AETHER_DATABASE_DRIVER: 'sqlite',
    AETHER_DATABASE_URL: `sqlite://${path.join(runtime, 'test.db')}?mode=rwc`,
    AETHER_RUNTIME_BACKEND: 'memory',
    AETHER_LOG_DESTINATION: 'stdout',
    RUST_LOG: 'aether_gateway=info',
    JWT_SECRET_KEY: randomBytes(48).toString('base64url'),
    ENCRYPTION_KEY: randomBytes(32).toString('base64url'),
    ADMIN_USERNAME: 'settings-qa',
    ADMIN_EMAIL: 'settings-qa@example.test',
    ADMIN_PASSWORD: 'Settings-QA-Only-2026!',
  },
})
gateway.unref()
mkdirSync(path.join(runtime, 'vite'), { recursive: true })
const viteLog = openSync(path.join(runtime, 'vite.log'), 'a')
const vite = spawn(process.execPath, [
  path.join(repo, 'frontend/node_modules/vite/bin/vite.js'), '--host', '127.0.0.1', '--port', '5173', '--strictPort',
], {
  cwd: path.join(repo, 'frontend'),
  detached: true,
  stdio: ['ignore', viteLog, viteLog],
  env: { ...process.env, APP_PORT: '18087' },
})
vite.unref()
writeFileSync(path.join(import.meta.dirname, 'settings-qa-state.json'), JSON.stringify({ runtime, gatewayPid: gateway.pid, vitePid: vite.pid }, null, 2))
console.log(JSON.stringify({ runtime, gatewayPid: gateway.pid, vitePid: vite.pid, url: 'http://127.0.0.1:5173' }))
