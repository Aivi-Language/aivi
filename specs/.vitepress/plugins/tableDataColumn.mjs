/**
 * markdown-it core rule plugin.
 *
 * For every table in the token stream, reads the <th> header labels from
 * <thead> and injects a `data-column="<label>"` attribute onto each
 * corresponding <td> in <tbody>.  The CSS layer can then use
 * `td::before { content: attr(data-column) }` for responsive card layouts.
 */
export function tableDataColumnPlugin(md) {
  md.core.ruler.push('table_data_column', (state) => {
    const tokens = state.tokens
    let i = 0

    while (i < tokens.length) {
      if (tokens[i].type !== 'table_open') {
        i++
        continue
      }

      // --- collect header labels from <thead> ---
      const headers = []
      let j = i + 1

      while (j < tokens.length && tokens[j].type !== 'thead_close') {
        if (tokens[j].type === 'th_open' && tokens[j + 1]?.type === 'inline') {
          headers.push(tokens[j + 1].content)
          j += 2
        } else {
          j++
        }
      }

      j++ // skip thead_close

      // --- annotate <td> tokens in <tbody> ---
      while (j < tokens.length && tokens[j].type !== 'table_close') {
        if (tokens[j].type === 'tr_open') {
          let col = 0
          j++
          while (j < tokens.length && tokens[j].type !== 'tr_close') {
            if (tokens[j].type === 'td_open') {
              if (col < headers.length) {
                tokens[j].attrSet('data-column', headers[col])
              }
              col++
            }
            j++
          }
        } else {
          j++
        }
      }

      i = j + 1 // skip past table_close
    }
  })
}
