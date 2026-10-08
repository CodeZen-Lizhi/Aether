// 在供应商间距预览页执行，使用真实 Table 和 ProviderTableRow 验证连续缩放。
async (page) => {
  await page.goto('http://localhost:5173/scripts/fixtures/provider-spacing-preview.html')
  await page.locator('tbody tr').first().waitFor({ state: 'attached' })
  for (const width of [840, 1024, 1280, 1920, 2560, 1024]) {
    await page.setViewportSize({ width, height: 900 })
    const result = await page.evaluate(() => {
      const table = document.querySelector('table')
      const container = document.querySelector('.provider-list')
      const rect = table.getBoundingClientRect()
      const cells = [...document.querySelector('tbody tr').children].map(cell => cell.getBoundingClientRect())
      const heads = [...document.querySelector('thead tr').children].map(cell => cell.getBoundingClientRect())
      return {
        visible: getComputedStyle(table.closest('.responsive-list-table')).display !== 'none',
        containerWidth: container.clientWidth,
        tableWidth: rect.width,
        overflow: document.documentElement.scrollWidth > innerWidth,
        aligned: cells.every((cell, i) => Math.abs(cell.x - heads[i].x) < 1 && Math.abs(cell.width - heads[i].width) < 1),
        actionsRight: cells[5].right,
        tableRight: rect.right,
        buttonsFit: [...document.querySelectorAll('tbody tr')].every(row => {
          const cell = row.children[5].getBoundingClientRect()
          return [...row.children[5].querySelectorAll('button')].every(button => {
            const bounds = button.getBoundingClientRect()
            return bounds.left >= cell.left && bounds.right <= cell.right
          })
        }),
      }
    })
    if (result.overflow) throw new Error(`${width}: 页面横向溢出`)
    if (result.visible) {
      if (Math.abs(result.tableWidth - result.containerWidth) > 2 || !result.aligned || !result.buttonsFit || Math.abs(result.actionsRight - result.tableRight) > 2) {
        throw new Error(`${width}: 列宽或对齐错误 ${JSON.stringify(result)}`)
      }
    } else if (width >= 1024) throw new Error(`${width}: 表格意外隐藏`)
    console.log(JSON.stringify({ width, ...result }))
  }
  await page.getByRole('button', { name: '测试长名称' }).click()
  await page.setViewportSize({ width: 1024, height: 900 })
  if (await page.locator('tbody tr').nth(1).evaluate(row => row.scrollWidth > row.clientWidth)) throw new Error('长名称导致行溢出')
  await page.setViewportSize({ width: 1920, height: 1080 })
  await page.screenshot({ path: '.cache/delivery/provider-layout/wide.png', fullPage: true })
}
