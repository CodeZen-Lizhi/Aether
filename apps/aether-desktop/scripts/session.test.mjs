import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import vm from 'node:vm'

const origin = 'http://127.0.0.1:18084'
const secret = 'a'.repeat(64)
const script = readFileSync(new URL('../src-tauri/src/session.js', import.meta.url), 'utf8')
  .replace('__AETHER_SESSION_ORIGIN__', JSON.stringify(origin))
  .replace('__AETHER_SESSION_SECRET__', JSON.stringify(secret))

function load(url = origin, { subframe = false, response, failure } = {}) {
  const requests = []
  const timers = new Set()
  const window = {
    location: { origin: url },
    async fetch(...args) {
      requests.push(args)
      if (failure) throw failure
      return response ?? { ok: true, json: async () => ({ access_token: 'test-access-token' }) }
    },
  }
  window.top = subframe ? {} : window
  vm.runInNewContext(script, {
    window,
    AbortController,
    setTimeout(callback, delay) {
      const timer = setTimeout(callback, delay)
      timers.add(timer)
      return timer
    },
    clearTimeout(timer) {
      timers.delete(timer)
      clearTimeout(timer)
    },
  })
  return { window, requests, timers }
}

test('only the managed main-frame origin receives the session function', () => {
  for (const url of ['https://example.com', 'http://127.0.0.1:18085', 'tauri://localhost']) {
    const fixture = load(url)
    assert.equal(fixture.window.__AETHER_DESKTOP__, undefined)
    assert.equal(fixture.requests.length, 0)
  }
  assert.equal(load(origin, { subframe: true }).window.__AETHER_DESKTOP__, undefined)
})

test('session exchange uses only the fixed origin with redirects disabled', async () => {
  const { window, requests, timers } = load()
  window.fetch = () => { throw new Error('page replaced fetch') }
  const session = await window.__AETHER_DESKTOP__.authenticate('test-device')
  assert.equal(session.access_token, 'test-access-token')
  const [url, request] = requests[0]
  assert.equal(url, `${origin}/_gateway/desktop/session`)
  assert.equal(request.method, 'POST')
  assert.equal(request.mode, 'same-origin')
  assert.equal(request.credentials, 'same-origin')
  assert.equal(request.redirect, 'error')
  assert.equal(request.cache, 'no-store')
  assert.equal(request.headers['X-Aether-Desktop-Session'], secret)
  assert.equal(request.headers['X-Client-Device-Id'], 'test-device')
  assert.equal(timers.size, 0)
  assert.equal(Object.isFrozen(window.__AETHER_DESKTOP__), true)
  assert.equal(JSON.stringify(window.__AETHER_DESKTOP__).includes(secret), false)
})

test('changing origin after injection cannot send the capability', async () => {
  const { window, requests } = load()
  window.location.origin = 'https://example.com'
  await assert.rejects(window.__AETHER_DESKTOP__.authenticate('test-device'))
  assert.equal(requests.length, 0)
})

test('failed and malformed sessions reject and release their timeout', async () => {
  for (const options of [
    { response: { ok: false } },
    { response: { ok: true, json: async () => ({}) } },
    { failure: new Error('network unavailable') },
  ]) {
    const { window, timers } = load(origin, options)
    await assert.rejects(window.__AETHER_DESKTOP__.authenticate('test-device'))
    assert.equal(timers.size, 0)
  }
})
