import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { root, desktop, run } from './build-common.mjs'

/** 解析交付参数；构建参数仅在显式分隔符后交给原有构建入口。 */
export function deliveryOptions(args, env = process.env) {
  const result = { build: false, gate: env.DELIVERY_GATE, python: env.PYTHON ?? 'python3', buildArgs: [] }
  for (let index = 0; index < args.length; index++) {
    const arg = args[index]
    if (arg === '--build') result.build = true
    else if (arg === '--') { result.buildArgs = args.slice(index + 1); break }
    else if (['--plan', '--output', '--gate', '--final'].includes(arg)) {
      const value = args[++index]
      if (!value || value.startsWith('--')) throw new Error(`${arg} 缺少参数`)
      result[arg.slice(2)] = value
    } else throw new Error(`不支持的交付参数：${arg}`)
  }
  if (!result.plan || !result.output || !result.gate) {
    throw new Error('需要 --plan、--output 和 --gate（也可用 DELIVERY_GATE 指定共享工具）。')
  }
  if (!result.build && result.buildArgs.length) throw new Error('验收已有产物时不能传构建参数。')
  for (const key of ['plan', 'output', 'gate', 'final']) {
    if (result[key]) result[key] = path.resolve(root, result[key])
  }
  return result
}

/** 对当前项目先校验清单，再构建候选包并验收；任一步失败立即停止。 */
export async function verifyDelivery(config, runner = run) {
  const plan = JSON.parse(await readFile(config.plan, 'utf8'))
  if (path.resolve(plan.root ?? '') !== root) throw new Error('清单 root 必须是当前 Aether 仓库的绝对路径。')
  if (plan.mode !== 'behavior') throw new Error('桌面交付必须使用 behavior 验收清单。')
  if (config.build && (!plan.artifacts?.length || !plan.checks?.some(check => check.kind === 'artifact'))) {
    throw new Error('已验证构建需要声明 artifacts 并检查实际构建产物。')
  }
  await runner(config.python, [config.gate, 'validate', config.plan], root, process.env)
  const args = [config.gate, 'run', config.plan, '--output', config.output]
  if (config.build) args.push('--prepare', process.execPath, path.join(desktop, 'scripts/desktop.mjs'), 'build', ...config.buildArgs)
  await runner(config.python, args, root, process.env)
  const verify = [config.gate, 'check', config.plan, '--output', config.output]
  if (config.final) verify.push('--final', config.final)
  await runner(config.python, verify, root, process.env)
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  verifyDelivery(deliveryOptions(process.argv.slice(2))).catch(error => {
    console.error(`交付未通过：${error.message}`)
    process.exitCode = 1
  })
}
