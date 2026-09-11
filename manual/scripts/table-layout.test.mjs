// Opt-in browser regression against a running manual (`pnpm dev`):
// AIVI_MANUAL_BROWSER_MODULE=playwright node --test scripts/table-layout.test.mjs
// The module may also be an absolute path to an existing Playwright installation.
// AIVI_MANUAL_BROWSER_EXECUTABLE optionally selects an installed Chromium binary.
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import test from 'node:test'

const browserModule = process.env.AIVI_MANUAL_BROWSER_MODULE
const base = process.env.AIVI_MANUAL_URL ?? 'http://127.0.0.1:5173'

test('manual tables preserve inline content, labels and viewport bounds', {
  skip: !browserModule && 'Set AIVI_MANUAL_BROWSER_MODULE and start the manual preview',
}, async () => {
  const { chromium } = createRequire(import.meta.url)(browserModule)
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.AIVI_MANUAL_BROWSER_EXECUTABLE,
  })
  try {
    const page = await browser.newPage()
    for (const width of [320, 390, 600, 767, 768, 1280]) {
      await page.setViewportSize({ width, height: 900 })
      for (const route of ['/guide/getting-started', '/guide/pipes', '/guide/values-and-functions', '/guide/source-catalog', '/stdlib/list']) {
        await page.goto(new URL(route, base).href)
        await page.waitForSelector('.vp-doc td[data-column]')
        const result = await page.locator('.vp-doc').evaluate(doc => {
          const mobile = innerWidth < 768
          const issues = []
          for (const table of doc.querySelectorAll('table')) {
            const headers = [...table.querySelectorAll('thead th')].map(th => th.textContent.trim())
            for (const row of table.querySelectorAll('tbody tr')) {
              for (const [index, cell] of [...row.cells].entries()) {
                if (cell.dataset.column !== headers[index]) issues.push('Missing column label')
                if (mobile && cell.scrollWidth > cell.clientWidth + 1) issues.push(`Cell overflows: ${cell.textContent}`)
                for (const code of cell.querySelectorAll('code')) {
                  if (getComputedStyle(code).display !== 'inline') issues.push(`Code is not inline: ${code.textContent}`)
                }
              }
            }
            if (!mobile && getComputedStyle(table.querySelector('thead')).display === 'none') issues.push('Desktop headers hidden')
          }
          if (document.documentElement.scrollWidth > innerWidth + 1) issues.push('Page overflows viewport')
          return issues
        })
        assert.deepEqual(result, [], `${route} at ${width}px`)
        if (route === '/guide/getting-started') {
          const tops = await page.locator('.vp-doc table').first().locator('tbody tr:first-child td:first-child code').evaluateAll(
            codes => codes.map(code => code.getBoundingClientRect().top)
          )
          assert.equal(tops.length, 2)
          assert.ok(Math.abs(tops[0] - tops[1]) < 1, `if/else split across lines at ${width}px`)
          if (width === 390 && process.env.AIVI_MANUAL_SCREENSHOT) {
            await page.screenshot({ path: process.env.AIVI_MANUAL_SCREENSHOT, fullPage: true })
          }
        }
      }
    }
    // Navigate through VitePress without reloading: labels must follow new content.
    await page.goto(new URL('/guide/getting-started', base).href)
    await page.locator('.vp-doc a[href*="/guide/thinking-in-aivi"]').first().click()
    await page.waitForURL(/\/guide\/thinking-in-aivi(?:\.html)?$/)
    await page.waitForSelector('.vp-doc td[data-column]')
  } finally {
    await browser.close()
  }
})
