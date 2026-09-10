#!/usr/bin/env node

import fs from 'node:fs'
import path from 'node:path'

const repoRoot = path.resolve(import.meta.dirname, '..')
const stdlibRoot = path.join(repoRoot, 'stdlib', 'aivi')
const manualRoot = path.join(repoRoot, 'manual', 'stdlib')
const importMetadataFile = path.join(repoRoot, 'crates', 'aivi-hir', 'src', 'lower', 'helpers.rs')

const docNames = new Map([
  ['aivi.core.bytes', 'bytes.md'],
  ['aivi.core.dict', 'dict.md'],
  ['aivi.core.either', 'either.md'],
  ['aivi.core.float', 'float.md'],
  ['aivi.core.fn', 'fn.md'],
  ['aivi.core.range', 'range.md'],
  ['aivi.core.set', 'set.md'],
  ['aivi.data.json', 'json.md'],
  ['aivi.desktop.xdg', 'xdg.md'],
  ['aivi.gnome.notifications', 'notifications.md'],
  ['aivi.gnome.onlineAccounts', 'onlineAccounts.md'],
  ['aivi.gnome.settings', 'settings.md'],
  ['aivi.gnome.tray', 'tray.md'],
  ['aivi.gtk.icons', 'icons.md'],
  ['aivi.gtk.styles', 'styles.md'],
])

// These names are resolved through builtin class dictionaries rather than a
// source declaration or ordinary import binding.
const classProvidedValues = new Map([
  ['aivi.list', new Set(['minimum', 'unique', 'sort'])],
  ['aivi.order', new Set(['min', 'max', 'minOf', 'maxOf', 'clamp'])],
  ['aivi.prelude', new Set(['min', 'max', 'minOf', 'maxOf', 'clamp'])],
])

function walk(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const file = path.join(dir, entry.name)
    return entry.isDirectory() ? walk(file) : entry.name.endsWith('.aivi') ? [file] : []
  })
}

function moduleName(file) {
  return `aivi.${path.relative(stdlibRoot, file).replace(/\.aivi$/, '').split(path.sep).join('.')}`
}

function exportedNames(source) {
  const names = []
  for (const match of source.matchAll(/^export\s+(?:\(([^)]*)\)|([^\n]+))/gm)) {
    names.push(...(match[1] ?? match[2]).split(/[\s,]+/).filter(Boolean))
  }
  return new Set(names)
}

function documentsName(documentation, name) {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return new RegExp('`' + escaped + '(?:`|[^A-Za-z0-9_])').test(documentation)
}

function selfImportedValues(source, module) {
  const names = new Set()
  const escaped = module.replaceAll('.', '\\.')
  const pattern = new RegExp(`^use\\s+${escaped}\\s+\\(([^)]*)\\)`, 'gm')
  for (const match of source.matchAll(pattern)) {
    for (const line of match[1].split('\n')) {
      const parts = line.trim().split(/\s+/)
      if (!parts[0]) continue
      names.add(parts[2] ?? parts[0])
    }
  }
  return names
}

function tests(source) {
  const starts = [...source.matchAll(/^@test\s*\nvalue\s+([A-Za-z][A-Za-z0-9_]*)/gm)]
  return starts.map((match, index) => ({
    name: match[1],
    body: source.slice(match.index, starts[index + 1]?.index ?? source.length),
  }))
}

