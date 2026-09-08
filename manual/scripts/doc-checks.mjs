import path from 'node:path'

export function fenceProblems(source) {
  const issues = []
  let open
  const lines = source.split('\n')
  for (const [index, line] of lines.entries()) {
    const match = line.match(/^ {0,3}(`{3,}|~{3,})(.*)$/)
    if (!match) continue
    if (!open) {
      open = { marker: match[1], line: index + 1, content: index + 1 }
    } else if (match[1][0] === open.marker[0] && match[1].length >= open.marker.length) {
      if (match[2].trim()) {
        issues.push(`line ${index + 1}: closing fence has trailing text`)
      } else {
        if (!lines.slice(open.content, index).join('\n').trim()) issues.push(`line ${open.line}: empty code fence`)
        open = undefined
      }
    }
  }
  if (open) issues.push(`line ${open.line}: unclosed code fence`)
  return issues
}

export function tableProblems(source) {
  const issues = []
  let fence
  for (const [index, line] of source.split('\n').entries()) {
    const marker = line.match(/^ {0,3}(`{3,}|~{3,})/)
    if (marker) {
      if (!fence) fence = marker[1]
      else if (marker[1][0] === fence[0] && marker[1].length >= fence.length) fence = undefined
      continue
    }
    if (fence || !line.trimStart().startsWith('|')) continue
    for (const span of line.matchAll(/`[^`]*`/g)) {
      if (/(?<!\\)\|/.test(span[0])) issues.push(`line ${index + 1}: escape pipes inside table code spans`)
    }
  }
  return issues
}

export function resolveDocTarget(from, href, isFile) {
  const decoded = decodeURIComponent(href.split(/[?#]/, 1)[0])
  if (!decoded) return from
  const manual = from.startsWith('manual/')
  const target = decoded.startsWith('/')
    ? path.posix.join(manual ? 'manual' : '', decoded)
    : path.posix.join(path.posix.dirname(from), decoded)
  const candidates = manual
    ? [target, target.replace(/\.html$/, '.md'), `${target}.md`, path.posix.join(target, 'index.md'), path.posix.join('manual/public', decoded)]
    : [target]
  return candidates.find(p => !p.startsWith('../') && isFile(p))
}
