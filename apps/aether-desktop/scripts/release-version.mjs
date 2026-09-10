import { mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'

export function nextVersion(version) {
  if (!/^(0|[1-9]\d{0,3})\.(0|[1-9]\d?)\.(0|[1-9]\d?)$/.test(version)) {
    throw new Error(`无效的桌面版本号：${version}，需要数字版本，如 0.1.1。`)
  }
  let [major, minor, patch] = version.split('.').map(Number)
  if (++patch > 99) { patch = 0; minor += 1 }
  if (minor > 99) { minor = 0; major += 1 }
  if (major > 9999) throw new Error('桌面版本号已超过 macOS 构建版本范围。')
  return `${major}.${minor}.${patch}`
}

function cargoPackage(source) {
  const blocks = source.split(/(?=^\[)/m)
  const indexes = blocks.flatMap((block, index) => (
    /^(?:\[package\]|\[\[package\]\])\r?\n/.test(block)
    && /^name\s*=\s*"aether-desktop"\s*$/m.test(block)
      ? [index] : []
  ))
  if (indexes.length !== 1) throw new Error('找不到唯一的 aether-desktop Cargo 包。')
  const index = indexes[0]
  const match = /^version\s*=\s*"([^"]+)"/m.exec(blocks[index])
  if (!match) throw new Error('aether-desktop 缺少 Cargo 版本号。')
  return {
    version: match[1],
    update(version) {
      blocks[index] = blocks[index].replace(match[0], match[0].replace(`"${match[1]}"`, `"${version}"`))
      return blocks.join('')
    },
  }
}

// The caller holds the build lock until packaging finishes so another build
// cannot change the Cargo/Tauri version while the first one is compiling.
export async function bumpReleaseVersion(root) {
  const desktop = path.join(root, 'apps/aether-desktop')
  const files = [
    path.join(desktop, 'package.json'),
    path.join(desktop, 'package-lock.json'),
    path.join(desktop, 'src-tauri/tauri.conf.json'),
    path.join(desktop, 'src-tauri/Cargo.toml'),
    path.join(root, 'Cargo.lock'),
  ]
  const originals = await Promise.all(files.map(file => readFile(file, 'utf8')))
  const [manifest, lock, config] = originals.slice(0, 3).map(value => JSON.parse(value))
  const cargo = originals.slice(3).map(cargoPackage)
  const current = config.version
  const version = nextVersion(current)
  if ([manifest.version, lock.version, lock.packages?.['']?.version, ...cargo.map(pkg => pkg.version)]
    .some(value => value !== current)) {
    throw new Error('桌面版本元数据不一致，请先核对 npm、Tauri 和 Cargo 版本；未修改任何文件。')
  }
  manifest.version = version
  lock.version = version
  lock.packages[''].version = version
  config.version = version
  const updated = [manifest, lock, config].map(value => `${JSON.stringify(value, null, 2)}\n`)
  updated.push(...cargo.map(pkg => pkg.update(version)))
  const written = []
  try {
    for (let index = 0; index < files.length; index++) {
      written.push(index)
      await writeFile(files[index], updated[index])
    }
  } catch (error) {
    const restored = await Promise.allSettled(written.map(index => writeFile(files[index], originals[index])))
    const failures = restored.filter(result => result.status === 'rejected')
    if (failures.length) throw new AggregateError([error, ...failures.map(result => result.reason)], '版本写入及回滚失败，请检查版本文件后重试。')
    throw error
  }
  return version
}

export async function withBuildLock(root, action) {
  const directory = path.join(root, 'target')
  const lock = path.join(directory, '.aether-desktop-build.lock')
  await mkdir(directory, { recursive: true })
  try {
    await mkdir(lock)
  } catch (error) {
    if (error.code !== 'EEXIST') throw error
    throw new Error(`已有桌面打包任务持有构建锁：${lock}。若上次进程异常终止，请确认其已退出后清理该锁。`)
  }
  try {
    await writeFile(path.join(lock, 'pid'), String(process.pid))
    return await action()
  } finally {
    await rm(lock, { recursive: true })
  }
}
