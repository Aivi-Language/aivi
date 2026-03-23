---
layout: home

hero:
  name: "AIVI"
  text: "Reactive apps for Linux"
  tagline: A purely functional, GTK-first language that makes desktop software as composable as spreadsheet formulas.
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
    details: Markup tags like &lt;Window&gt;, &lt;Button&gt;, and &lt;each&gt; compile directly to native GTK4 widgets via the AIVI runtime.
  - icon: 🔒
    title: No null. No exceptions. No loops.
    details: Types are closed, exhaustive, and null-free. Control flow is pattern matching and recursion. Bugs hide nowhere to go.
---

## A taste of AIVI

This complete program renders a counter with increment and decrement buttons.

```aivi
fun add:Int #x:Int #n:Int => n + x

@source button.clicked "increment"
sig added : Signal Int =
    0
    @|> add 1
    <|@ add 1

@source button.clicked "decrement"
sig removed : Signal Int =
    0
    @|> add 1
    <|@ add 1

sig count : Signal Int = added - removed

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

Each button owns its own recurrent signal. `@|>` starts the recurrence, `<|@` is the step.
`count` is a pure derived signal — the difference of two independent accumulators.
No event listeners. No mutable state. No async juggling.