function compilerProvidedValues() {
  const source = fs.readFileSync(importMetadataFile, 'utf8')
  const values = new Map()
  const pattern = /\("([^"]+)",\s*"([^"]+)"\)\s*=>\s*Some\(\s*(intrinsic_import_value|ImportBindingMetadata::AmbientValue)/g
  for (const match of source.matchAll(pattern)) {
    const module = match[1]
    const name = match[2]
    if (!values.has(module)) values.set(module, new Map())
    values.get(module).set(name, match[3] === 'intrinsic_import_value' ? 'intrinsic' : 'ambient')
  }
  return values
}

function externalCoverage() {
  const coverage = new Map()
  const roots = [path.join(repoRoot, 'stdlib', 'tests'), path.join(repoRoot, 'crates')]
  const files = roots.flatMap(root => walkCoverageFiles(root))
  for (const file of files) {
    const source = fs.readFileSync(file, 'utf8')
    for (const match of source.matchAll(/@covers\s+(aivi(?:\.[A-Za-z][A-Za-z0-9_]*)+)\s+([^\n]+)/g)) {
      const module = match[1]
      for (const name of match[2].split(/[\s,]+/).filter(name => /^[a-z][A-Za-z0-9_]*$/.test(name))) {
        coverage.set(`${module}.${name}`, `external:${path.relative(repoRoot, file)}`)
      }
    }
  }
  return coverage
}

function walkCoverageFiles(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const file = path.join(dir, entry.name)
    if (entry.isDirectory()) return entry.name === 'target' ? [] : walkCoverageFiles(file)
    return /\.(?:aivi|rs)$/.test(entry.name) ? [file] : []
  })
}

const rows = []
const exportRows = []
const compilerValues = compilerProvidedValues()
const coveredExternally = externalCoverage()
for (const file of walk(stdlibRoot).sort()) {
  if (path.basename(file).startsWith('bundledsmoke')) continue
  const source = fs.readFileSync(file, 'utf8')
  const module = moduleName(file)
  const exports = exportedNames(source)
  const authored = new Set([...source.matchAll(/^func\s+([A-Za-z][A-Za-z0-9_]*)\s*=/gm)].map(match => match[1]))
  const intrinsic = selfImportedValues(source, module)
  const localTests = tests(source)
  const docFile = path.join(manualRoot, docNames.get(module) ?? `${path.basename(file, '.aivi')}.md`)
  const documentation = fs.existsSync(docFile) ? fs.readFileSync(docFile, 'utf8') : ''

  for (const name of [...exports].sort()) {
    if (/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) {
      exportRows.push({
        module,
        name,
        source: path.relative(repoRoot, file),
        documentation: path.relative(repoRoot, docFile),
        documented: documentsName(documentation, name),
      })
    }
    if (!/^[a-z]/.test(name)) continue
    const implementation = authored.has(name)
      ? 'stdlib'
      : intrinsic.has(name)
        ? 'intrinsic'
        : compilerValues.get(module)?.get(name)
          ?? (classProvidedValues.get(module)?.has(name) ? 'class' : null)
    if (!implementation) continue
    rows.push({
      module,
      name,
      implementation,
      source: path.relative(repoRoot, file),
      documentation: path.relative(repoRoot, docFile),
      documented: documentsName(documentation, name),
      tests: [
        ...localTests
        .filter(test => {
          const directReference = new RegExp(`(^|[^A-Za-z0-9_])${name}([^A-Za-z0-9_]|$)`)
          return directReference.test(test.body)
        })
        .map(test => test.name),
        ...(coveredExternally.has(`${module}.${name}`) ? [coveredExternally.get(`${module}.${name}`)] : []),
      ],
    })
  }
}

const gaps = rows.filter(row => !row.documented || row.tests.length === 0)
const exportGaps = exportRows.filter(row => !row.documented)
if (process.argv.includes('--json')) {
  process.stdout.write(`${JSON.stringify({ functions: rows, gaps, exports: exportRows, exportGaps }, null, 2)}\n`)
} else {
  for (const row of gaps) {
    const missing = [!row.documented && 'documentation', row.tests.length === 0 && 'behavioral test'].filter(Boolean).join(', ')
    console.error(`${row.module}.${row.name}: missing ${missing}`)
  }
  for (const row of exportGaps) {
    console.error(`${row.module}.${row.name}: missing reference documentation`)
  }
  console.log(
    `stdlib API audit: ${rows.length - gaps.length}/${rows.length} exported functions have behavioral coverage; ` +
    `${exportRows.length - exportGaps.length}/${exportRows.length} public exports have reference documentation`,
  )
}

if (gaps.length > 0 || exportGaps.length > 0) process.exitCode = 1
