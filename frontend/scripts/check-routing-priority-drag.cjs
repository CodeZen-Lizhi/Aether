// 调度优先级真实拖拽检查：验证上下跨多项落点、落点线、按钮尺寸及边界禁用。
async (page) => {
  const fixtureOrigin = new URL(page.url()).origin
  if (!fixtureOrigin.startsWith('http://127.0.0.1:')) {
    throw new Error(`夹具入口不是本地 HTTP：${fixtureOrigin}`)
  }

  const rows = () => page.evaluate(() => (
    [...document.querySelectorAll('ul.provider-priority-list > li')].map(row => ({
      id: row.dataset.providerId ?? '',
      name: row.querySelector('.font-medium')?.textContent?.trim() ?? '',
      handle: row.querySelector('.cursor-grab')?.getBoundingClientRect().toJSON(),
      row: row.getBoundingClientRect().toJSON(),
    }))
  ))

  const assertOrder = (actual, expected, scene) => {
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
      throw new Error(`${scene}: 顺序错误，实际 ${JSON.stringify(actual)}`)
    }
  }

  const dragTo = async (source, targetY, scene) => {
    const startY = source.handle.y + source.handle.height / 2
    await page.mouse.move(source.handle.x + source.handle.width / 2, startY)
    await page.mouse.down()
    for (let step = 1; step <= 12; step += 1) {
      await page.mouse.move(
        source.handle.x + source.handle.width / 2,
        startY + (targetY - startY) * step / 12,
      )
      await page.waitForTimeout(20)
    }
    const dropIndicatorCount = await page.locator('.provider-drop-indicator').count()
    if (dropIndicatorCount !== 1) {
      throw new Error(`${scene}: 拖动中应只有一条落点线，实际 ${dropIndicatorCount}`)
    }
    await page.mouse.up()
    await page.waitForFunction(() => !document.querySelector('.provider-drag-preview'))
    await page.waitForTimeout(250)
  }

  await page.setViewportSize({ width: 1440, height: 1000 })
  await page.goto(
    `${fixtureOrigin}/scripts/fixtures/routing-priority-preview.html?test_run=${Date.now()}`,
    { waitUntil: 'domcontentloaded' },
  )
  await page.waitForSelector('ul.provider-priority-list > li')
  await page.waitForTimeout(200)

  let current = await rows()
  const before = current.map(row => row.name)
  await dragTo(current[6], current[0].row.top - 20, '向上跨多项')
  current = await rows()
  assertOrder(
    current.map(row => row.name),
    [before[6], ...before.slice(0, 6), ...before.slice(7)],
    '向上跨多项',
  )

  await dragTo(current[0], current.at(-1).row.bottom + 20, '向下跨多项')
  current = await rows()
  assertOrder(
    current.map(row => row.name),
    [...before.slice(0, 6), ...before.slice(7), before[6]],
    '向下跨多项',
  )

  const buttons = await page.evaluate(() => {
    const first = document.querySelector('ul.provider-priority-list > li')
    const last = [...document.querySelectorAll('ul.provider-priority-list > li')].at(-1)
    const firstUp = first?.querySelector('[aria-label^="上移"]')
    const firstDown = first?.querySelector('[aria-label^="下移"]')
    const lastUp = last?.querySelector('[aria-label^="上移"]')
    const lastDown = last?.querySelector('[aria-label^="下移"]')
    const rect = element => element?.getBoundingClientRect().toJSON() ?? null
    return {
      up: rect(firstUp),
      down: rect(firstDown),
      firstUpDisabled: firstUp?.disabled ?? false,
      firstDownDisabled: firstDown?.disabled ?? true,
      lastUpDisabled: lastUp?.disabled ?? true,
      lastDownDisabled: lastDown?.disabled ?? false,
    }
  })
  if (buttons.up?.width !== 44 || buttons.up?.height !== 40
    || buttons.down?.width !== 44 || buttons.down?.height !== 40) {
    throw new Error(`上下移动按钮尺寸错误：${JSON.stringify(buttons)}`)
  }
  if (!buttons.firstUpDisabled || buttons.firstDownDisabled
    || buttons.lastUpDisabled || !buttons.lastDownDisabled) {
    throw new Error(`上下移动按钮边界禁用错误：${JSON.stringify(buttons)}`)
  }

  return {
    before,
    afterUp: [before[6], ...before.slice(0, 6), ...before.slice(7)],
    afterDown: [...before.slice(0, 6), ...before.slice(7), before[6]],
    buttons,
  }
}
