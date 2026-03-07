# HTML Sigil (`~<html>...</html>`)

<!-- quick-info: {"kind":"module","name":"aivi.ui"} -->
The `~<html>...</html>` sigil lets you write HTML-shaped UI trees directly inside AIVI code and lowers them to typed `aivi.ui.VNode msg` values.

This is typed templating, not string templating: the result is a `VNode`, so the compiler can still help with structure and event wiring.
<!-- /quick-info -->
<div class="import-badge">use aivi.ui</div>

## What it is for

Use the HTML sigil when you want browser-style UI structure without building `VNode` constructors by hand. It is especially helpful for:

- rendering small HTML views clearly,
- mixing static markup with dynamic values,
- attaching typed event handlers,
- building reusable components that still lower to `VNode` trees.

## Splices

Inline expressions use `{ expr }`:

<<< ../../snippets/from_md/stdlib/ui/html/splices_02.aivi{aivi}

If the splice is `Text` (or implements `ToText`), it is coerced by wrapping it with `TextNode` and inserting `toText` when needed.

In attribute position, `...={expr}` is type-checked against the attribute's expected type. For example, `style` expects a record rather than a raw CSS string.

<<< ../../snippets/from_md/stdlib/ui/html/splices_02.aivi{aivi}

## Attributes

Some HTML-looking attributes lower to more specific typed constructors:

- `class="..."` → `Class "..."`
- `id="..."` → `Id "..."`
- `style={ expr }` → `Style expr` (expects a record; see `aivi.ui.layout` for units such as `10px` and `50%`)
- `onClick={ msg }` → `OnClick msg`
- `onInput={ f }` → `OnInput f` where `f : Text -> msg`

Any attribute without a special typed lowering becomes `Attr name value`:

- `title="Hello"` → `Attr "title" "Hello"`
- `data-x={ expr }` → `Attr "data-x" (toText expr)`

## Component tags

Uppercase or dotted tag names are treated as component calls instead of intrinsic HTML elements:

- `<Card ...>...</Card>`
- `<Ui.Card ...>...</Ui.Card>`

Lowering shape:

- `Card [attrs...] [children...]`
- `Ui.Card [attrs...] [children...]`

Lowercase tags still lower to intrinsic `Element` nodes.

## Keys

The `key=` attribute is special-cased so list rendering can keep stable identity:

- `<li key="k">...</li>` lowers to `Keyed "k" (Element "li" ...)`

Use keys when list items may move, be inserted, or be removed and you want diffing to stay stable.

## Whitespace

Whitespace-only text between tags is ignored. That lets you indent templates cleanly without accidentally creating extra `TextNode` values.

## Multiple roots

`~<html>...</html>` must contain exactly one top-level node. If you need several siblings, wrap them in a single root element such as a `<div>` or another layout container.
