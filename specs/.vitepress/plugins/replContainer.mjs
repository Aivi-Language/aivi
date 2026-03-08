import container from 'markdown-it-container'

/**
 * Registers a `::: repl` custom container that renders as a visually
 * distinct "Try it in the REPL" block.
 *
 * Usage in markdown:
 *
 *   ::: repl
 *   x = 42
 *   x + 8
 *   -- 50
 *   :::
 */
export function replContainerPlugin(md) {
  md.use(container, 'repl', {
    render(tokens, idx) {
      if (tokens[idx].nesting === 1) {
        return `<div class="repl-block"><div class="repl-header"><span class="repl-prompt">▶</span> Try it in the REPL</div><div class="repl-body">\n`
      } else {
        return `</div></div>\n`
      }
    }
  })
}
