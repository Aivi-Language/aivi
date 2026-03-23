---
layout: home

hero:
  name: "AIVI"
  text: "Reactive apps for Linux"
  tagline: A purely functional, GTK-first language that makes desktop software as composable as spreadsheet formulas.
  image:
    src: /logo.svg
    alt: AIVI logo
  actions:
    - theme: brand
      text: Language Tour
      link: /tour/
    - theme: alt
      text: Introduction
      link: /introduction
    - theme: alt
      text: Playground
      link: /playground/

features:
  - icon: 🔁
    title: Signals, not callbacks
    details: Every value that changes over time is a signal. The runtime wires the dependency graph — you declare transformations.
  - icon: 🧩
    title: Pipe algebra
    details: Data flows left-to-right through typed pipes. Transform, gate, fan-out, and match — all as composable pipe operators.
  - icon: 🎨
    title: GTK / libadwaita first-class
    details: Markup tags like <Window>, <Button>, and <each> compile directly to native GTK4 widgets via the AIVI runtime.
  - icon: 🔒
    title: No null. No exceptions. No loops.
    details: Types are closed, exhaustive, and null-free. Control flow is pattern matching and recursion. Bugs hide nowhere to go.
---

## A taste of AIVI

This complete program renders a counter with increment and decrement buttons.

```aivi
type Msg = Increment | Decrement

fun apply:Int #msg:Msg #count:Int =>
    msg
     ||> Increment => count + 1
     ||> Decrement => count - 1

@source button.clicked "increment"
sig increment : Signal Msg = Increment

@source button.clicked "decrement"
sig decrement : Signal Msg = Decrement

sig count : Signal Int =
    0
    @|> apply increment
    <|@ apply decrement

sig label : Signal Text =
    count
     |> .toString

val main =
    <Window title="Counter">
        <Box orientation={Vertical} spacing={12}>
            <Label text={label} />
            <Button id="increment" label="+" />
            <Button id="decrement" label="−" />
        </Box>
    </Window>

export main
```

The signal `count` starts at `0` and is updated whenever `increment` or `decrement` fires.
No event listeners. No mutable state. No async juggling.
