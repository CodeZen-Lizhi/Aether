import { spawn, execFileSync } from 'node:child_process'
import { copyFile, chmod, mkdir, access } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

export const desktop = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
export const root = path.resolve(desktop, '../..')
export const supportedTargets = ['aarch64-apple-darwin', 'x86_64-apple-darwin']

export function options(args, development = false) {
  const result = { debug: development, target: undefined, tauri: [] }
  for (let index = 0; index < args.length; index++) {
    const argument = args[index]
    if (argument === '--debug' && !development) result.debug = true
    else if (argument === '--target') {
      result.target = args[++index]
      if (!supportedTargets.includes(result.target)) throw new Error(`--target 须为 ${supportedTargets.join(' 或 ')}`)
    } else if (['--config', '--bundles'].includes(argument)) {
      const value = args[++index]
      if (!value || value.startsWith('--')) throw new Error(`${argument} 缺少参数`)
      result.tauri.push(argument, value)
    } else if (['--no-bundle', '--no-watch', '--verbose'].includes(argument)) result.tauri.push(argument)
    else throw new Error(`不支持的参数：${argument}`)
  }
  return result
}

export function binaryPath(targetDirectory, build) {
  return path.join(targetDirectory, build.target ?? '', build.debug ? 'debug' : 'release', 'aether-gateway')
}

export function run(command, args, cwd, env) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, env, stdio: 'inherit', shell: false })
    const interrupt = signal => child.kill(signal)
    const sigint = () => interrupt('SIGINT')
    const sigterm = () => interrupt('SIGTERM')
    process.once('SIGINT', sigint)
    process.once('SIGTERM', sigterm)
    function cleanup() {
      process.off('SIGINT', sigint)
      process.off('SIGTERM', sigterm)
    }
    child.once('error', error => { cleanup(); reject(error) })
    child.once('exit', (code, signal) => {
      cleanup()
      if (code === 0) resolve()
      else reject(new Error(`${command} ${args[0] ?? ''} 未完成（${signal ?? code}）`))
    })
  })
}

export async function prepare(build) {
  if (process.platform !== 'darwin') throw new Error('此桌面包目前只支持在 macOS 上构建。')
  const host = /^host: (.+)$/m.exec(execFileSync('rustc', ['-vV'], { encoding: 'utf8' }))?.[1]
  const target = build.target ?? host
  if (!supportedTargets.includes(target)) throw new Error(`不支持的构建目标：${target}`)
  const targetDirectory = path.resolve(root, process.env.CARGO_TARGET_DIR ?? 'target')
  const env = {
    ...process.env,
    CARGO_TARGET_DIR: targetDirectory,
    CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS ?? '2',
    MACOSX_DEPLOYMENT_TARGET: process.env.MACOSX_DEPLOYMENT_TARGET ?? '14.0',
    // Rust's symbol stripping can produce a misaligned Mach-O string table in
    // proc-macro dylibs on macOS 27. These build tools are not bundled.
    CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP: process.env.CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP ?? 'none',
  }
  // Dependency installation is explicit; npm ci must not trigger a full Rust build.
  await access(path.join(root, 'frontend/node_modules/.bin/vite')).catch(() => {
    throw new Error('请先在 frontend 目录执行 npm ci。')
  })
  await run('npm', ['run', 'build'], path.join(root, 'frontend'), env)
  const cargo = ['build', '-p', 'aether-gateway', '--bin', 'aether-gateway', '--locked']
  if (!build.debug) cargo.push('--release')
  if (build.target) cargo.push('--target', build.target)
  await run('cargo', cargo, root, env)
  const binaries = path.join(desktop, 'src-tauri/binaries')
  await mkdir(binaries, { recursive: true })
  const destination = path.join(binaries, `aether-gateway-${target}`)
  await copyFile(binaryPath(targetDirectory, build), destination)
  await chmod(destination, 0o755)
  return { env, targetDirectory }
}
