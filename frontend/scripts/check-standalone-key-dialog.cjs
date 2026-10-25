// 用真实 StandaloneKeyFormDialog 组件检查最小窗口下的弹窗层级、动画帧和表单可达性。
/* eslint-disable @typescript-eslint/no-require-imports -- 现有夹具脚本均由 Node 直接运行。 */
const { chromium, webkit } = require('playwright')

const BROWSER = process.env.PREVIEW_BROWSER === 'webkit' ? webkit : chromium

const URL = process.env.PREVIEW_URL
  || 'http://127.0.0.1:5173/scripts/fixtures/standalone-key-dialog.html'

function assert(condition, message) {
  if (!condition) throw new Error(message)
}

async function captureOpeningFrames(page, selector) {
  return page.evaluate(async (buttonSelector) => {
    const samples = []
    const started = performance.now()
    const button = document.querySelector(buttonSelector)
    button?.click()
    for (let index = 0; index < 12; index += 1) {
      await new Promise(resolve => requestAnimationFrame(resolve))
      const dialog = document.querySelector('.app-dialog')
      const backdrop = document.querySelector('[data-app-dialog-backdrop]')
      const dialogRect = dialog?.getBoundingClientRect()
      const backdropStyle = backdrop ? getComputedStyle(backdrop) : null
      const dialogStyle = dialog ? getComputedStyle(dialog) : null
      samples.push({
        t: Number((performance.now() - started).toFixed(1)),
        hasDialog: !!dialog,
        dialog: dialogRect
          ? {
              top: Number(dialogRect.top.toFixed(1)),
              left: Number(dialogRect.left.toFixed(1)),
              width: Number(dialogRect.width.toFixed(1)),
              height: Number(dialogRect.height.toFixed(1)),
            }
          : null,
        dialogOpacity: dialogStyle?.opacity,
        dialogTransform: dialogStyle?.transform,
        dialogBackground: dialogStyle?.backgroundColor,
        backdropOpacity: backdropStyle?.opacity,
        backdropBackground: backdropStyle?.backgroundColor,
        backdropFilter: backdropStyle?.backdropFilter,
      })
    }
    return samples
  }, selector)
}

async function waitForOpenDialog(page) {
  await page.locator('.app-dialog').waitFor({ state: 'visible' })
  await page.waitForFunction(() => {
    const dialog = document.querySelector('.app-dialog')
    return dialog && getComputedStyle(dialog).opacity === '1'
  })
}

async function closeDialog(page) {
  await page.locator('.app-dialog button', { hasText: '取消' }).click()
  await page.locator('.app-dialog').waitFor({ state: 'detached' })
}

async function inspectLayout(page, label) {
  return page.evaluate((viewportLabel) => {
    const container = document.querySelector('[data-app-dialog-container]')
    const backdrop = document.querySelector('[data-app-dialog-backdrop]')
    const dialog = document.querySelector('.app-dialog')
    const body = document.querySelector('.dialog-body')
    const footer = dialog?.querySelector(':scope > div:last-child')
    const dialogRect = dialog?.getBoundingClientRect()
    const footerRect = footer?.getBoundingClientRect()
    const backdropStyle = backdrop ? getComputedStyle(backdrop) : null
    const dialogStyle = dialog ? getComputedStyle(dialog) : null
    return {
      viewportLabel,
      viewport: { width: window.innerWidth, height: window.innerHeight },
      pageOverflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
      container: !!container,
      backdrop: backdropStyle
        ? {
            backgroundColor: backdropStyle.backgroundColor,
            backdropFilter: backdropStyle.backdropFilter,
          }
        : null,
      dialog: dialogRect
        ? {
            top: Number(dialogRect.top.toFixed(1)),
            left: Number(dialogRect.left.toFixed(1)),
            width: Number(dialogRect.width.toFixed(1)),
            height: Number(dialogRect.height.toFixed(1)),
            background: dialogStyle?.backgroundColor,
            overflowX: body ? body.scrollWidth > body.clientWidth : false,
          }
        : null,
      footerVisible: !!footerRect && footerRect.top >= 0 && footerRect.bottom <= window.innerHeight,
      text: dialog?.innerText ?? '',
    }
  }, label)
}

async function main() {
  const browser = await BROWSER.launch()
  const page = await browser.newPage({ viewport: { width: 900, height: 640 } })
  const errors = []
  page.on('pageerror', error => errors.push(error.message))
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(message.text())
  })

  try {
    await page.goto(URL, { waitUntil: 'domcontentloaded' })
    await page.locator('#open-edit').waitFor()

    const editFrames = await captureOpeningFrames(page, '#open-edit')
    await waitForOpenDialog(page)
    const editLayout = await inspectLayout(page, 'edit-900x640')
    assert(editLayout.dialog, '编辑弹窗未渲染')
    assert(editLayout.footerVisible, '编辑弹窗底部操作不可达')
    assert(!editLayout.dialog.overflowX, '编辑弹窗正文横向溢出')
    assert(!editLayout.pageOverflow, '编辑弹窗导致页面横向溢出')
    assert(editLayout.text.startsWith('编辑密钥\n'), '编辑弹窗标题仍包含复杂说明')
    assert(!editLayout.text.includes('修改密钥名称'), '编辑弹窗仍显示冗余副标题')
    assert(!editLayout.text.includes('当前使用独立无限额度'), '编辑弹窗仍显示冗余额度说明')
    await closeDialog(page)

    const createFrames = await captureOpeningFrames(page, '#open-create')
    await waitForOpenDialog(page)
    const createLayout = await inspectLayout(page, 'create-900x640')
    assert(createLayout.dialog, '新建弹窗未渲染')
    assert(createLayout.footerVisible, '新建弹窗底部操作不可达')
    assert(!createLayout.dialog.overflowX, '新建弹窗正文横向溢出')
    assert(!createLayout.pageOverflow, '新建弹窗导致页面横向溢出')
    assert(createLayout.text.startsWith('新建密钥\n'), '新建弹窗标题仍包含复杂说明')
    assert(!createLayout.text.includes('用于非注册用户'), '新建弹窗仍显示冗余副标题')
    await closeDialog(page)

    const firstPaint = [...editFrames, ...createFrames].filter(frame => frame.hasDialog)
    assert(firstPaint.length > 0, '未采集到弹窗打开帧')
    const transparentFrames = firstPaint.filter((frame) => {
      const alpha = frame.dialogBackground?.match(/rgba?\([^)]*,\s*([\d.]+)\)$/)?.[1]
      return frame.dialogOpacity === '0' || alpha === '0'
    })
    const legacyCompositeFrames = firstPaint.filter(frame =>
      frame.dialogTransform !== 'none' && frame.backdropFilter?.includes('blur') === true,
    )
    assert(transparentFrames.length === 0, '弹窗打开帧出现透明内容层')
    assert(legacyCompositeFrames.length === 0, '遮罩模糊与弹窗变换仍同时参与合成')
    console.log(JSON.stringify({
      editLayout,
      createLayout,
      editFrames,
      createFrames,
      transparentFrameCount: transparentFrames.length,
      legacyCompositeFrameCount: legacyCompositeFrames.length,
      errors,
    }, null, 2))
    assert(errors.length === 0, `页面存在错误：${errors.join(' | ')}`)
  } finally {
    await browser.close()
  }
}

main().catch((error) => {
  console.error(error.stack || error)
  process.exitCode = 1
})
