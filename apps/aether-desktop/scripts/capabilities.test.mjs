import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const tauriRoot = new URL('../src-tauri/', import.meta.url)

function read(relativePath) {
  return readFileSync(new URL(relativePath, tauriRoot), 'utf8')
}

test('desktop IPC commands are allowed for both managed desktop surfaces', () => {
  const commandSource = read('src/commands.rs')
  const commandNames = [...commandSource.matchAll(/pub (?:async )?fn (desktop_[a-z_]+)/g)]
    .map(([, name]) => name)
    .sort()
  const permissionSource = read('permissions/desktop-control.toml')
  const allowBlock = permissionSource.match(/commands\.allow = \[([\s\S]*?)\]/)?.[1]
  const allowedCommands = [...(allowBlock ?? '').matchAll(/"(desktop_[a-z_]+)"/g)]
    .map(([, name]) => name)
    .sort()

  assert.ok(commandNames.length > 0)
  assert.deepEqual(allowedCommands, commandNames)

  for (const capability of ['desktop-local.json', 'desktop-dashboard.json']) {
    const manifest = JSON.parse(read(`capabilities/${capability}`))
    assert.ok(manifest.permissions.includes('desktop-control'), `${capability} cannot invoke desktop IPC commands`)
  }
})
