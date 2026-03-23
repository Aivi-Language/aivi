# Markup

AIVI markup is a tree of GTK widget declarations. It looks like JSX, but compiles to
native GTK4/libadwaita widgets — no web rendering, no virtual DOM, no Electron.

## Basic tags

```aivi
val main =
    <Window title="Hello AIVI">
        <Box orientation={Vertical} spacing={8}>
            <Label text="Welcome!" />
            <Button label="Click me" />
        </Box>
    </Window>
```

Tags are PascalCase GTK widget names. Attributes set widget properties.
Self-closing tags (`<Label ... />`) have no children.

The outer `<Window>` is the root widget. Every AIVI application has exactly one `export main`
with a `<Window>` at the top.

## String interpolation

Use `{expression}` inside a double-quoted string to embed a value:

```aivi
val score = 42
val msg   = "Your score is {score} points!"

val main =
    <Window title="Score: {score}">
        <Label text="You scored {score} points!" />
    </Window>
```

The interpolation works in both `val` strings and markup attribute strings.

## Binding signals to attributes

When an attribute value is wrapped in `{...}` with a signal, the widget re-renders automatically
when the signal changes:

```aivi
sig count : Signal Int = 0

sig labelText : Signal Text =
    count
     |> \n => "Clicked {n} times"

val main =
    <Window title="Counter">
        <Label text={labelText} />
    </Window>
```

The `<Label>` text updates every time `labelText` changes — which happens whenever `count`
changes. There is no explicit update call.

## The each tag

`<each>` renders a list of items. It requires a `key` attribute to help the runtime identify
stable items across updates:

```aivi
type User = { id: Int, name: Text }

sig users : Signal (List User)

val main =
    <Window title="Users">
        <Box orientation={Vertical} spacing={4}>
            <each of={users} as={user} key={user.id}>
                <Label text={user.name} />
            </each>
        </Box>
    </Window>
```

- `of={users}` — the list signal to iterate.
- `as={user}` — the name bound to each item inside the block.
- `key={user.id}` — a stable unique identifier for each item.

The `key` attribute is required. It allows the runtime to reuse widgets for unchanged items
rather than rebuilding the whole list.

## Nested each

```aivi
sig boardRows : Signal (List BoardRow)

val board =
    <Box orientation={Vertical} spacing={2}>
        <each of={boardRows} as={row} key={row.id}>
            <Box orientation={Horizontal} spacing={2}>
                <each of={row.cells} as={cell} key={cell.id}>
                    <Label text={cellGlyph cell.kind} />
                </each>
            </Box>
        </each>
    </Box>
```

Each row is a horizontal `<Box>`, and each cell inside it is a `<Label>`.
This is the exact structure in the Snake demo.

## The match tag

`<match>` and `<case>` are markup-level pattern matching. They render different widget trees
based on a value:

```aivi
sig status : Signal Status

val statusView =
    <match value={status}>
        <case pattern={Running}>
            <Label text="Game is running" />
        </case>
        <case pattern={GameOver}>
            <Label text="Game over!" />
        </case>
    </match>
```

Like `\|\|>`, `<match>` is exhaustive — all variants must be covered.

## The show tag

`<show>` renders its children only when a condition is true:

```aivi
sig isLoggedIn : Signal Bool

val loginButton =
    <show when={isLoggedIn}>
        <Button label="Log out" />
    </show>
```

When `isLoggedIn` is `False`, the `<Button>` is removed from the widget tree.

## Orientation and spacing

GTK `Box` is the main layout widget. `orientation` takes `Vertical` or `Horizontal`
(both are AIVI values of type `Orientation`). `spacing` is an `Int` in pixels.

```aivi
<Box orientation={Vertical} spacing={12}>
    <Label text="First" />
    <Label text="Second" />
</Box>
```

## Attribute expressions

Attribute values can be any AIVI expression:

```aivi
val cellSize = 32

val grid =
    <Box orientation={Horizontal} spacing={cellSize}>
        <Label text={"Width: " ++ toString boardWidth} />
    </Box>
```

## Summary

- Tags are GTK widget names in PascalCase.
- `{signal}` binds an attribute to a live signal.
- String interpolation: `"Hello {name}"`.
- `<each of={list} as={item} key={item.id}>` iterates a list signal.
- `<match value={signal}>` with `<case pattern=...>` arms for conditional rendering.
- `<show when={boolSignal}>` for presence/absence toggling.

[Next: Type Classes →](/tour/08-typeclasses)
