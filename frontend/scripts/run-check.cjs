// 通用夹具检查执行器：node scripts/run-check.cjs scripts/check-xxx.cjs [url]
// 复用 playwright-cli 约定写法的检查函数，避免为每次回归单独写启动脚本。
const fs = require('node:fs')
const path = require('node:path')

async function main() {
  const [checkFile, url] = process.argv.slice(2)
  if (!checkFile) throw new Error('缺少检查脚本路径')
  const target = url || 'http://localhost:5173/scripts/fixtures/provider-detail-tabs.html'
  const source = fs.readFileSync(path.resolve(checkFile), 'utf8')
  const check = new Function(`return (${source})`)()
  const { chromium } = require('playwright')
  const browser = await chromium.launch()
  const page = await browser.newPage()
  await page.goto(target)
  try {
    console.log('PASS', JSON.stringify(await check(page), null, 2))
  } catch (error) {
    console.log('FAIL', error.stack)
    process.exitCode = 1
  } finally {
    await browser.close()
  }
}

main()
