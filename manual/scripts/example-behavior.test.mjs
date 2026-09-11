// Build `cargo build --bin aivi` before running these checks, or set AIVI_BIN.
// Execute the actual documented examples: type-checking alone cannot catch a
// function that returns its input while the prose promises an update.
import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { promisify } from 'node:util'
import test from 'node:test'

const root = fileURLToPath(new URL('../../', import.meta.url))
const binary = process.env.AIVI_BIN ?? path.join(root, 'target/debug/aivi')
const run = promisify(execFile)

const cases = [
  ['guide/values-and-functions.md', 'func addFrom =', 'total == 42', 'value!'],
  ['guide/values-and-functions.md', 'func bump =', 'bumped.total == 5 and bumped.ready', 'counter!', /applicative cluster is not supported in typed-core general expressions/],
  ['guide/values-and-functions.md', 'func readNested =', 'nestedTotal == 41', '{ x.y.z! }'],
  ['guide/values-and-functions.md', 'func statusLineFor =', 'statusText == "ready" and scoreText == "Score: 42"', '{.}'],
  ['guide/predicates.md', 'func promoteActive =', 'promoted.users == [{ name: "Ada", role: "admin", active: True }, { name: "Grace", role: "guest", active: False }]', 'users[.active].role', /patch expression is not supported in typed-core general expressions/],
  ['guide/predicates.md', 'func discountExpensive =', 'discounted.items == [{ name: "Desk", price: 110, inStock: True }, { name: "Lamp", price: 40, inStock: True }]', 'items[.price >= 100].price', /patch expression is not supported in typed-core general expressions/],
  ['guide/pipes.md', 'func nameLength =', 'result == "Hello, Ada"', '| nameLength'],
]

for (const [file, marker, expectation, syntax, executionLimit] of cases) {
  test(`documented behavior: ${marker}`, { skip: !existsSync(binary) && 'Build aivi or set AIVI_BIN' }, async () => {
    const markdown = await readFile(path.join(root, 'manual', file), 'utf8')
    const blocks = [...markdown.matchAll(/^```aivi\s*\n([\s\S]*?)^```/gm)]
    const block = blocks.find(match => match[1].includes(marker))?.[1]
    assert.ok(block, `Missing complete example for ${marker}`)
    assert.ok(block.includes(syntax), `Example must demonstrate ${syntax}`)
    const dir = await mkdtemp(path.join(os.tmpdir(), 'aivi-manual-behavior-'))
    try {
      const source = path.join(dir, 'main.aivi')
      await writeFile(source, `${block}\n@test\nvalue documentedBehavior : Task Text Bool = pure (${expectation})\n`)
      const options = { cwd: root, timeout: 60000 }
      await run(binary, ['check', source], options)
      if (executionLimit) {
        // If backend support lands, this fails and prompts removal of the warning.
        assert.match(markdown, /`aivi test` currently rejects/)
        await assert.rejects(run(binary, ['test', source, 'documentedBehavior'], options), error => {
          assert.match(error.stderr, executionLimit)
          return true
        })
      } else {
        const { stdout } = await run(binary, ['test', source, 'documentedBehavior'], options)
        assert.match(stdout, /1 passed/)
      }
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })
}
