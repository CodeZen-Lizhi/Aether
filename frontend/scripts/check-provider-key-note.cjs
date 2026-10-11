// 打开 /scripts/fixtures/provider-detail-tabs.html 后，用 playwright-cli run-code --filename 执行。
// 挂载真实 ProviderDetailDrawer 与真实 CSS，验证密钥备注的行内展示与详情浮层。
// eslint-disable-next-line @typescript-eslint/no-unused-expressions -- playwright-cli 执行此函数。
async (page) => {
  const failures = []
  page.on('pageerror', error => failures.push(error.message))
  await page.reload()
  await page.setViewportSize({ width: 1280, height: 900 })
  await page.getByRole('button', { name: '添加密钥', exact: true }).waitFor()

  function rowOf(name) {
    return page.locator('[class~="group/item"]').filter({ hasText: name }).first()
  }

  // 抽屉里同时存在代理节点等其他浮层，必须按详情浮层自身标记定位，不能依赖 DOM 顺序。
  function detailPopover() {
    return page.locator('[data-radix-popper-content-wrapper]')
      .filter({ has: page.getByTestId('provider-key-detail-edit') })
      .last()
  }

  /** 打开并定位详情浮层；已打开时直接复用，避免重复点击把它收起。 */
  async function openDetail(name) {
    const row = rowOf(name)
    await row.waitFor()
    const popover = detailPopover()
    if (!await popover.isVisible()) await row.getByTestId('provider-key-detail-trigger').click()
    await popover.waitFor()
    return { row, popover }
  }

  /** 收起详情浮层；Escape 会连带关闭供应商抽屉导致密钥条目重渲染，必须点触发按钮。 */
  async function closeDetail(row, popover) {
    if (!await popover.isVisible()) return
    await row.getByTestId('provider-key-detail-trigger').click()
    await popover.waitFor({ state: 'hidden' })
  }

  // 一级：有备注才显示单行备注，且不撑破容器宽度。
  const noteOf = row => row.locator('[data-testid="provider-key-note"]')
  if (await noteOf(rowOf('alpha key 0')).innerText() !== '备注: 财务对账专用，7 月勿删') {
    throw new Error('key 0 行内备注文案不正确')
  }
  if (await noteOf(rowOf('alpha key 2')).count() !== 0) throw new Error('无备注密钥不应出现备注行')
  const longRow = rowOf('alpha key 1')
  const longNote = noteOf(longRow)
  const longMetrics = await longNote.evaluate(element => ({
    overflow: element.scrollWidth > element.clientWidth + 1,
    title: element.getAttribute('title'),
    rowOverflow: element.closest('[class~="group/item"]').scrollWidth > element.closest('[class~="group/item"]').clientWidth + 1,
  }))
  if (!longMetrics.overflow) throw new Error('长备注未触发单行省略')
  if (!longMetrics.title || longMetrics.title.length < 80) throw new Error('长备注缺少完整悬浮文本')
  if (longMetrics.rowOverflow) throw new Error('长备注撑破密钥条目宽度')

  // 二级：详情浮层完整展示备注，无备注时显示空态，浮层本身不横向溢出。
  const results = []
  const cases = [
    { name: 'alpha key 0', expected: '财务对账专用，7 月勿删' },
    { name: 'alpha key 1', expected: 'LONG' },
    { name: 'alpha key 2', expected: null },
  ]
  for (const item of cases) {
    const { row, popover } = await openDetail(item.name)
    const note = popover.locator('[data-testid="provider-key-detail-note"]')
    if (item.expected === null) {
      // 无备注时不渲染备注内容，只显示空态文案。
      if (await note.count() !== 0) throw new Error(`${item.name} 不应渲染备注文本`)
      if (!(await popover.innerText()).includes('暂无备注')) throw new Error(`${item.name} 缺少备注空态`)
    } else {
      await note.waitFor()
      const text = (await note.innerText()).trim()
      if (item.expected === 'LONG') {
        if (text !== (await longNote.getAttribute('title'))) throw new Error('长备注在详情浮层被截断')
        if (!text.includes('\n')) throw new Error('长备注在详情浮层未保留原始文本')
      } else if (text !== item.expected) {
        throw new Error(`${item.name} 详情浮层备注不正确: ${text.slice(0, 40)}`)
      }
    }
    const metrics = await popover.evaluate(element => ({ overflow: element.scrollWidth > element.clientWidth + 1 }))
    if (metrics.overflow) throw new Error(`${item.name} 详情浮层横向溢出`)
    if (item.name === 'alpha key 0') {
      const body = (await popover.innerText()).trim()
      for (const field of ['备注', 'API 格式', 'RPM 限制', '并发限制', '成本倍率', '允许的模型', '创建时间']) {
        if (!body.includes(field)) throw new Error(`详情浮层缺少字段: ${field}`)
      }
      await popover.getByTestId('provider-key-detail-edit').click()
      await page.getByRole('heading', { name: '编辑密钥', exact: true }).waitFor()
      // Radix 卸载浮层带动画，等待详情内容真正从 DOM 移除再断言收起。
      await page.getByTestId('provider-key-detail-note').waitFor({ state: 'detached', timeout: 3000 })
      // 用对话框内的取消按钮退出，避免 Escape 冒泡把外层供应商抽屉一起关掉。
      await page.getByRole('button', { name: '取消', exact: true }).click()
      await page.getByRole('heading', { name: '编辑密钥', exact: true }).waitFor({ state: 'hidden' })
      await page.getByRole('button', { name: '添加密钥', exact: true }).waitFor()
    }
    await closeDetail(row, popover)
    results.push(`${item.name}:ok`)
  }

  // 最小窗口下备注仍可读，浮层不溢出视口。
  for (const width of [390, 840]) {
    await page.setViewportSize({ width, height: 700 })
    const { row, popover } = await openDetail('alpha key 1')
    const box = await popover.boundingBox()
    if (!box || box.x < -1 || box.x + box.width > width + 1) throw new Error(`${width}px 详情浮层超出视口`)
    if (await popover.evaluate(element => element.scrollWidth > element.clientWidth + 1)) throw new Error(`${width}px 详情浮层横向溢出`)
    await closeDetail(row, popover)
  }

  if (failures.length) throw new Error(failures.join('\n'))
  return { inline: '有备注/无备注/长备注省略均通过', popover: results, edit: 'passed', widths: ['390', '840'] }
}
