# HTML Sigil (`~html{ ... }`)

The `~html{ ... }` sigil parses HTML-like syntax and lowers it to `aivi.ui.VNode msg` constructors.

`~html{ ... }` is **typed templating**: it produces `VNode` values, not HTML strings.

## Splices

Use `{ expr }` inside `~html{}` to splice an AIVI expression:

- In child position, the expected type is `VNode msg`. If the splice is `Text` (or implements `ToText`), it is coerced via `text` / `toText`.
- In attribute position, `...={expr}` is type-checked against the attribute's expected type (e.g. `style` expects a record).

<<< ../../snippets/from_md/05_stdlib/04_ui/03_html/block_01.aivi{aivi}

## Attributes

The compiler lowers some attributes to typed constructors:

- `class="..."` -> `aivi.ui.className`
- `id="..."` -> `aivi.ui.id`
- `style={ expr }` -> `aivi.ui.style expr` (expects a record; see `aivi.ui.layout` for units like `10px`, `50%`)
- `onClick={ msg }` -> `aivi.ui.onClick msg`
- `onInput={ f }` -> `aivi.ui.onInput f` where `f : Text -> msg`

All other attributes lower to `aivi.ui.attr name value`:

- `title="Hello"` -> `attr "title" "Hello"`
- `data-x={ expr }` -> `attr "data-x" (toText expr)` (via expected-type `Text` coercion)

## Keys

The `key=` attribute is special-cased to produce keyed nodes:

- `<li key="k">...</li>` lowers to `aivi.ui.keyed "k" (element "li" ...)`.

## Whitespace

Whitespace-only text between tags (indentation/newlines) is ignored so templates can be indented without creating extra `TextNode`s.

## Multiple Roots

If a `~html{ ... }` sigil contains multiple top-level nodes, it is wrapped in a synthetic `<div>...</div>` to produce a single `VNode`.
