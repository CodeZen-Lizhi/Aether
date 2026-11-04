import test from 'node:test'
import assert from 'node:assert/strict'
import {
  LOCAL_SIGNING_CONFIG,
  LOCAL_SIGNING_IDENTITY,
  signingConfig,
} from './local-signing.mjs'

test('local signing config uses a stable named identity and lives outside source inputs', () => {
  assert.deepEqual(signingConfig(), {
    bundle: { macOS: { signingIdentity: LOCAL_SIGNING_IDENTITY } },
  })
  assert.match(LOCAL_SIGNING_CONFIG, /apps\/aether-desktop\/signing\.local\.json$/)
})
