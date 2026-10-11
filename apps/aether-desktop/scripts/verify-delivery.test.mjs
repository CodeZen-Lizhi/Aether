import test from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, writeFile, rm } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { root } from './build-common.mjs'
import { deliveryOptions, verifyDelivery } from './verify-delivery.mjs'

/** 用临时清单验证调度契约，不触发真实打包和版本递增。 */
async function fixture(t, fields = {}) {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'aether-delivery-'))
  t.after(() => rm(dir, { recursive: true, force: true }))
  const plan = path.join(dir, 'plan.json')
  await writeFile(plan, JSON.stringify({ root, mode: 'behavior', artifacts: ['app'], checks: [{ kind: 'artifact' }], ...fields }))
  return deliveryOptions(['--plan', plan, '--output', path.join(dir, 'evidence'), '--gate', '/test/gate.py'], {})
}

test('缺少清单、工具或参数时明确拒绝，路径按项目根解析', () => {
  for (const args of [[], ['--plan'], ['--unknown'], ['--plan', 'x', '--output', 'y']]) {
    assert.throws(() => deliveryOptions(args, {}))
  }
  const config = deliveryOptions(['--plan', 'task.json', '--output', '.cache/run'], { DELIVERY_GATE: '/gate.py' })
  assert.equal(config.plan, path.join(root, 'task.json'))
  assert.equal(config.gate, '/gate.py')
})

test('清单失败不能执行构建，验收失败不能执行最终复核', async t => {
  const config = await fixture(t)
  config.build = true
  for (const failAt of ['validate', 'run']) {
    const calls = []
    await assert.rejects(verifyDelivery(config, async (command, args) => {
      calls.push(args[1])
      if (args[1] === failAt) throw new Error('模拟失败')
    }))
    assert.deepEqual(calls, failAt === 'validate' ? ['validate'] : ['validate', 'run'])
  }
})

test('已验证构建传入原有构建入口并复核交付草稿', async t => {
  const config = await fixture(t)
  Object.assign(config, { build: true, buildArgs: ['--bundles', 'app'], final: '/draft.md' })
  const calls = []
  await verifyDelivery(config, async (command, args, cwd) => { calls.push(args); assert.equal(cwd, root) })
  assert.deepEqual(calls.map(args => args[1]), ['validate', 'run', 'check'])
  assert.deepEqual(calls[1].slice(-5), [process.execPath, path.join(root, 'apps/aether-desktop/scripts/desktop.mjs'), 'build', '--bundles', 'app'])
  assert.deepEqual(calls[2].slice(-2), ['--final', '/draft.md'])
})

test('已有产物只做验收；错误项目、静态清单和缺少产物检查不能冒充验证构建', async t => {
  const config = await fixture(t)
  const calls = []
  await verifyDelivery(config, async (command, args) => { calls.push(args) })
  assert.equal(calls[1].includes('--prepare'), false)
  for (const fields of [{ root: '/another-project' }, { mode: 'static' }, { artifacts: [] }, { checks: [{ kind: 'static' }] }]) {
    const invalid = await fixture(t, fields)
    invalid.build = true
    await assert.rejects(verifyDelivery(invalid, async () => { assert.fail('无效计划不得执行命令') }))
  }
})
