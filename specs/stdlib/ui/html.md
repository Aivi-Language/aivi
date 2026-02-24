# HTML Sigil (`~<html>...</html>`)

> **Status: Specified, not implemented in runtime v0.1.**

<!-- quick-info: {"kind":"module","name":"aivi.ui"} -->
The `~<html>...</html>` sigil allows embedding HTML inside Aivi code:syntax and lowers it to `aivi.ui.VNode msg` constructors.

`~<html>...</html>` is **typed templating**: it produces `VNode` values, not HTML strings.

<!-- /quick-info -->
<div class="import-badge">use aivi.ui</div>

## Splices

Use `{ expr }` inline:

<<< ../../snippets/from_md/stdlib/ui/html/splices_02.aivi{aivi}


If the splice is `Text` (or implements `ToText`), it is coerced by wrapping with `TextNode` (and inserting `toText` when needed).

- In attribute position, `...={expr}` is type-checked against the attribute's expected type (e.g. `style` expects a record).

<<< ../../snippets/from_md/stdlib/ui/html/splices_02.aivi{aivi}

## Attributes

The compiler lowers some attributes to typed constructors:

- `class="..."` -> `Class "..."`
- `id="..."` -> `Id "..."`
- `style={ expr }` -> `Style expr` (expects a record; see `aivi.ui.layout` for units like `10px`, `50%`)
- `onClick={ msg }` -> `OnClick msg`
- `onInput={ f }` -> `OnInput f` where `f : Text -> msg`

All other attributes lower to `Attr name value`:

- `title="Hello"` -> `Attr "title" "Hello"`
- `data-x={ expr }` -> `Attr "data-x" (toText expr)` (via expected-type `Text` coercion)

## Component tags

Uppercase or dotted tag names are treated as component calls instead of intrinsic HTML elements:

- `<Card ...>...</Card>`
- `<Ui.Card ...>...</Ui.Card>`

Lowering shape:

- `Card [attrs...] [children...]`
- `Ui.Card [attrs...] [children...]`

Intrinsic lowercase tags still lower to `Element`.

## Keys

The `key=` attribute is special-cased to produce keyed nodes:

- `<li key="k">...</li>` lowers to `Keyed "k" (Element "li" ...)`.

## Whitespace

Whitespace-only text between tags (indentation/newlines) is ignored so templates can be indented without creating extra `TextNode`s.

## Multiple Roots

If a `~<html>...</html>` sigil contains multiple top-level nodes, compilation fails with an error. Wrap your nodes in a single root element (usually a `<div>`).
