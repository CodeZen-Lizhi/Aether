// 验证实际供应商抽屉中的单密钥和供应商级同步入口。
async (page) => {
  await page.getByLabel('倍率来源').first().selectOption('upstream')
  await page.getByText('fixture group', { exact: false }).first().waitFor()
  await page.getByRole('button', { name: '同步上游倍率', exact: true }).first().click()
  for (const width of [900, 1280]) {
    await page.setViewportSize({ width, height: 640 })
    const overflow = await page.locator('.drawer-panel').evaluate(element => element.scrollWidth > element.clientWidth)
    if (overflow) throw new Error('供应商倍率布局溢出')
  }
  console.log('供应商倍率入口及布局检查通过')
}
