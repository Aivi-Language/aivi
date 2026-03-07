# UI Virtual DOM

<!-- quick-info: {"kind":"module","name":"aivi.ui"} -->
The `aivi.ui` module defines a typed virtual DOM, `VNode msg`. You build `VNode` trees in AIVI, and the runtime handles rendering and diffing.
<!-- /quick-info -->
<div class="import-badge">use aivi.ui</div>

If you have used React, Elm, or other virtual-DOM systems, the idea will feel familiar. The difference here is that the tree, attributes, and events are ordinary typed AIVI values.

## Core types

<<< ../../snippets/from_md/stdlib/ui/vdom/style_records_typed_css_data.aivi{aivi}

### `VNode msg`

- `Element tag attrs children` is an HTML-like node where `tag : Text`.
- `TextNode text` is a text leaf.
- `Keyed key node` attaches a stable key, which is especially useful when rendering lists that may reorder or change size.

### `Attr msg`

Attributes are typed values rather than raw string pairs.

- `Class Text`, `Id Text`
- `Style { ... }` where the style value is a record
- `OnClick msg`, `OnInput (Text -> msg)` for common event wiring
- `OnClickE (Click -> msg)` and `OnInputE (Input -> msg)` for richer payloads
- keyboard, pointer, transition, animation, focus, and blur handlers
- `Attr Text Text` as an escape hatch for attributes without a dedicated typed constructor

## Constructing nodes

Start with the constructors and helpers below if you want to build trees directly in code:

<<< ../../snippets/from_md/stdlib/ui/vdom/constructing_nodes.aivi{aivi}

## Rendering and diffs

`aivi.ui` exposes runtime-backed functions for the browser-oriented workflow:

- `renderHtml : VNode msg -> Text` renders a `VNode` tree to HTML, including stable `data-aivi-node` ids,
- `diff : VNode msg -> VNode msg -> List PatchOp` computes a patch stream between trees,
- `patchToJson : List PatchOp -> Text` encodes patch operations to JSON for a browser client.

In practice, that means you can keep your UI as pure data, compare old and new trees, and hand only the minimal patch information to the renderer.

## Style records (typed CSS data)

The `style={ ... }` attribute expects a record, so record patching with `<|` works naturally:

<<< ../../snippets/from_md/stdlib/ui/vdom/style_records_typed_css_data.aivi{aivi}

Style values are not limited to `Text`. The renderer recognizes common shapes such as:

- `Text`, `Int`, `Float`, `Bool`,
- `aivi.ui.layout` unit constructors like `Px 1`, `Em 2`, and `Pct 50`,
- `{ r: Int, g: Int, b: Int }` as a CSS `#rrggbb` color.

That makes style code much easier to refactor than long CSS strings built by hand.
