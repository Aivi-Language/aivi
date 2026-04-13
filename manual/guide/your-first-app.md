# Build a Small Task Tracker

This tutorial walks through one complete beginner-sized AIVI app: a small GTK task tracker.
By the end, you will have an app that can:

- capture text input
- add tasks to a list
- toggle tasks done or undone
- filter the list by **All**, **Active**, or **Done**

The point is not to build the world's fanciest task app. The point is to make the AIVI model click:
**types describe the data, signals carry change, pure functions describe state transitions, and the
UI simply reflects the signal graph**.

## Step 1: Model the data first

Before building UI, define what exists in the app.

```aivi
type Filter =
  | All
  | Active
  | Done

type Todo = {
    id: Int,
    text: Text,
    done: Bool
}

type State = {
    nextId: Int,
    draft: Text,
    filter: Filter,
    items: List Todo
}

type Event =
  | DraftChanged Text
  | AddTodo
  | ToggleTodo Int
  | SetFilter Filter
  | ClearDone

value initial : State = {
    nextId: 1,
    draft: "",
    filter: All,
    items: []
}
```

This is already a functional-programming habit worth keeping: do not start with callbacks or widget
handlers. Start by making the application's shape explicit.

## Step 2: Write the pure state logic

Now we describe what each event means. This is ordinary pure code: given an event and the current
state, return the next state.

```aivi
type Filter =
  | All
  | Active
  | Done

type Todo = {
    id: Int,
    text: Text,
    done: Bool
}

type State = {
    nextId: Int,
    draft: Text,
    filter: Filter,
    items: List Todo
}

type Event =
  | DraftChanged Text
  | AddTodo
  | ToggleTodo Int
  | SetFilter Filter
  | ClearDone

type Todo -> Bool
func isOpen = todo => todo.done
 T|> False
 F|> True

type Filter -> Todo -> Bool
func matchesFilter = current todo => current
 ||> All    -> True
 ||> Active -> isOpen todo
 ||> Done   -> todo.done

type Todo -> Text
func todoLabel = todo => todo.done
 T|> "[x] {todo.text}"
 F|> "[ ] {todo.text}"

type Int -> Todo -> Todo
func toggleItem = target todo => (todo.id == target, todo.done)
 ||> (True, True)  -> todo <| { done: False }
 ||> (True, False) -> todo <| { done: True }
 ||> (False, _)    -> todo

type State -> State
func addItem = state => trim state.draft == ""
 T|> state
 F|> state <| { nextId: state.nextId + 1, draft: "", items: append state.items [{ id: state.nextId, text: trim state.draft, done: False }] }

type Todo -> Bool
func keepActive = todo =>
    isOpen todo

type State -> State
func clearCompleted = state =>
    state <| { items: filter keepActive state.items }

type Event -> State -> State
func step = event state => event
 ||> DraftChanged text -> state <| { draft: text }
 ||> AddTodo           -> addItem state
 ||> ToggleTodo id     -> state <| { items: map (toggleItem id) state.items }
 ||> SetFilter current -> state <| { filter: current }
 ||> ClearDone         -> clearCompleted state
```

The important part is the `step` function. It is the same idea you would use in Elm, Redux, or a
state machine: one closed event type in, one new state out.

## Step 3: Turn UI events into domain events

GTK widgets can emit signal payloads directly into your reactive graph.

```aivi
type Filter =
  | All
  | Active
  | Done

type Event =
  | DraftChanged Text
  | AddTodo
  | ToggleTodo Int
  | SetFilter Filter
  | ClearDone

signal draftChanged : Signal Text
signal addClick : Signal Unit
signal toggleTodo : Signal Int
signal setFilter : Signal Filter
signal clearDone : Signal Unit

signal event : Signal Event = draftChanged | addClick | toggleTodo | setFilter | clearDone
  ||> draftChanged text => DraftChanged text
  ||> addClick _ => AddTodo
  ||> toggleTodo id => ToggleTodo id
  ||> setFilter filter => SetFilter filter
  ||> clearDone _ => ClearDone
```

This is the bridge from UI events to your app's own vocabulary. The buttons and entry do not mutate
state directly; they emit values, and the signal graph folds those values into state.

## Step 4: Derive the state the UI needs

Once `event` exists, accumulation gives us the live application state:

```aivi
type Filter =
  | All
  | Active
  | Done

type Todo = {
    id: Int,
    text: Text,
    done: Bool
}

type State = {
    nextId: Int,
    draft: Text,
    filter: Filter,
    items: List Todo
}

type Event =
  | DraftChanged Text
  | AddTodo
  | ToggleTodo Int
  | SetFilter Filter
  | ClearDone

value initial : State = {
    nextId: 1,
    draft: "",
    filter: All,
    items: []
}

type Todo -> Bool
func isOpen = todo => todo.done
 T|> False
 F|> True

type Filter -> Todo -> Bool
func matchesFilter = current todo => current
 ||> All    -> True
 ||> Active -> isOpen todo
 ||> Done   -> todo.done

type Event -> State -> State
func step = event state => event
 ||> DraftChanged text -> state <| { draft: text }
 ||> AddTodo           -> state
 ||> ToggleTodo _      -> state
 ||> SetFilter current -> state <| { filter: current }
 ||> ClearDone         -> state

type State -> List Todo
func visibleTodos = state =>
    filter (matchesFilter state.filter) state.items

type State -> Bool
func hasDraft = state =>
    trim state.draft != ""

type State -> Text
func footer = state => state.items
  |> length
  |> "{.} total tasks"

signal event : Signal Event

signal state = event
 +|> initial step

signal draftText = state
  |> .draft

signal visibleItems = state
  |> visibleTodos

signal canAdd = state
  |> hasDraft

signal footerText = state
  |> footer
```

