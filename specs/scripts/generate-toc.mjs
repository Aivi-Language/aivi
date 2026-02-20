import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { sidebar } from '../nav.mjs'

const START = '<!-- toc:start -->'
const END = '<!-- toc:end -->'

const rootDir = resolve(fileURLToPath(new URL('..', import.meta.url)))
const markdownTarget = [
  { path: resolve(rootDir, 'index.md'), withExtension: false },
  { path: resolve(rootDir, 'README.md'), withExtension: true }
]

function formatLink(link, withExtension) {
  if (!link) return ''
  const [rawBase, hash] = link.split('#')
  let base = rawBase
  if (base === '/') {
    base = 'index'
  } else {
    base = base.startsWith('/') ? base.slice(1) : base
    if (!base) base = 'index'
  }

  if (withExtension) {
    if (!base.endsWith('.md')) base += '.md'
  } else if (base.endsWith('.md')) {
    base = base.slice(0, -3)
  }

  return hash ? `${base}#${hash}` : base
}

function renderItems(items, withExtension, depth) {
  const lines = []
  let bulletLines = []
  const headingLevel = Math.min(6, 4 + depth)

  const flushBullets = () => {
    if (!bulletLines.length) return
    lines.push(bulletLines.join('\n'))
    bulletLines = []
  }

  for (const item of items ?? []) {
    if (item.items && item.items.length) {
      flushBullets()
      lines.push(`${'#'.repeat(headingLevel)} ${item.text}`)
      const nested = renderItems(item.items, withExtension, depth + 1)
      if (nested.trim()) lines.push(nested)
      continue
    }

    if (item.link) {
      bulletLines.push(`- [${item.text}](${formatLink(item.link, withExtension)})`)
    }
  }

  flushBullets()
  return lines.join('\n')
}

function renderSidebar(withExtension) {
  return sidebar
    .map((section) => {
      const lines = [`### ${section.text}`]
      const body = renderItems(section.items, withExtension, 0)
      if (body.trim()) lines.push(body)
      return lines.join('\n')
    })
    .join('\n\n')
}

function updateFile({ path, withExtension }) {
  const source = readFileSync(path, 'utf-8')
  if (!source.includes(START) || !source.includes(END)) {
    throw new Error(`Missing TOC markers in ${path}`)
  }

  const toc = renderSidebar(withExtension)
  const before = source.split(START)[0]
  const after = source.split(END)[1]
  const updated = `${before}${START}\n\n${toc}\n\n${END}${after}`

  if (updated !== source) {
    writeFileSync(path, updated)
  }
}

for (const target of markdownTarget) {
  updateFile(target)
}

console.log('TOC updated.')
