import test from 'node:test'
import assert from 'node:assert/strict'
import { binaryPath, options } from './build-common.mjs'

test('native and explicit cross builds resolve the matching sidecar output', () => {
  assert.equal(binaryPath('/tmp/target', options([])), '/tmp/target/release/aether-gateway')
  assert.equal(binaryPath('/tmp/target', options(['--target', 'x86_64-apple-darwin', '--debug'])), '/tmp/target/x86_64-apple-darwin/debug/aether-gateway')
  assert.equal(binaryPath('/tmp/target', options([], true)), '/tmp/target/debug/aether-gateway')
})

test('build flags cannot accidentally select an unsupported sidecar architecture', () => {
  for (const args of [['--target'], ['--target', '../../elsewhere'], ['--target', 'universal-apple-darwin'], ['--config']]) {
    assert.throws(() => options(args))
  }
  assert.deepEqual(options(['--config', '/tmp/signing.json']).tauri, ['--config', '/tmp/signing.json'])
})
