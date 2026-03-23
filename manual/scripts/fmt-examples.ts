import fs from 'node:fs'
import path from 'node:path'
import { execSync } from 'node:child_process'

// ── Types ──────────────────────────────────────────────────────────────────

interface Snippet {
  startIndex: number  // index of the opening ``` line in the lines array
  endIndex:   number  // index of the closing ``` line
  content:    string
}

// ── Helpers ────────────────────────────────────────────────────────────────

function aiviInPath(): boolean {
  try {
    execSync('which aivi', { stdio: 'ignore' })
    return true
  } catch {
    return false
  }
}

function globMarkdown(root: string): string[] {
  const results: string[] = []

  function walk(dir: string): void {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name)
      if (entry.isDirectory()) {
        if (entry.name === 'node_modules' || entry.name === 'dist') continue
        walk(full)
      } else if (entry.isFile() && entry.name.endsWith('.md')) {
        results.push(full)
      }
    }
  }

  walk(root)
  return results
}

function extractSnippets(lines: string[]): Snippet[] {
  const results: Snippet[] = []
  let inside = false
  let start  = 0
  const buf: string[] = []

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    if (!inside && /^```aivi\s*$/.test(line)) {
      inside = true
      start  = i
      buf.length = 0
    } else if (inside && /^```\s*$/.test(line)) {
      inside = false
      results.push({ startIndex: start, endIndex: i, content: buf.join('\n') })
    } else if (inside) {
      buf.push(line)
    }
  }

  return results
}

function formatSnippet(content: string): string {
  const result = execSync('aivi fmt --stdin', {
    input: content,
    encoding: 'utf8',
  })
  return result.trim()
}

// ── Main ───────────────────────────────────────────────────────────────────

if (!aiviInPath()) {
  console.warn('⚠  aivi not found in PATH — skipping formatting')
  process.exit(0)
}

const manualRoot    = path.resolve(import.meta.dirname ?? __dirname, '..')
const markdownFiles = globMarkdown(manualRoot)
const changed: string[] = []

for (const mdFile of markdownFiles) {
  const original = fs.readFileSync(mdFile, 'utf8')
  const lines    = original.split('\n')
  const snippets = extractSnippets(lines)

  if (snippets.length === 0) continue

  let modified = false

  // Process in reverse so indices stay valid after splicing
  for (const snippet of [...snippets].reverse()) {
    let formatted: string
    try {
      formatted = formatSnippet(snippet.content)
    } catch (err: unknown) {
      const rel = path.relative(manualRoot, mdFile)
      const msg = (err as { stderr?: Buffer }).stderr?.toString() ?? String(err)
      console.error(`WARN   ${rel}: fmt failed: ${msg.split('\n')[0]}`)
      continue
    }

    if (formatted === snippet.content) continue

    // Replace the lines between the fences (exclusive)
    const newContentLines = formatted.split('\n')
    lines.splice(snippet.startIndex + 1, snippet.endIndex - snippet.startIndex - 1, ...newContentLines)
    modified = true
  }

  if (modified) {
    fs.writeFileSync(mdFile, lines.join('\n'), 'utf8')
    changed.push(path.relative(manualRoot, mdFile))
  }
}

if (changed.length === 0) {
  console.log('All AIVI examples are already formatted.')
} else {
  console.log(`Formatted AIVI examples in ${changed.length} file(s):`)
  for (const f of changed) {
    console.log(`  ${f}`)
  }
}
process.exit(0)
