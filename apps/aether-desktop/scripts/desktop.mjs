import path from 'node:path'
import { desktop, options, prepare, run } from './build-common.mjs'

try {
  const [mode, ...args] = process.argv.slice(2)
  if (!['dev', 'build'].includes(mode)) throw new Error('用法：npm run dev 或 npm run build [-- --target <triple> --debug]')
  const build = options(args, mode === 'dev')
  const { env } = await prepare(build)
  const tauri = [mode]
  // Use the bundled origin in development too; the native command allowlist does
  // not grant privileged access to an HTTP development server.
  if (mode === 'dev') tauri.push('--no-dev-server')
  else {
    tauri.push('--ci')
    if (build.debug) tauri.push('--debug')
  }
  if (build.target) tauri.push('--target', build.target)
  tauri.push(...build.tauri, '--', '--locked')
  await run(path.join(desktop, 'node_modules/.bin/tauri'), tauri, desktop, env)
} catch (error) {
  console.error(error.message)
  process.exitCode = 1
}
