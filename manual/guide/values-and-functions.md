# Values & Functions

At the top level, AIVI keeps values and functions separate:

- `value` declares a named constant expression
- `func` declares a named pure function

That split keeps intent obvious in larger modules and matches the current surface language directly.

## Values

A `value` binds a name to a single expression:

```aivi
value answer : Int = 42
value greeting : Text = "Hello"
value isReady : Bool = True
```

Type annotations are optional when the compiler can infer them, but they are useful in public modules and documentation.

Values can refer to earlier values:

```aivi
value width : Int = 24
value height : Int = 20
value cellCount : Int = width * height
```

Values can also hold records and lists:

```aivi
type BoardSize = {
    width: Int,
    height: Int
}

value boardSize : BoardSize = {
    width: 24,
    height: 20
}

value checkpoints : List Int = [
    4,
    8,
    12
]
```

## Functions

A `func` declaration names a pure function:

```aivi
type Int -> Int -> Int
func add = x y =>
    x + y

value total = add 3 4
```

Function signatures can live on a preceding `type` line, and the `func` header keeps parameters unannotated.

```aivi
type Text -> Text
func greet = name =>
    "Hello, {name}!"

value greetingText = greet "Ada"
```

## Contextual function inference

Local same-module functions can sometimes be inferred from nearby usage even without a standalone
`type` line:

```aivi
func keep = value =>
    value

value chosen : Int = keep 1
```

This first slice is intentionally conservative. It only infers same-module monomorphic `func`
declarations when nearby callsites, partial applications, parameter annotations, or result
expectations provide enough information to prove one concrete signature. Underconstrained helpers
like `func id = x => x` still need an explicit `type` line.

## Multiple parameters

Parameters are separated by spaces, not commas:

```aivi
type Int -> Int -> Int -> Bool
func between = low high n =>
    n >= low and n <= high

value scoreAllowed = between 0 100 42
```

## Multi-line bodies

Function bodies are still just expressions, so multi-line definitions usually lean on pipes:

```aivi
type Int -> Text
func describeScore = score => score >= 50
 T|> "good"
 F|> "keep going"

value scoreLabel = describeScore 88
```

When a stage needs both the old and new value, use `#name` instead of breaking out into a helper
just to simulate a local binding. If a stage only needs its current subject once, prefer `.` and
keep the pipe simple.

```aivi
type Text -> Text
func explainNormalization =
  |> #original trim original #clean
  |> "{original} becomes {clean}"
```

## Unary subject sugar

When a unary function starts from its argument directly, you can omit the explicit parameter and use `.` as the implicit subject:

```aivi
type Status = { status: Text }

type Status -> Text
func statusLineFor = .status

type Int -> Text
func scoreLineFor = "Score: {.}"

value statusText = statusLineFor { status: "ready" }
value scoreText = scoreLineFor 42
```

`.status` projects from the implicit subject; `{.}` interpolates it without naming a parameter.

The same shorthand also works for pipe-rooted bodies. When the unary body starts with a pipe
operator, you can omit the explicit `.` head and start directly with the stages:

```aivi
type Text -> Text
func trimStatus =
  |> trim (.)
```

If the unary input is intentionally ignored, give it an explicit descriptive name:

```aivi
type Int -> Text
func constantLabel = ignored =>
    "ready"
```

## Selected subject headers

When a function has multiple parameters but its body should immediately continue from one specific
argument, mark that argument with `!` and omit `=>`:

```aivi
type Int -> Int -> Int
func add = left right =>
    left + right

type Int -> Int -> Int
func addFrom = amount value!
  |> add amount

value total = addFrom 2 40
```

The marked parameter becomes the subject for the following continuation, so the next line can start
with `|>` directly. Here `amount value!` is equivalent to `amount value => value` followed by
the same stages. Select exactly one named parameter; `!` does not change argument order.
The same sugar also works for patch-rooted bodies:

```aivi
type Counter = {
    total: Int,
    ready: Bool
}

type Counter -> Int -> Counter
func bump = counter! amount
    <| {
        total: . + amount,
        ready: True,
    }

value bumped = bump ({ total: 2, ready: False }) 3
```

This patch-rooted form passes `aivi check`, but `aivi test` currently rejects this example
during typed-core lowering. Its intended result is `{ total: 5, ready: True }`.
The pipe-rooted examples above and below do execute through `aivi test`.

You can also select a projection from the preceding named parameter:

```aivi
type Z = { z: Int }

type Y = { y: Z }

type X = { x: Y }

type Int -> Int
func addOne = . + 1

type X -> Int
func readNested = state { x.y.z! }
  |> addOne

value nestedTotal =
    readNested (
        {
            x: { y: { z: 40 } }
        }
    )
```

