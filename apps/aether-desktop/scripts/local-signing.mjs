import { execFileSync } from 'node:child_process'
import { constants } from 'node:fs'
import { access, chmod, copyFile, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
import { randomBytes } from 'node:crypto'
import { desktop } from './build-common.mjs'

export const LOCAL_SIGNING_IDENTITY = 'Aether Local Signing'
export const LOCAL_SIGNING_CONFIG = path.join(desktop, 'signing.local.json')

/**
 * 返回本地稳定签名配置；该身份只用于本机重建，不能替代 Developer ID 分发签名。
 */
export function signingConfig(identity = LOCAL_SIGNING_IDENTITY) {
  return { bundle: { macOS: { signingIdentity: identity } } }
}

/**
 * 判断构建脚本是否已有本地稳定签名配置；证书可用性由初始化脚本单独校验。
 */
export async function hasLocalSigningConfig() {
  try {
    await access(LOCAL_SIGNING_CONFIG, constants.R_OK)
    return true
  } catch {
    return false
  }
}

/**
 * 获取当前用户的默认钥匙串，避免把本地签名写入系统钥匙串或要求管理员授权。
 */
function loginKeychain() {
  const value = run('security', ['default-keychain', '-d', 'user']).trim()
  return value.replace(/^"|"$/g, '')
}

/**
 * 执行命令并只保留 stderr 摘要，避免错误信息回显命令行中的临时密码。
 */
function run(command, args, options = {}) {
  try {
    return execFileSync(command, args, {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
      ...options,
    })
  } catch (error) {
    const detail = String(error.stderr || error.stdout || '').trim()
    throw new Error(`${command} 执行失败${detail ? `：${detail}` : ''}`)
  }
}

/**
 * 检查证书是否已存在；存在时绝不重新生成，否则证书根变化会让钥匙串再次失去授权。
 */
function certificateExists() {
  try {
    run('security', ['find-certificate', '-c', LOCAL_SIGNING_IDENTITY, '-p', loginKeychain()])
    return true
  } catch {
    return false
  }
}

/**
 * 用临时 Mach-O 副本验证证书和私钥都可被 codesign 使用，不触碰应用产物。
 */
async function verifyIdentity(directory) {
  const probe = path.join(directory, 'signing-probe')
  await copyFile('/bin/echo', probe)
  run('codesign', [
    '--force',
    '--sign',
    LOCAL_SIGNING_IDENTITY,
    '--keychain',
    loginKeychain(),
    probe,
  ])
}

/**
 * 创建只用于本机的自签名代码签名身份；私钥只导入登录钥匙串并授权 codesign 使用。
 */
async function createIdentity() {
  const directory = await mkdtemp(path.join(tmpdir(), 'aether-local-signing-'))
  try {
    const key = path.join(directory, 'key.pem')
    const certificate = path.join(directory, 'certificate.pem')
    const archive = path.join(directory, 'identity.p12')
    const password = randomBytes(24).toString('base64url')
    run('openssl', [
      'req',
      '-x509',
      '-newkey',
      'rsa:2048',
      '-sha256',
      '-days',
      '3650',
      '-nodes',
      '-keyout',
      key,
      '-out',
      certificate,
      '-subj',
      `/CN=${LOCAL_SIGNING_IDENTITY}/O=Aether Local`,
      '-addext',
      'basicConstraints=critical,CA:TRUE',
      '-addext',
      'keyUsage=critical,digitalSignature,keyCertSign',
      '-addext',
      'extendedKeyUsage=codeSigning',
    ])

    const opensslVersion = run('openssl', ['version']).trim()
    const opensslMajor = Number(/^OpenSSL (\d+)/.exec(opensslVersion)?.[1] ?? 0)
    const exportArgs = ['pkcs12', '-export']
    // OpenSSL 3 默认算法无法被 macOS security 导入，需显式使用兼容格式。
    if (opensslMajor >= 3) exportArgs.push('-legacy')
    exportArgs.push(
      '-descert',
      '-out',
      archive,
      '-inkey',
      key,
      '-in',
      certificate,
      '-passout',
      'env:AETHER_LOCAL_SIGNING_PASSWORD',
      '-name',
      LOCAL_SIGNING_IDENTITY,
    )
    run('openssl', exportArgs, {
      env: { ...process.env, AETHER_LOCAL_SIGNING_PASSWORD: password },
    })
    run('security', [
      'import',
      archive,
      '-k',
      loginKeychain(),
      '-P',
      password,
      '-T',
      '/usr/bin/codesign',
    ])
    await verifyIdentity(directory)
  } finally {
    await rm(directory, { recursive: true, force: true })
  }
}

/**
 * 确保本地稳定签名可用并写入构建配置；已有身份只校验，不重新生成。
 */
export async function ensureLocalSigning() {
  if (certificateExists()) {
    const directory = await mkdtemp(path.join(tmpdir(), 'aether-local-signing-check-'))
    try {
      await verifyIdentity(directory)
    } catch {
      throw new Error(
        `证书“${LOCAL_SIGNING_IDENTITY}”已存在，但私钥不可用。请在“钥匙串访问”中删除该证书和私钥后重新运行。`,
      )
    } finally {
      await rm(directory, { recursive: true, force: true })
    }
  } else {
    await createIdentity()
  }

  const bytes = `${JSON.stringify(signingConfig(), null, 2)}\n`
  await writeFile(LOCAL_SIGNING_CONFIG, bytes, { mode: 0o600 })
  await chmod(LOCAL_SIGNING_CONFIG, 0o600)
  return LOCAL_SIGNING_IDENTITY
}

async function main() {
  if (process.argv[2] === '--check') {
    if (!(await hasLocalSigningConfig())) process.exitCode = 1
    return
  }
  const identity = await ensureLocalSigning()
  console.log(`本地稳定签名已就绪：${identity}`)
  console.log(`构建配置：${LOCAL_SIGNING_CONFIG}`)
  console.log('首次用该签名启动时，请允许 Aether 访问钥匙串并选择“始终允许”；后续重装不再重复询问。')
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main().catch(error => {
    console.error(error.message)
    process.exitCode = 1
  })
}
