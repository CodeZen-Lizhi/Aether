// 用真实密钥表单验证共用 Select 在容器查询弹窗内的定位和选择行为。
/* eslint-disable @typescript-eslint/no-require-imports -- 沿用现有浏览器检查脚本的 Node 入口。 */
const { chromium, webkit } = require('playwright')

/** 测量真实菜单与触发框的间距，并检查挂载层和视口边界。 */
async function inspectPosition(page, width, height) {
  await page.waitForFunction(() => {
    const trigger = document.querySelector('button[aria-label="倍率来源"]')
    const content = document.querySelector('[role="listbox"]')
    if (!trigger || !content) return false
    const triggerRect = trigger.getBoundingClientRect()
    const menuRect = content.getBoundingClientRect()
    const gap = content.dataset.side === 'top'
      ? triggerRect.top - menuRect.bottom
      : menuRect.top - triggerRect.bottom
    return Math.abs(menuRect.left - triggerRect.left) <= 2 && Math.abs(gap - 4) <= 2
  }, undefined, { timeout: 5000 })
  const geometry = await page.evaluate(() => {
    const triggerRect = document.querySelector('button[aria-label="倍率来源"]').getBoundingClientRect()
    const content = document.querySelector('[role="listbox"]')
    return {
      trigger: triggerRect.toJSON(),
      menu: content.getBoundingClientRect().toJSON(),
      side: content.dataset.side,
      insideDialog: !!content.closest('.app-dialog'),
      horizontalOverflow: document.documentElement.scrollWidth > window.innerWidth,
    }
  })
  if (geometry.insideDialog || geometry.horizontalOverflow
    || geometry.menu.left < 0 || geometry.menu.right > width
    || geometry.menu.top < 0 || geometry.menu.bottom > height) {
    throw new Error(`菜单挂载位置或窗口边界错误：${JSON.stringify(geometry)}`)
  }
  return geometry
}

/** 在所选浏览器中检查菜单锚定、边界避让和选择后的状态。 */
async function main() {
  const engine = process.env.PREVIEW_BROWSER === 'webkit' ? webkit : chromium
  const browser = await engine.launch()
  const page = await browser.newPage()
  const errors = []
  page.on('pageerror', error => errors.push(error.message))

  try {
    for (const [width, height] of [[840, 620], [900, 640], [1024, 768], [1280, 800], [1440, 900], [1920, 1080], [390, 844], [639, 844], [640, 844]]) {
      await page.setViewportSize({ width, height })
      await page.goto(process.env.PREVIEW_URL
        || 'http://127.0.0.1:5173/scripts/fixtures/key-form-dialog.html')
      const trigger = page.getByRole('combobox', { name: '倍率来源' })
      await trigger.click()
      const menu = page.getByRole('listbox')
      await menu.waitFor({ state: 'visible' })
      await page.waitForFunction(() => {
        const content = document.querySelector('[role="listbox"]')
        return content && content.getAnimations().every(animation => animation.playState === 'finished')
      })

      const geometry = await inspectPosition(page, width, height)
      if (width === 1920) {
        for (let resizeWidth = 1860; resizeWidth >= 900; resizeWidth -= 60) {
          await page.setViewportSize({ width: resizeWidth, height: 640 })
          await inspectPosition(page, resizeWidth, 640)
        }
        await page.setViewportSize({ width, height })
        await inspectPosition(page, width, height)
        console.log('菜单打开时 1920 → 900 → 1920 缩放检查通过，缩小每步 60px')
      }

      await page.getByRole('option', { name: '跟随上游', exact: true }).click()
      await menu.waitFor({ state: 'detached' })
      if (!(await trigger.innerText()).includes('上游')) {
        throw new Error('选择后触发框未更新')
      }
      await trigger.click()
      await menu.waitFor({ state: 'visible' })
      await page.getByRole('option', { name: '手动', exact: true }).click()
      await menu.waitFor({ state: 'detached' })
      if (!(await trigger.innerText()).includes('手动')) {
        throw new Error('重新选择后触发框未更新')
      }
      if (!(await page.locator('.app-dialog').isVisible())) {
        throw new Error('选择菜单选项意外关闭了弹窗')
      }
      console.log(JSON.stringify({ width, height, ...geometry }))
    }
    if (errors.length) throw new Error(errors.join(' | '))
  } finally {
    await browser.close()
  }
}

main().catch(error => {
  console.error(error.stack || error)
  process.exitCode = 1
})
