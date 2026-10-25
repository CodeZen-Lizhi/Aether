// 验证真实批量模型映射工作台在最后一行/卡片打开候选时，Portal 浮层不被裁切。
// 只操作 fixture 草稿，不点击保存，也不写真实数据库。
const URL = process.env.PREVIEW_URL
  || 'http://127.0.0.1:5173/scripts/fixtures/model-mapping-preview.html?mode=batch'

function assert(condition, message) {
  if (!condition) throw new Error(message)
}

async function assertOverlay(modelName, inputSelector, viewport) {
  const page = browserPage
  await page.setViewportSize(viewport)
  const surface = viewport.width >= 900 ? 'table' : 'cards'
  const rowSelector = surface === 'table'
    ? '[data-layout-surface="table"] tbody tr'
    : '[data-layout-surface="cards"] > div'
  const surfaceSelector = surface === 'table'
    ? '[data-layout-surface="table"]'
    : '[data-layout-surface="cards"]'
  const addButton = page.locator(surfaceSelector)
    .locator(`[aria-label="为 ${modelName} 添加提供商模型"]`)
    .last()
  await addButton.click()
  const input = page.locator(surfaceSelector)
    .locator(`[data-mapping-add-input][aria-label="${modelName} 的提供商模型"]`)
  await input.waitFor({ state: 'visible' })

  const geometry = await page.evaluate(({ modelName, rowSelector }) => {
    const input = [...document.querySelectorAll('[data-mapping-add-input]')]
      .find(candidate => candidate.getAttribute('aria-label') === `${modelName} 的提供商模型`
        && candidate.getBoundingClientRect().width > 0)
    const listbox = input?.getAttribute('aria-controls')
      ? document.getElementById(input.getAttribute('aria-controls'))
      : null
    const portalContent = listbox?.closest('[data-radix-popper-content-wrapper]') ?? listbox
    const rows = [...document.querySelectorAll(rowSelector)]
    const inputRect = input?.getBoundingClientRect()
    const listboxRect = listbox?.getBoundingClientRect()
    const portalRect = portalContent?.getBoundingClientRect()
    return {
      rowCount: rows.length,
      listboxVisible: !!listboxRect && listboxRect.width > 0 && listboxRect.height > 0,
      inputTop: inputRect?.top ?? 0,
      listboxTop: listboxRect?.top ?? 0,
      listboxBottom: listboxRect?.bottom ?? 0,
      portalBottom: portalRect?.bottom ?? 0,
      insideViewport: !!listboxRect && listboxRect.top >= 0 && listboxRect.bottom <= window.innerHeight,
      notClippedByRow: !!inputRect && !!listboxRect
        && (listboxRect.top >= inputRect.top || listboxRect.bottom <= inputRect.top),
      pageOverflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
    }
  }, { modelName, rowSelector })

  assert(geometry.listboxVisible, `${viewport.width}px: ${modelName} 候选浮层不可见`)
  assert(geometry.insideViewport, `${viewport.width}px: 候选浮层超出视口 ${JSON.stringify(geometry)}`)
  assert(geometry.notClippedByRow, `${viewport.width}px: 候选浮层被行容器裁切 ${JSON.stringify(geometry)}`)
  assert(!geometry.pageOverflow, `${viewport.width}px: 候选浮层导致页面横向溢出`)

  const overlay = page.locator('[data-radix-popper-content-wrapper]').filter({
    has: page.locator(
      surface === 'table'
        ? '#batch-mapping-options-table-model-gpt-61-sol'
        : '#batch-mapping-options-cards-model-gpt-61-sol',
    ),
  })
  await overlay.getByRole('option').first().click()
  await page.getByRole('button', { name: '保存 1 项更改' }).waitFor()
  await page.getByRole('button', { name: '恢复为已保存状态' }).click()
  await page.waitForFunction(() => {
    const save = [...document.querySelectorAll('.app-dialog button')]
      .find(button => button.textContent?.trim() === '保存')
    return save?.disabled === true
  })
}

let browserPage

async function main() {
  const { chromium: playwrightChromium } = require('playwright')
  const browser = await playwrightChromium.launch()
  browserPage = await browser.newPage({ viewport: { width: 1200, height: 820 } })
  const consoleErrors = []
  browserPage.on('console', message => {
    if (message.type() === 'error') consoleErrors.push(message.text())
  })
  browserPage.on('pageerror', error => consoleErrors.push(error.message))

  try {
    await browserPage.goto(URL, { waitUntil: 'networkidle' })
    await browserPage.locator('[data-layout-surface="table"] tbody tr').first().waitFor()
    await assertOverlay('GPT-6.1 Sol', '#batch-mapping-add-table-model-gpt-61-sol', {
      width: 1200,
      height: 820,
    })
    await assertOverlay('GPT-6.1 Sol', '#batch-mapping-add-cards-model-gpt-61-sol', {
      width: 520,
      height: 760,
    })
    assert(consoleErrors.length === 0, `页面存在控制台错误：${consoleErrors.join(' | ')}`)
    console.log(JSON.stringify({
      status: 'passed',
      url: URL,
      scenarios: ['table-last-row', 'cards-last-row'],
      viewports: [{ width: 1200, height: 820 }, { width: 520, height: 760 }],
      previewWrites: await browserPage.evaluate(() => window.__modelMappingPreviewWrites?.length ?? 0),
      consoleErrors,
    }, null, 2))
  } finally {
    await browser.close()
  }
}

main().catch(error => {
  console.error(error.stack || error.message)
  process.exit(1)
})