This is the core AIVI move: **declare the dependency graph once**. `draftText`, `visibleItems`,
`canAdd`, and `footerText` all stay correct because they are defined from `state`.

## Step 5: Build the UI

Now the UI is mostly straightforward. It just reads from signals and emits event payloads back into
the graph.

```aivi
type Filter =
  | All
  | Active
  | Done

type Todo = {
    id: Int,
    text: Text,
    done: Bool
}

type State = {
    nextId: Int,
    draft: Text,
    filter: Filter,
    items: List Todo
}

type Event =
  | DraftChanged Text
  | AddTodo
  | ToggleTodo Int
  | SetFilter Filter
  | ClearDone

value initial : State = {
    nextId: 1,
    draft: "",
    filter: All,
    items: []
}

type Todo -> Bool
func isOpen = todo => todo.done
 T|> False
 F|> True

type Filter -> Todo -> Bool
func matchesFilter = current todo => current
 ||> All    -> True
 ||> Active -> isOpen todo
 ||> Done   -> todo.done

type Todo -> Text
func todoLabel = todo => todo.done
 T|> "[x] {todo.text}"
 F|> "[ ] {todo.text}"

type Int -> Todo -> Todo
func toggleItem = target todo => (todo.id == target, todo.done)
 ||> (True, True)  -> todo <| { done: False }
 ||> (True, False) -> todo <| { done: True }
 ||> (False, _)    -> todo

type State -> State
func addItem = state => trim state.draft == ""
 T|> state
 F|> state <| { nextId: state.nextId + 1, draft: "", items: append state.items [{ id: state.nextId, text: trim state.draft, done: False }] }

type Todo -> Bool
func keepActive = todo =>
    isOpen todo

type State -> State
func clearCompleted = state =>
    state <| { items: filter keepActive state.items }

type Event -> State -> State
func step = event state => event
 ||> DraftChanged text -> state <| { draft: text }
 ||> AddTodo           -> addItem state
 ||> ToggleTodo id     -> state <| { items: map (toggleItem id) state.items }
 ||> SetFilter current -> state <| { filter: current }
 ||> ClearDone         -> clearCompleted state

type State -> List Todo
func visibleTodos = state =>
    filter (matchesFilter state.filter) state.items

type State -> Bool
func hasDraft = state =>
    trim state.draft != ""

type State -> Text
func footer = state => state.items
  |> length
  |> "{.} total tasks"

signal draftChanged : Signal Text
signal addClick : Signal Unit
signal toggleTodo : Signal Int
signal setFilter : Signal Filter
signal clearDone : Signal Unit

signal event : Signal Event = draftChanged | addClick | toggleTodo | setFilter | clearDone
  ||> draftChanged text => DraftChanged text
  ||> addClick _ => AddTodo
  ||> toggleTodo id => ToggleTodo id
  ||> setFilter filter => SetFilter filter
  ||> clearDone _ => ClearDone

signal state = event
 +|> initial step

signal draftText = state
  |> .draft

signal visibleItems = state
  |> visibleTodos

signal canAdd = state
  |> hasDraft

signal footerText = state
  |> footer

value main =
    <Window title="AIVI Task Tracker" defaultWidth={420} defaultHeight={480}>
        <Box orientation="vertical" spacing={12} marginTop={16} marginBottom={16} marginStart={16} marginEnd={16}>
            <Label text="Task Tracker" />
            <Label text="Type a task, press Add, then click an item to toggle it done." />
            <Entry text={draftText} onChange={draftChanged} />
            <Button label="Add task" onClick={addClick} sensitive={canAdd} />
            <Box orientation="horizontal" spacing={8}>
                <Button label="All" onClick={setFilter All} />
                <Button label="Active" onClick={setFilter Active} />
                <Button label="Done" onClick={setFilter Done} />
                <Button label="Clear done" onClick={clearDone} />
            </Box>
            <Box orientation="vertical" spacing={6}>
                <each of={visibleItems} as={todo} key={todo.id}>
                    <Button label={todoLabel todo} onClick={toggleTodo todo.id} />
                </each>
            </Box>
            <Label text={footerText} />
        </Box>
    </Window>

export main
```

## What this app teaches

| Concept | Where it appears |
| --- | --- |
| **Closed types** | `Filter` and `Event` make the app vocabulary explicit |
| **Plain records** | `Todo` and `State` hold application data |
| **Pure functions** | `step`, `addItem`, `toggleItem`, `visibleTodos` |
| **Signals** | `event`, `state`, `draftText`, `visibleItems`, `canAdd`, `footerText` |
| **Merge and accumulation** | UI events merge into `event`; `+|>` folds them into `state` |
| **GTK markup** | `Entry`, `Button`, `Box`, `Label`, and `<each>` |
| **Functional style without ceremony** | No mutable variables, no callback soup, no unnecessary memo names |

## The data flow

```
Entry + buttons  →  UI event signals
                        ↓
               merged into Event values
                        ↓
                 +|> folds into State
                        ↓
            derived signals feed the GTK tree
```

Every arrow is declared in the source. There is no hidden mutation site to go hunting for later.

## Next steps

- [How-to Guides](/how-to/) — move from the learning path to concrete tasks
- [Signals](/guide/signals) — deeper reference for merge, accumulation, and derivation
- [Markup & UI](/guide/markup) — widget catalog and control nodes
- [Snake](/guide/building-snake) — optional bigger example once this app feels comfortable
