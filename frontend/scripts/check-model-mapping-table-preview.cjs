// 在挂载正式 BatchModelMappingDialog 的预览页执行真实浏览器布局与交互断言。
// 验收覆盖 Aether 默认窗口、原生最小窗口、两套主题、搜索筛选、添加、清空、隐藏和恢复基线。
const { chromium } = require('playwright')

const URL = process.env.PREVIEW_URL
  || 'http://127.0.0.1:5173/scripts/fixtures/model-mapping-preview.html?mode=batch'

function assert(condition, message) {
  if (!condition) throw new Error(message)
}

async function main() {
  const browser = await chromium.launch()
  const page = await browser.newPage({ viewport: { width: 1200, height: 820 } })
  const consoleErrors = []
  page.on('console', message => {
    if (message.type() === 'error') consoleErrors.push(message.text())
  })
  page.on('pageerror', error => consoleErrors.push(error.message))

  try {
    await page.goto(URL, { waitUntil: 'networkidle' })
    await page.getByRole('heading', { name: '批量添加模型映射' }).waitFor()
    await page.locator('[data-layout-surface="table"] tbody tr').first().waitFor()
    assert(
      await page.locator('.app-dialog tbody tr').count() === 4,
      '打开批量映射后没有展示全部 4 个客户端模型',
    )

    const viewports = [
      { width: 1200, height: 820 },
      { width: 900, height: 640 },
      { width: 520, height: 760 },
    ]

    for (const viewport of viewports) {
      await page.setViewportSize(viewport)
      const layout = await page.evaluate(() => {
        const dialog = document.querySelector('.app-dialog')
        const tableSurface = dialog?.querySelector('[data-layout-surface="table"]')
        const cardsSurface = dialog?.querySelector('[data-layout-surface="cards"]')
        const table = tableSurface?.querySelector('table')
        const save = [...document.querySelectorAll('.app-dialog button')]
          .find(button => button.textContent?.trim().startsWith('保存'))
        const close = [...document.querySelectorAll('.app-dialog button')]
          .find(button => button.textContent?.trim() === '关闭')
        const rect = element => element?.getBoundingClientRect()
        const saveRect = rect(save)
        const closeRect = rect(close)
        return {
          pageOverflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
          tableOverflow: !!table && table.scrollWidth > table.clientWidth,
          tableClientWidth: table?.clientWidth ?? 0,
          tableVisible: !!tableSurface && getComputedStyle(tableSurface).display !== 'none',
          cardsVisible: !!cardsSurface && getComputedStyle(cardsSurface).display !== 'none',
          saveVisible: !!saveRect && saveRect.top >= 0 && saveRect.bottom <= window.innerHeight,
          closeVisible: !!closeRect && closeRect.top >= 0 && closeRect.bottom <= window.innerHeight,
          saveText: save?.textContent?.trim(),
        }
      })
      const label = `${viewport.width}x${viewport.height}`
      assert(!layout.pageOverflow, `${label}: 页面横向溢出`)
      assert(!layout.tableOverflow, `${label}: 表格横向溢出（${JSON.stringify(layout)}）`)
      assert(
        layout.tableVisible !== layout.cardsVisible,
        `${label}: 表格与卡片必须恰好显示一种（${JSON.stringify(layout)}）`,
      )
      if (layout.tableVisible) {
        assert(layout.tableClientWidth > 0, `${label}: 表格未取得有效宽度`)
      }
      assert(layout.closeVisible, `${label}: 关闭按钮不可达`)
      assert(layout.saveVisible, `${label}: 保存按钮不可达`)
      assert(layout.saveText === '保存', `${label}: 初始保存状态异常 ${layout.saveText}`)
    }

    // 后续操作在主表格宽度执行，避免命中窄屏卡片里隐藏的那一份控件。
    await page.setViewportSize({ width: 1200, height: 820 })
    const searchInput = page.getByPlaceholder('搜索客户端或提供商模型…')
    await searchInput.fill('astra')
    await page.waitForFunction(() => document.querySelectorAll('.app-dialog tbody tr').length === 1)
    assert(
      await page.locator('.app-dialog tbody tr').first().innerText().then(text => text.includes('GPT-6 Astra')),
      '搜索 astra 没有只保留 GPT-6 Astra',
    )
    await searchInput.fill('')

    await page.getByRole('button', { name: '未映射 1' }).click()
    await page.waitForFunction(() => document.querySelectorAll('.app-dialog tbody tr').length === 1)
    assert(
      await page.locator('.app-dialog tbody tr').first().innerText().then(text => text.includes('GPT-6.1 Sol')),
      '未映射筛选没有只保留 GPT-6.1 Sol',
    )
    await page.getByRole('button', { name: '全部 4' }).click()

    await page.locator('[data-layout-surface="table"]')
      .locator('[aria-label="为 GPT-5.6 Sol 添加提供商模型"]')
      .last()
      .click()
    // 表格与卡片会同时挂载，浏览器操作必须限定到当前可见的表格 surface。
    const addInput = page.locator(
      '[data-layout-surface="table"] [data-mapping-add-input][aria-label="GPT-5.6 Sol 的提供商模型"]',
    )
    await addInput.waitFor({ state: 'visible' })
    await addInput.fill('gpt-5.6-mini')
    await addInput.press('Enter')
    const saveChanged = page.getByRole('button', { name: '保存 1 项更改' })
    await saveChanged.waitFor()
    const addedRow = await page.locator('.app-dialog tbody tr').first().innerText()
    assert(addedRow.includes('gpt-5.6-mini'), '新增映射没有显示在表格中')
    assert(addedRow.includes('已映射 2'), '新增映射后数量没有变为 2')
    assert(
      await page.locator('.app-dialog').innerText()
        .then(text => text.includes('1 个客户端模型有待保存更改')),
      '页脚没有显示待保存更改',
    )

    await page.evaluate(() => document.documentElement.classList.add('dark'))
    await page.locator('body').evaluate(element => element.setAttribute('theme-mode', 'dark'))
    const dark = await page.evaluate(() => {
      const save = [...document.querySelectorAll('.app-dialog button')]
        .find(button => button.textContent?.trim().startsWith('保存'))
      const saveRect = save?.getBoundingClientRect()
      return {
        pageOverflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
        saveVisible: !!saveRect && saveRect.top >= 0 && saveRect.bottom <= window.innerHeight,
        dialogBackground: getComputedStyle(document.querySelector('.app-dialog')).backgroundColor,
      }
    })
    assert(!dark.pageOverflow, '深色主题下页面横向溢出')
    assert(dark.saveVisible, '深色主题下保存按钮不可达')
    assert(
      dark.dialogBackground !== 'rgba(0, 0, 0, 0)' && dark.dialogBackground !== 'transparent',
      `深色主题未生效：${dark.dialogBackground}`,
    )

    await page.evaluate(() => document.documentElement.classList.remove('dark'))
    await page.locator('body').evaluate(element => element.setAttribute('theme-mode', 'light'))
    await page.getByRole('button', { name: '清空 GPT-5.6 Sol 的映射' }).click()
    await page.waitForFunction(() => {
      const text = document.querySelector('.app-dialog')?.innerText ?? ''
      return text.includes('1 个客户端模型有待保存更改')
    })

    await page.getByRole('button', { name: '隐藏 GPT-6 Astra' }).click()
    assert(
      await page.locator('.app-dialog tbody tr').count() === 3,
      '隐藏模型后表格行数没有减少',
    )
    await page.getByText('显示已隐藏（1）').click()
    await page.getByRole('button', { name: '显示 GPT-6 Astra' }).click()
    assert(
      await page.locator('.app-dialog tbody tr').count() === 4,
      '恢复模型后表格行数没有恢复',
    )

    await page.getByTitle('恢复为已保存状态').click()
    await page.waitForFunction(() => {
      const save = [...document.querySelectorAll('.app-dialog button')]
        .find(button => button.textContent?.trim() === '保存')
      return save?.disabled === true
    })
    await page.goto(`${URL}${URL.includes('?') ? '&' : '?'}locale=en`, { waitUntil: 'networkidle' })
    await page.getByRole('heading', { name: 'Add model mappings in bulk' }).waitFor()
    const english = await page.locator('.app-dialog').innerText()
    for (const text of [
      'All 4',
      'Mapped 3',
      'Unmapped 1',
      'Modified 0',
      'Show hidden (0)',
    ]) {
      assert(english.includes(text), `英文工作台缺少文案：${text}`)
    }
    assert(
      await page.getByRole('button', { name: 'Refresh provider models' }).count() === 1,
      '英文刷新提供商模型按钮缺少可访问名称',
    )
    assert(
      await page.getByRole('button', { name: 'Restore saved state' }).count() === 1,
      '英文恢复已保存状态按钮缺少可访问名称',
    )
    assert(
      await page.getByRole('group', { name: 'Mapping status filters' }).count() === 1,
      '英文映射状态筛选组缺少可访问名称',
    )
    await page.getByRole('button', { name: 'Unmapped 1' }).click()
    await page.waitForFunction(() => document.querySelectorAll('.app-dialog tbody tr').length === 1)
    assert(
      (await page.locator('.app-dialog').innerText()).includes('Add provider model'),
      '英文未映射筛选后的空状态或添加入口不完整',
    )
    await page.locator('[data-layout-surface="table"]')
      .locator('[aria-label="Add a provider model for GPT-6.1 Sol"]')
      .last()
      .click()
    const englishInput = page.locator(
      '[data-layout-surface="table"] [data-mapping-add-input][aria-label="Provider models for GPT-6.1 Sol"]',
    )
    await englishInput.waitFor({ state: 'visible' })
    await englishInput.fill('gpt-6.1-pro')
    await englishInput.press('Enter')
    await page.getByRole('button', { name: 'All 4' }).click()
    assert(
      (await page.locator('.app-dialog').innerText()).includes('1 pending change'),
      '英文工作台新增映射后待保存统计不正确',
    )
    assert(
      await page.getByRole('button', { name: 'Save', exact: true }).isEnabled(),
      '英文工作台新增映射后保存按钮不可用',
    )
    await page.getByRole('button', { name: 'Restore saved state' }).click()
    assert(
      !/[已映射未映射已修改已隐藏只看已修改显示已隐藏恢复为已保存状态]/.test(english),
      `英文工作台仍包含未翻译中文：${english}`,
    )
    assert(
      await page.evaluate(() => window.__modelMappingPreviewWrites?.length === 0),
      '预览交互意外触发了保存请求',
    )
    assert(consoleErrors.length === 0, `页面存在控制台错误：${consoleErrors.join(' | ')}`)

    console.log(JSON.stringify({
      status: 'passed',
      url: URL,
      viewports,
      interactions: ['search', 'status-filter', 'add', 'dark-theme', 'clear', 'hide-restore', 'reset', 'english'],
      previewWrites: 0,
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
