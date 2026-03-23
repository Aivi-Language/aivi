# List Rendering

Rendering lists efficiently is one of the most common tasks in UI programming.
AIVI provides two complementary tools: `<each>` for markup and `*\|>` for signal-level
transformations.

## Basic list rendering with each

```aivi
type Task = {
    id:   Int,
    text: Text,
    done: Bool
}

sig tasks : Signal (List Task)

val taskList =
    <Box orientation={Vertical} spacing={4}>
        <each of={tasks} as={task} key={task.id}>
            <Box orientation={Horizontal} spacing={8}>
                <CheckButton active={task.done} />
                <Label text={task.text} />
            </Box>
        </each>
    </Box>
```

The `key` attribute is how the runtime tracks which widget corresponds to which item.
When the list updates, the runtime:
1. Reuses widgets for items with matching keys (no re-render for unchanged items).
2. Creates widgets for new keys.
3. Destroys widgets for removed keys.

**Always provide a stable, unique `key`.**

## Transforming lists before rendering

Use `*\|>` (map pipe) to transform each item in a list signal before passing it to `<each>`:

```aivi
type Task = { id: Int, text: Text, done: Bool }

type TaskView = {
    id:          Int,
    displayText: Text,
    styleClass:  Text
}

fun toTaskView:TaskView #task:Task =>
    {
        id:          task.id,
        displayText: task.text,
        styleClass:  task.done ||> True => "done" ||> False => "active"
    }

sig tasks : Signal (List Task)

sig taskViews : Signal (List TaskView) =
    tasks
     *|> toTaskView
```

`*\|>` applies `toTaskView` to every item in the list.
The result is a new list of the same length with transformed items.

## Filtering with ?\|> on list elements

To render only a subset of a list, use `List.filter` combined with the gate pipe:

```aivi
type Filter = All | Active | Done

sig currentFilter : Signal Filter

sig filteredTasks : Signal (List Task) =
    tasks
     &|> \ts =>
        currentFilter
         ||> All    => ts
         ||> Active => List.filter (\t => t.done == False) ts
         ||> Done   => List.filter (\t => t.done == True) ts
```

`filteredTasks` recomputes whenever `tasks` or `currentFilter` changes.

## Fan-out with *\|> and <\|*

The fan-out pattern maps a list signal into multiple per-item signals.
This is useful when each list item has its own independent reactive state.

```aivi
sig rowSignals : Signal (List (Signal BoardRow)) =
    boardRows
     *|> \row => row |> processRow

sig mergedRows : Signal (List BoardRow) =
    rowSignals
     <|* identity
```

`*\|>` fans out: one list signal → list of signals (one per item).
`<\|*` fans in: list of signals → one list signal (merging the results).

## Nested lists

The snake game renders a board as a list of rows, each containing a list of cells.
This is nested `<each>`:

```aivi
sig boardRows : Signal (List BoardRow)

val boardView =
    <Box orientation={Vertical} spacing={0}>
        <each of={boardRows} as={row} key={row.id}>
            <Box orientation={Horizontal} spacing={0}>
                <each of={row.cells} as={cell} key={cell.id}>
                    <Label text={cellGlyph cell.kind} />
                </each>
            </Box>
        </each>
    </Box>
```

The outer `<each>` iterates rows; the inner `<each>` iterates cells within each row.
Keys are scoped to their respective `<each>` block.

## Dynamic keys

The `key` attribute must be unique within a single `<each>` block but does not need to be
globally unique. Row IDs and cell IDs can both be integers starting from `0` as long as
they are unique within their own list.

## Computing list statistics

```aivi
sig taskCount : Signal Int =
    tasks
     |> List.length

sig doneCount : Signal Int =
    tasks
     |> List.filter (\t => t.done)
     |> List.length

sig activeCount : Signal Int =
    tasks
     |> List.filter (\t => t.done == False)
     |> List.length

sig statusText : Signal Text =
    activeCount
     |> \n => "{n} items remaining"
```

These are all derived signals — they update automatically when `tasks` changes.

## Summary

- `<each of={listSignal} as={item} key={item.id}>` renders a list.
- `key` is required and must be unique within the block.
- `*\|>` transforms every item in a list signal.
- Filter with `List.filter` applied to the list signal.
- `*\|>` fans out to per-item signals; `<\|*` fans them back in.
- Nest `<each>` blocks for 2D data structures.
