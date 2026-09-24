async (page) => {
  await page.goto('http://127.0.0.1:5175/admin/system?tab=general')
  await page.getByRole('button', { name: '创建备份', exact: true }).waitFor()
  await page.evaluate(() => document.fonts.ready)
  const results = []
  for (const [width, height] of [[900, 640], [1024, 768], [1200, 820], [1440, 900], [1920, 1080]]) {
    await page.setViewportSize({ width, height })
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))
    const metrics = await page.evaluate(() => {
      const visible = el => el.getClientRects().length > 0 && getComputedStyle(el).visibility !== 'hidden'
      const root = document.querySelector('.system-settings')
      const rect = el => { const r = el.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height, bottom: r.bottom } }
      const overflows = [document.documentElement, ...root.querySelectorAll('*')]
        .filter(el => visible(el) && el.clientWidth > 0 && el.scrollWidth > el.clientWidth + 1)
        .map(el => ({ tag: el.tagName, class: el.className, client: el.clientWidth, scroll: el.scrollWidth }))
      const controls = ['#default-proxy', '#gateway-port', '#desktop-autostart'].map(selector => {
        const el = root.querySelector(selector)
        return { selector, rect: rect(el), font: getComputedStyle(el).fontSize }
      })
      const actions = [...root.querySelectorAll('button,a')].filter(el => visible(el) && /创建备份|恢复备份|高级设置/.test(el.textContent))
      return { content: rect(root), headingFont: getComputedStyle(root.querySelector('h1')).fontSize, overflows, controls,
        actions: actions.map(el => ({ text: el.textContent.trim(), ...rect(el) })),
        columns: [...root.querySelectorAll('.settings-row')].filter(visible).map(el => getComputedStyle(el).gridTemplateColumns),
        screen: { w: screen.availWidth, h: screen.availHeight },
      }
    })
    if (metrics.overflows.length) throw new Error(JSON.stringify({ width, overflows: metrics.overflows }))
    if (width === 900 && metrics.actions.some(action => action.bottom > height)) throw new Error('Minimum window common actions below viewport')
    await page.screenshot({ path: `output/playwright/settings-stable-${width}.png`, scale: 'css' })
    results.push({ width, height, ...metrics })
  }
  for (const width of [1180, 1080, 1030, 1024, 1000, 960, 940, 920, 900, 920, 1000, 1200]) {
    await page.setViewportSize({ width, height: 640 })
    const result = await page.evaluate(() => {
      const root = document.querySelector('.system-settings')
      return {
        title: root.querySelector('h1').textContent,
        display: [...root.querySelectorAll('.settings-row')].filter(el => el.getClientRects().length).map(el => getComputedStyle(el).gridTemplateColumns),
        overflow: root.scrollWidth > root.clientWidth + 1,
      }
    })
    if (result.overflow || result.title.trim() !== '系统设置' || result.display.some(columns => columns.split(' ').length !== 2)) throw new Error(JSON.stringify({ width, result }))
  }
  await page.setViewportSize({ width: 900, height: 640 })
  return { viewports: results, continuousResize: 'passed' }
}