`{ x.y.z! }` means "use `state.x.y.z` as the subject for the continuation." It is header sugar for
subject selection, not a general parameter-destructuring form.

## Calling functions

Call a function by writing the function name followed by its arguments:

```aivi
type Int -> Int -> Int
func area = width height =>
    width * height

value roomArea = area 5 8
```

If an argument is itself an expression, wrap it in parentheses:

```aivi
type Int -> Int -> Int
func area = width height =>
    width * height

value adjustedArea = area (2 + 3) (4 * 2)
```

That includes negative literals in call position, for example `abs (-3)` rather than `abs -3`.

## Partial application

Functions can be partially applied. Supplying fewer arguments returns another function:

```aivi
type Int -> Int -> Int
func multiply = left right =>
    left * right

value double = multiply 2
value ten = double 5
```

## Inline lambdas

Anonymous lambdas are ordinary expressions. Explicit lambdas can take one or more named parameters.
Use them when you want an inline adapter or predicate:

```aivi
type Coord = Coord Int Int

value items : List Coord = [
    Coord 0 0,
    Coord 1 1
]

value cell : Coord = Coord 1 1
value explicitMatch : Bool = any (coord => coord == cell) items

value next : Int = 0
  |> x => x + 1
```

For short unary predicates, AIVI also accepts a narrow shorthand based on the existing subject
syntax:

```aivi
type Score = { score: Int }

value items : List Int = [
    1,
    2,
    3
]

value threshold : Int = 2
value shorthandMatch : Bool = any (. == threshold) items
value highScore : Score -> Bool = .score >= 10
```

That shorthand is always unary and only applies to **composed** dot-rooted expressions such as
`. == threshold` or `.score >= 10`. Bare `.` and `.field` keep their established ambient-subject meaning
so existing pipe, patch, and unary-subject code keeps reading the same way.

## Named helpers and inline lambdas

Inline lambdas now work, but named helpers are still better when logic is reused or deserves a
stable name:

```aivi
type Text -> Text
func trimStatus =
  |> trim (.)

type Text -> Text
func decorateStatus = "[{.}]"

value shownStatus = " ready "
  |> trimStatus
  |> decorateStatus
```

That style gives each step a reusable name and often reads better in longer pipes.

When a unary helper body starts with a pipe operator, the leading-pipe form is equivalent to the
same helper written with an explicit unary subject head such as `func trimStatus = .` followed by
the same stages.

## Structural patches

The checker accepts the patch forms described here. Execution support is narrower:
see the [current patch-function execution limit](/guide/predicates#predicates-in-patches).

Use `<|` to produce an updated value without mutating the original:

```aivi
type User = {
    name: Text,
    isAdmin: Bool
}

value user : User = {
    name: "Ada",
    isAdmin: False
}

value promoted : User =
    user <| {
        name: "Grace",
        isAdmin: True,
    }
```

`patch { ... }` builds a reusable same-shape update function:

```aivi
type User = {
    name: Text,
    isAdmin: Bool
}

value promote : (User -> User) =
    patch {
        isAdmin: True,
    }
```

Selectors are relative to the current focus:

- record roots accept either `field` or `.<field>`; nested field steps still use dots, as in `profile.name`
- `[*]` traverses `List` elements or `Map` values
- `[predicate]` filters `List` elements
- `["key"]` or `[.key == "id-1"]` selects `Map` entries

Inside patch predicates, use dot-prefixed projections such as `.active`, `.price`, `.key`, and `.value`.

The current checked slice also accepts constructor focus through `Some`, `Ok`, `Err`, `Valid`, `Invalid`, and same-module constructors with exactly one payload field.

`:=` stores a function value as data instead of applying it during patch execution.

```aivi
type Int -> Int
func increment = . + 1

type Counter = {
    step: Int -> Int
}

value keepCounter : (Counter -> Counter) =
    patch {
        step: := increment,
    }
```

Structural removal syntax (`field: -`) removes a field from the result type. See [Record Patterns § Patch removal](/guide/record-patterns#patch-removal) for details.

## Type annotations

Annotate a `value` with `:`, and give a named function its signature on a preceding `type` line:

```aivi
value count : Int = 0

type Int -> Int
func negate = n =>
    0 - n
```

## Summary

| Form | Example |
| --- | --- |
| Value | `value answer : Int = 42` |
| Function | `type Int -> Int -> Int` / `func add = x y => x + y` |
| Function call | `add 3 4` |
| Partial application | `value double = multiply 2` |
| Patch apply | `value promoted = user <\| { isAdmin: True }` |
