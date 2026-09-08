import assert from 'node:assert/strict'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import { createMarkdownRenderer, disposeMdItInstance } from 'vitepress'
import { markdownOptions } from '../.vitepress/markdown.mjs'

test('site grammar highlights complete AIVI and contextual fragments consistently', async () => {
  const warnings = []
  const md = await createMarkdownRenderer(fileURLToPath(new URL('../', import.meta.url)), markdownOptions, '/', { warn: message => warnings.push(message) })
  try {
    for (const language of ['aivi', 'aivi-fragment']) {
      const html = md.render(`\`\`\`${language}\n// explanation\nvalue answer = 42\n\`\`\`\n`)
      assert.match(html, /color:#707880/)
      assert.match(html, /color:#81A2BE/)
      assert.match(html, /color:#DE935F/)
      assert.match(html, /answer/)
    }
    assert.deepEqual(warnings, [])
    md.render('```not-an-aivi-language\nvalue n = 1\n```')
    assert.ok(warnings.some(message => message.includes('not-an-aivi-language')))
  } finally {
    disposeMdItInstance()
  }
})
