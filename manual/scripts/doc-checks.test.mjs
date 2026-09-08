import assert from 'node:assert/strict'
import test from 'node:test'
import { fenceProblems, resolveDocTarget, tableProblems } from './doc-checks.mjs'

test('fences reject trailing closing text, empty bodies and missing closure', () => {
  assert.deepEqual(fenceProblems('```aivi\nvalue n = 1\n```'), [])
  assert.match(fenceProblems('```aivi\nvalue n = 1\n``` — visible')[0], /trailing text/)
  assert.match(fenceProblems('```aivi\n\n```')[0], /empty/)
  assert.match(fenceProblems('```aivi\nvalue n = 1')[0], /unclosed/)
})

test('long fences may contain shorter example fences; tilde fences work', () => {
  assert.deepEqual(fenceProblems('````text\n```aivi\nvalue n = 1\n```\n````'), [])
  assert.deepEqual(fenceProblems('~~~text\noutput\n~~~'), [])
})

test('manual routes resolve pages, directories, encoded names and public assets', () => {
  const files = new Set(['manual/guide/types.md', 'manual/stdlib/index.md', 'manual/public/logo.png', 'manual/guide/a b.md'])
  const resolve = href => resolveDocTarget('manual/guide/test.md', href, p => files.has(p))
  assert.equal(resolve('/stdlib/'), 'manual/stdlib/index.md')
  assert.equal(resolve('types.html#records'), 'manual/guide/types.md')
  assert.equal(resolve('types?query=1#records'), 'manual/guide/types.md')
  assert.equal(resolve('a%20b.md'), 'manual/guide/a b.md')
  assert.equal(resolve('/logo.png'), 'manual/public/logo.png')
  assert.equal(resolve('#local'), 'manual/guide/test.md')
  assert.equal(resolve('missing.md'), undefined)
})

test('repository links use exact paths and cannot escape the repository', () => {
  const isFile = p => p === 'AIVI_RFC.md' || p === '../outside.md'
  assert.equal(resolveDocTarget('crates/aivi-query/README.md', '../../AIVI_RFC.md#26-cli-reference', isFile), 'AIVI_RFC.md')
  assert.equal(resolveDocTarget('README.md', '../outside.md', isFile), undefined)
  assert.equal(resolveDocTarget('README.md', 'AIVI_RFC', isFile), undefined)
  assert.throws(() => resolveDocTarget('README.md', '%zz', isFile), URIError)
})

test('pipe operators in table code spans require Markdown escaping', () => {
  assert.match(tableProblems('| Match | `||>` |')[0], /escape pipes/)
  assert.deepEqual(tableProblems('| Match | `\\|\\|>` |'), [])
  assert.deepEqual(tableProblems('```text\n| Match | `||>` |\n```'), [])
})
