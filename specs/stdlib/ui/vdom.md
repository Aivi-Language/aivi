# UI Virtual DOM

<!-- quick-info: {"kind":"module","name":"aivi.ui"} -->
The `aivi.ui` module defines a typed virtual DOM, `VNode msg`. You build `VNode` trees in AIVI, and the runtime can render them to HTML or compute diffs between versions.
<!-- /quick-info -->
<div class="import-badge">use aivi.ui</div>

If you have used React, Elm, or other virtual-DOM systems, the idea will feel familiar. The difference here is that the tree, attributes, and events are ordinary typed AIVI values rather than framework-specific objects.

If you prefer template-like authoring, pair this page with the [`~<html>...</html>` sigil](./html.md). This page focuses on the underlying data model and the runtime functions that operate on it.

## Core types

`aivi.ui` revolves around three pieces:

- `VNode msg` for the tree itself,
- `Attr msg` for typed attributes and event bindings,
- `PatchOp` for the runtime diff output produced by `diff`.

You will usually construct `VNode` and `Attr` values directly and let the runtime produce `PatchOp` values for you.

### `VNode msg`

- `Element tag attrs children` is an HTML-like node where `tag : Text`.
- `TextNode text` is a text leaf.
- `Keyed key node` attaches a stable key, which is especially useful when rendering lists that may reorder or change size.

### `Attr msg`

Attributes are typed values rather than raw string pairs. That keeps common attributes and events explicit in the type system while still leaving an escape hatch for custom attributes.

- `Class Text`, `Id Text`
- `Style { ... }` where the style value is a record
- `OnClick msg`, `OnInput (Text -> msg)` for common event wiring
- `OnClickE (Click -> msg)` and `OnInputE (Input -> msg)` for richer payloads
- keyboard, pointer, focus, and blur handlers
- `Attr Text Text` as an escape hatch for attributes without a dedicated typed constructor

## Event payloads

The richer handlers use ordinary record types, so you can inspect the event data without leaving typed AIVI values:

- `Click` / `ClickEvent = { button: Int, alt: Bool, ctrl: Bool, shift: Bool, meta: Bool }`
- `Input` / `InputEvent = { value: Text }`
- `KeyboardEvent = { key: Text, code: Text, alt: Bool, ctrl: Bool, shift: Bool, meta: Bool, repeat: Bool, isComposing: Bool }`
- `PointerEvent = { pointerId: Int, pointerType: Text, button: Int, buttons: Int, clientX: Float, clientY: Float, alt: Bool, ctrl: Bool, shift: Bool, meta: Bool }`

`OnFocus` and `OnBlur` carry plain messages rather than event records.

## Constructing nodes

Start with the constructors and helpers below if you want to build trees directly in code:

- constructors: `Element`, `TextNode`, `Keyed`
- helper functions: `vElement`, `vText`, `vKeyed`

The snippet below shows the constructor form. Use whichever style reads more clearly in the surrounding code.

<<< ../../snippets/from_md/stdlib/ui/vdom/constructing_nodes.aivi{aivi}

## Rendering and diffs

`aivi.ui` exposes runtime-backed functions for the browser-oriented workflow:

- `renderHtml : VNode msg -> Text` renders a `VNode` tree to HTML, including stable `data-aivi-node` ids,
- `diff : VNode msg -> VNode msg -> List PatchOp` computes a patch stream between trees,
- `patchToJson : List PatchOp -> Text` encodes patch operations to JSON for a browser client.

When rendered, keyed nodes also carry `data-aivi-key`, which helps preserve identity across diffs.

In practice, that means you can keep your UI as pure data, compare old and new trees, and hand only the minimal patch information to the renderer. Tags, attribute names, and CSS property names are sanitized before rendering, so unsafe names are dropped or replaced with safe defaults.

## Style records (typed CSS data)

The `Style` attribute expects a record, so record patching with `<|` works naturally. In the [`~<html>...</html>` sigil](./html.md), `style={ ... }` lowers to the same `Style` value:

<<< ../../snippets/from_md/stdlib/ui/vdom/style_records_typed_css_data.aivi{aivi}

Style values are not limited to `Text`. The current renderer recognizes common shapes such as:

- `Text`, `Int`, `Float`, `Bool`,
- [`aivi.ui.layout`](./layout.md) unit constructors like `Px 1`, `Em 2`, `Rem 3`, `Vh 100`, `Vw 50`, and `Pct 50`,
- `{ r: Int, g: Int, b: Int }`, which renders as a CSS `#rrggbb` color.

Other values fall back to their textual representation, but explicit `Text` and layout units are usually clearer in examples and easier to refactor.

That makes style code much easier to refactor than long CSS strings built by hand.
