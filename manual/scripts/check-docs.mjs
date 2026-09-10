import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { createMarkdownRenderer, disposeMdItInstance } from 'vitepress'
import { nav, sidebar } from '../.vitepress/navigation.ts'
import { fenceProblems, resolveDocTarget, tableProblems } from './doc-checks.mjs'
import { markdownOptions } from '../.vitepress/markdown.mjs'

const root = fileURLToPath(new URL('../../', import.meta.url))
const manual = path.join(root, 'manual')
const issues = []
const external = new Set()
const files = ['AGENTS.md', 'README.md', 'CONTRIBUTING.md', 'AIVI_RFC.md', 'syntax.md',
  'packages/application-types/API.md',
  'packages/application-types/README.md',
  'tooling/packages/vscode-aivi/README.md',
  ...fs.readdirSync(path.join(root, 'crates')).filter(p => fs.existsSync(path.join(root, 'crates', p, 'README.md'))).map(p => `crates/${p}/README.md`),
  ...fs.readdirSync(manual, { recursive: true }).filter(p => p.endsWith('.md') && !p.startsWith('node_modules/') && !p.startsWith('.vitepress/')).map(p => `manual/${p}`),
].sort()
const md = await createMarkdownRenderer(manual, markdownOptions, '/', {
  warn: message => issues.push(`highlighting: ${message}`),
})
const documents = new Map()
const walk = function* (tokens) {
  for (const token of tokens) {
    yield token
    if (token.children) yield* walk(token.children)
  }
}
let fences = 0
let fragments = 0
for (const file of files) {
  const source = fs.readFileSync(path.join(root, file), 'utf8')
  for (const issue of fenceProblems(source)) issues.push(`${file}: ${issue}`)
  for (const issue of tableProblems(source)) issues.push(`${file}: ${issue}`)
  const env = { path: path.join(root, file), relativePath: file.replace(/^manual\//, '') }
  const tokens = md.parse(source, env)
  const ids = new Set()
  const links = []
  const githubCounts = new Map()
  for (const [index, token] of tokens.entries()) {
    if (token.type === 'heading_open') {
      let id = token.attrGet('id')
      if (!file.startsWith('manual/')) {
        // Repository Markdown is rendered by GitHub, not VitePress (notably numeric headings).
        const title = [...walk(md.parseInline(tokens[index + 1].content, {}))].filter(t => ['text', 'code_inline'].includes(t.type)).map(t => t.content).join('')
        const slug = title.toLowerCase().replace(/[^\p{L}\p{N}\p{M}\s_-]/gu, '').replace(/ /g, '-')
        const count = githubCounts.get(slug) ?? 0
        githubCounts.set(slug, count + 1)
        id = count ? `${slug}-${count}` : slug
      }
      ids.add(id)
    }
    if (token.type === 'fence') {
      fences++
      if (token.info.trim() === 'aivi-fragment') fragments++
      if (!token.info.trim()) issues.push(`${file}:${token.map[0] + 1}: missing fence language (use text for plain output)`)
      const closing = source.split('\n')[token.map[1] - 1]?.trim() ?? ''
      if (!new RegExp(`^${token.markup[0]}{${token.markup.length},}\\s*$`).test(closing)) issues.push(`${file}:${token.map[0] + 1}: unclosed code fence`)
    }
  }
  for (const token of walk(tokens)) {
    if (token.type === 'link_open' && token.attrGet('class') !== 'header-anchor') links.push(token.attrGet('href'))
    if (token.type === 'image') links.push(token.attrGet('src'))
    if (['html_block', 'html_inline'].includes(token.type)) {
      for (const match of token.content.matchAll(/\b(?:href|src)=["']([^"']+)["']/g)) links.push(match[1])
      for (const match of token.content.matchAll(/\bid=["']([^"']+)["']/g)) ids.add(match[1])
    }
  }
  // Render every fence through the same grammar/theme as the site; unknown languages warn.
  md.renderer.render(tokens, md.options, env)
  for (const action of env.frontmatter?.hero?.actions ?? []) links.push(action.link)
  documents.set(file, { ids, links })
}

function resolveTarget(from, href) {
  return resolveDocTarget(from, href, p => fs.existsSync(path.join(root, p)) && fs.statSync(path.join(root, p)).isFile())
}
let linkCount = 0
function checkLink(from, href) {
  if (!href) return
  linkCount++
  if (/^https?:\/\//.test(href)) { external.add(href); return }
  if (/^(?:mailto:|data:|tel:)/.test(href)) return
  try {
    const target = resolveTarget(from, href)
    if (!target) { issues.push(`${from}: missing target ${href}`); return }
    const hash = href.includes('#') ? decodeURIComponent(href.slice(href.indexOf('#') + 1)) : ''
    if (hash && documents.has(target) && !documents.get(target).ids.has(hash)) issues.push(`${from}: missing anchor ${href} (in ${target})`)
  } catch (error) { issues.push(`${from}: invalid link ${href}: ${error.message}`) }
}
for (const [file, document] of documents) for (const link of document.links) checkLink(file, link)
const routes = new Set()
function checkNavigation(items) {
  for (const item of items) {
    if (item.link) {
      checkLink('manual/index.md', item.link)
      routes.add(resolveTarget('manual/index.md', item.link))
    }
    if (item.items) checkNavigation(item.items)
  }
}
checkNavigation(nav)
for (const items of Object.values(sidebar)) checkNavigation(items)
for (const file of files.filter(p => p.startsWith('manual/') && !['manual/index.md', 'manual/guide/README.md'].includes(p))) {
  if (!routes.has(file)) issues.push(`${file}: missing from navigation`)
}

if (process.argv.includes('--external')) {
  // Bound concurrency and duration; access failures are not proof of a dead URL.
  const pending = [...external]
  await Promise.all(Array.from({ length: 4 }, async () => {
    while (pending.length) {
      const url = pending.shift()
      try {
        const response = await fetch(url, { signal: AbortSignal.timeout(15000), redirect: 'follow' })
        await response.body?.cancel()
        if (!response.ok) issues.push(`external: ${url}: HTTP ${response.status} (verify manually; may deny automated access)`)
      } catch (error) { issues.push(`external: ${url}: unverified (${error.message})`) }
    }
  }))
}
console.log(`Checked ${files.length} documents, ${fences} fences, ${linkCount} links, ${external.size} external URLs.`)
console.log(`${fragments} AIVI fragments are highlighted only; use the separate manual-snippets gate for complete AIVI blocks.`)
if (!process.argv.includes('--external')) console.log('External URLs were inventoried, not fetched; use --external to check HTTP reachability. External fragments need manual review.')
for (const issue of issues) console.error(issue)
disposeMdItInstance()
process.exitCode = issues.length ? 1 : 0
