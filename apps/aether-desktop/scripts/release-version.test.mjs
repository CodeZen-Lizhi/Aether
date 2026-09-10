import test from 'node:test'
import assert from 'node:assert/strict'
import { access, copyFile, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { execFile } from 'node:child_process'
import os from 'node:os'
import path from 'node:path'
import { promisify } from 'node:util'
import { bumpReleaseVersion, nextVersion, withBuildLock } from './release-version.mjs'

const execFileAsync = promisify(execFile)

async function fixture(t) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'aether-version-'))
  t.after(() => rm(root, { recursive: true, force: true }))
  const desktop = path.join(root, 'apps/aether-desktop')
  await mkdir(path.join(desktop, 'src-tauri'), { recursive: true })
  await writeFile(path.join(desktop, 'package.json'), JSON.stringify({ name: 'aether-desktop', version: '0.1.0', private: true }))
  await writeFile(path.join(desktop, 'package-lock.json'), JSON.stringify({
    version: '0.1.0', packages: { '': { version: '0.1.0' }, 'node_modules/unrelated': { version: '0.1.0' } },
  }))
  await writeFile(path.join(desktop, 'src-tauri/tauri.conf.json'), JSON.stringify({ version: '0.1.0', identifier: 'com.aether.desktop' }))
  await writeFile(path.join(desktop, 'src-tauri/Cargo.toml'), '[package]\nname = "aether-desktop"\nversion = "0.1.0"\n\n[dependencies]\nother = "0.1.0"\n')
  await writeFile(path.join(root, 'Cargo.lock'), 'version = 4\n\n[[package]]\nname = "aether-gateway"\nversion = "0.1.0"\n\n[[package]]\nname = "aether-desktop"\nversion = "0.1.0"\ndependencies = ["other"]\n')
  return { root, desktop }
}

async function entrypointFixture(t) {
  const { root, desktop } = await fixture(t)
  const scripts = path.join(desktop, 'scripts')
  await mkdir(scripts)
  for (const file of ['desktop.mjs', 'prepare.mjs', 'release-version.mjs']) {
    await copyFile(new URL(file, import.meta.url), path.join(scripts, file))
  }
  // Run the real entrypoints and version allocator, replacing only expensive build stages.
  await writeFile(path.join(scripts, 'build-common.mjs'), [
    "import path from 'node:path'",
    "import { access, readFile, writeFile } from 'node:fs/promises'",
    'export { options } from ' + JSON.stringify(new URL('build-common.mjs', import.meta.url).href),
    'export const root = ' + JSON.stringify(root),
    "export const desktop = path.join(root, 'apps/aether-desktop')",
    'const steps = []',
    'async function record(stage, details) {',
    "  const config = JSON.parse(await readFile(path.join(desktop, 'src-tauri/tauri.conf.json'), 'utf8'))",
    '  let locked = true',
    "  await access(path.join(root, 'target/.aether-desktop-build.lock')).catch(error => {",
    "    if (error.code !== 'ENOENT') throw error",
    '    locked = false',
    '  })',
    '  steps.push({ stage, version: config.version, locked, ...details })',
    "  await writeFile(path.join(root, 'build-stages.json'), JSON.stringify(steps))",
    "  if (process.env.AETHER_VERSION_TEST_FAIL === stage) throw new Error('simulated ' + stage + ' failure')",
    '}',
    'export async function prepare(build) {',
    "  await record('prepare', { build })",
    "  return { env: { ...process.env, AETHER_VERSION_TEST_PREPARED: 'yes' } }",
    '}',
    'export async function run(command, args, cwd, env) {',
    "  await record('tauri', { command, args, cwd, prepared: env.AETHER_VERSION_TEST_PREPARED === 'yes' })",
    '}',
  ].join('\n'))
  return {
    root, desktop,
    launch: (args, env = {}, entry = 'desktop.mjs') => execFileAsync(process.execPath, [path.join(scripts, entry), ...args], {
      cwd: desktop, env: { ...process.env, AETHER_VERSION_TEST_FAIL: '', ...env }, timeout: 10_000,
    }),
    stages: async () => JSON.parse(await readFile(path.join(root, 'build-stages.json'), 'utf8')),
  }
}

test('build numbers increment and carry within macOS numeric version components', () => {
  assert.equal(nextVersion('0.1.0'), '0.1.1')
  assert.equal(nextVersion('0.1.99'), '0.2.0')
  assert.equal(nextVersion('1.99.99'), '2.0.0')
  for (const version of ['0.1.0-beta', '2026.0910.1000', '1.2', undefined, '9999.99.99']) {
    assert.throws(() => nextVersion(version))
  }
})

test('two builds allocate distinct versions across all metadata without changing dependencies', async t => {
  const { root, desktop } = await fixture(t)
  assert.equal(await withBuildLock(root, () => bumpReleaseVersion(root)), '0.1.1')
  assert.equal(await withBuildLock(root, () => bumpReleaseVersion(root)), '0.1.2')
  const json = async file => JSON.parse(await readFile(file, 'utf8'))
  assert.equal((await json(path.join(desktop, 'package.json'))).version, '0.1.2')
  assert.equal((await json(path.join(desktop, 'src-tauri/tauri.conf.json'))).version, '0.1.2')
  const lock = await json(path.join(desktop, 'package-lock.json'))
  assert.equal(lock.version, '0.1.2')
  assert.equal(lock.packages[''].version, '0.1.2')
  assert.equal(lock.packages['node_modules/unrelated'].version, '0.1.0')
  assert.match(await readFile(path.join(desktop, 'src-tauri/Cargo.toml'), 'utf8'), /version = "0.1.2"\n\n\[dependencies\]\nother = "0.1.0"/)
  const cargo = await readFile(path.join(root, 'Cargo.lock'), 'utf8')
  assert.match(cargo, /name = "aether-desktop"\nversion = "0.1.2"/)
  assert.match(cargo, /name = "aether-gateway"\nversion = "0.1.0"/)
})

