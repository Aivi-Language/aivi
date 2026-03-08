# HTML Sigil (`~<html>...</html>`)

<!-- quick-info: {"kind":"module","name":"aivi.ui"} -->
The `~<html>...</html>` sigil lets you write HTML-shaped UI trees directly inside AIVI code and lowers them to typed `aivi.ui.VNode msg` values.

This is typed templating, not string templating: the result is a `VNode`, so the compiler can still help with structure and event wiring.
<!-- /quick-info -->
<div class="import-badge">use aivi.ui</div>

For the underlying `VNode`/`Attr` data model, see [UI Virtual DOM](./vdom.md). For the parser-level overview of structured sigils, see [Syntax: operators](../../syntax/operators.md#html-and-gtk-sigils).

## What it is for

Use the HTML sigil when you want browser-style UI structure without building `VNode` constructors by hand. It is especially helpful for:

- rendering small HTML views clearly,
- mixing static markup with dynamic values,
- attaching typed event handlers,
- building reusable components that still lower to `VNode` trees.

## Splices

A child splice uses `{ expr }` inside element content. If `expr` already has type `VNode msg`, it is inserted directly. If it is `Text` or another `ToText` value, the compiler coerces it into a `TextNode` and inserts `toText` when needed.

<<< ../../snippets/from_md/stdlib/ui/html/block_01.aivi{aivi}


In attribute position, write `name={ expr }`. The expression is type-checked against that attribute's expected type. Typed attributes keep their specific types, while generic attributes elaborate against `Text`.

<<< ../../snippets/from_md/stdlib/ui/html/block_02.aivi{aivi}


## Attributes

The sigil special-cases a small set of common HTML-looking attributes:

- `class="..."` → `Class "..."`
- `id="..."` → `Id "..."`
- `style={ expr }` → `Style expr` (expects a record; see `aivi.ui.layout` for units such as `10px` and `50%`)
- `onClick={ msg }` → `OnClick msg`
- `onClickE={ f }` → `OnClickE f` where `f : Click -> msg`
- `onInput={ f }` → `OnInput f` where `f : Text -> msg`
- `onInputE={ f }` → `OnInputE f` where `f : Input -> msg`
- `onKeyDown={ f }`, `onKeyUp={ f }`, `onPointerDown={ f }`, `onPointerUp={ f }`, and `onPointerMove={ f }` lower to their matching typed handlers
- `onFocus={ msg }` and `onBlur={ msg }` carry plain messages

Any other attribute form becomes `Attr name value`:

- `title="Hello"` → `Attr "title" "Hello"`
- `data-x={ expr }` → `Attr "data-x" (toText expr)`
- `disabled` → `Attr "disabled" "true"`

The special cases are syntax-sensitive. For example, `class="card"` lowers to `Class "card"`, but `class={ dynamicClass }` falls back to `Attr "class" (toText dynamicClass)`.

For the event payload record shapes behind `Click`, `Input`, keyboard, and pointer handlers, see [UI Virtual DOM](./vdom.md#event-payloads).

## Component tags

Uppercase or dotted tag names are treated as component calls instead of intrinsic HTML elements. They use record-based lowering:

- `<Card ...>...</Card>`
- `<Ui.Card ...>...</Ui.Card>`

Lowering shape:

- `Card { title: "Hello", children: [...] }`
- `Ui.Card { title: "Hello", children: [...] }`

All attributes become record fields, and child nodes become a `children` field.

Lowercase tags still lower to intrinsic `Element` nodes.

## Keys

On intrinsic lowercase tags, the `key=` attribute is special-cased so list rendering can keep stable identity:

- `<li key="k">...</li>` lowers to `Keyed "k" (Element "li" ...)`

Use keys when list items may move, be inserted, or be removed and you want diffing to stay stable. On component tags, `key` is just another record field because component tags use record-based lowering.

## Whitespace

Whitespace-only text between tags is ignored. That lets you indent templates cleanly without accidentally creating extra `TextNode` values.

## Multiple roots

`~<html>...</html>` must contain exactly one top-level node. If you need several siblings, wrap them in a single root element such as a `<div>` or another layout container. Multiple roots produce diagnostic `E1601`.
