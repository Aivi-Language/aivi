# UI Virtual DOM

The `aivi.ui` module defines a **typed Virtual DOM** (`VNode msg`). Programs construct `VNode` trees and leave rendering + diffing to the runtime.

## Core Types

<<< ../../snippets/from_md/05_stdlib/04_ui/02_vdom/block_01.aivi{aivi}

### `VNode msg`

- `Element tag attrs children` is an HTML-like node (`tag : Text`).
- `TextNode text` is a text leaf.
- `Keyed key node` attaches a stable key used by the diff/patch protocol (useful for lists).

### `Attr msg`

Attributes are typed values, not raw strings:

- `Class Text`, `Id Text`
- `Style { ... }` where the style value is a record (see Layout units and CSS records below)
- `OnClick msg`, `OnInput (Text -> msg)` for event wiring
- `Attr Text Text` for unknown/escape-hatch attributes

## Constructing Nodes

<<< ../../snippets/from_md/05_stdlib/04_ui/02_vdom/block_02.aivi{aivi}

## Rendering + Diffs

`aivi.ui` exposes runtime-backed functions:

- `renderHtml : VNode msg -> Text` renders a `VNode` tree to HTML (including stable `data-aivi-node` ids).
- `diff : VNode msg -> VNode msg -> List PatchOp` computes a patch stream between trees.
- `patchToJson : List PatchOp -> Text` encodes patch ops to JSON for the browser client.
- `eventFromJson : Text -> Result LiveError Event` decodes browser events.

## Style Records (Typed CSS Data)

The `style={ ... }` attribute expects a record, so `<|` patching works naturally:

```aivi
base = { width: 10px, display: "block" }
next = base <| { width: 12px }
```

Style values are not restricted to `Text`. The runtime renderer recognizes common shapes:

- `Text`, `Int`, `Float`, `Bool`
- `aivi.ui.layout` unit constructors like `Px 1`, `Em 2`, `Pct 50` (via literals `1px`, `2em`, `50%`)
- `{ r: Int, g: Int, b: Int }` as a CSS `#rrggbb` color