test('inconsistent versions fail before any metadata changes', async t => {
  const { root, desktop } = await fixture(t)
  const cargo = path.join(desktop, 'src-tauri/Cargo.toml')
  await writeFile(cargo, (await readFile(cargo, 'utf8')).replace('version = "0.1.0"', 'version = "0.1.9"'))
  const manifest = path.join(desktop, 'package.json')
  const before = await readFile(manifest, 'utf8')
  await assert.rejects(withBuildLock(root, () => bumpReleaseVersion(root)), /版本元数据不一致/)
  assert.equal(await readFile(manifest, 'utf8'), before)
  assert.match(await readFile(cargo, 'utf8'), /version = "0.1.9"/)
})

test('concurrent builds are excluded and a failed attempt keeps its version but releases the lock', async t => {
  const { root } = await fixture(t)
  await assert.rejects(withBuildLock(root, async () => {
    assert.equal(await bumpReleaseVersion(root), '0.1.1')
    await assert.rejects(withBuildLock(root, () => bumpReleaseVersion(root)), /构建锁/)
    throw new Error('build failed')
  }), /build failed/)
  assert.equal(await withBuildLock(root, () => bumpReleaseVersion(root)), '0.1.2')
})

test('build entrypoint holds its lock and preserves flags before overriding signing versions', async t => {
  const { root, desktop, launch, stages } = await entrypointFixture(t)
  const signing = JSON.stringify({ version: '0.1.0', bundle: { macOS: { bundleVersion: '0.1.0', signingIdentity: '-' } } })
  const args = ['--target', 'x86_64-apple-darwin', '--debug', '--config', signing, '--no-bundle']
  const { stdout } = await launch(['build', ...args])
  assert.match(stdout, /Aether 桌面构建版本：0\.1\.1/)
  const [prepared, tauri] = await stages()
  assert.deepEqual([prepared.stage, tauri.stage], ['prepare', 'tauri'])
  for (const stage of [prepared, tauri]) {
    assert.equal(stage.locked, true)
    assert.equal(stage.version, '0.1.1')
  }
  assert.equal(prepared.build.debug, true)
  assert.equal(prepared.build.target, 'x86_64-apple-darwin')
  assert.deepEqual(tauri.args.slice(0, -4), ['build', '--ci', '--debug', '--target', 'x86_64-apple-darwin', '--config', signing, '--no-bundle'])
  assert.deepEqual(tauri.args.slice(-4), [
    '--config', JSON.stringify({ version: '0.1.1', bundle: { macOS: { bundleVersion: '0.1.1' } } }), '--', '--locked',
  ])
  assert.equal(tauri.command, path.join(desktop, 'node_modules/.bin/tauri'))
  assert.equal(tauri.cwd, desktop)
  assert.equal(tauri.prepared, true)
  await assert.rejects(access(path.join(root, 'target/.aether-desktop-build.lock')), { code: 'ENOENT' })
})

test('dev and standalone prepare keep the current version without a build lock', async t => {
  const { root, desktop, launch, stages } = await entrypointFixture(t)
  await launch(['dev', '--no-watch'])
  const [prepared, tauri] = await stages()
  assert.equal(prepared.build.debug, true)
  assert.deepEqual(tauri.args, ['dev', '--no-dev-server', '--no-watch', '--', '--locked'])
  for (const stage of [prepared, tauri]) {
    assert.equal(stage.locked, false)
    assert.equal(stage.version, '0.1.0')
  }
  await launch(['--debug'], {}, 'prepare.mjs')
  const standalone = await stages()
  assert.equal(standalone.length, 1)
  assert.equal(standalone[0].stage, 'prepare')
  assert.equal(standalone[0].version, '0.1.0')
  assert.equal(standalone[0].locked, false)
  assert.equal(JSON.parse(await readFile(path.join(desktop, 'package.json'), 'utf8')).version, '0.1.0')
  await assert.rejects(access(path.join(root, 'target/.aether-desktop-build.lock')), { code: 'ENOENT' })
})

for (const stage of ['prepare', 'tauri']) {
  test('failed ' + stage + ' preserves the allocated version and releases the entrypoint lock', async t => {
    const { root, desktop, launch } = await entrypointFixture(t)
    await assert.rejects(launch(['build'], { AETHER_VERSION_TEST_FAIL: stage }), error => {
      assert.equal(error.code, 1)
      assert.match(error.stderr, new RegExp('simulated ' + stage + ' failure'))
      return true
    })
    assert.equal(JSON.parse(await readFile(path.join(desktop, 'package.json'), 'utf8')).version, '0.1.1')
    await assert.rejects(access(path.join(root, 'target/.aether-desktop-build.lock')), { code: 'ENOENT' })
    await launch(['build'])
    assert.equal(JSON.parse(await readFile(path.join(desktop, 'package.json'), 'utf8')).version, '0.1.2')
  })
}
