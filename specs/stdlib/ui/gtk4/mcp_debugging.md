# MCP Debugging

Part of the [Writing Native Apps](../gtk4.md) guide.

The MCP server is the best way to inspect a running AIVI GTK app from an editor agent or other local automation client.

Start an MCP host against:

```text
aivi mcp serve . --ui --allow-effects
```

See [MCP Server](../../../tools/mcp.md) for the full protocol and tool list. In practice, the native-app debugging loop looks like this.

## Launch or attach

Launch a target app under inspection with `aivi_gtk_launch`:

```json
{ "target": "demos/snake.aivi" }
```

Save the returned `sessionId`.

If the app is already running, use `aivi_gtk_discover` to find candidate sockets, then `aivi_gtk_attach` once you have the matching `socketPath` and token.

## Confirm the session and list widgets

Check that the session is alive with `aivi_gtk_hello`:

```json
{ "sessionId": "<sessionId>" }
```

Then list stable widget handles with `aivi_gtk_listWidgets`.

This is where widget `id="..."` names pay off. Give important inputs, buttons, panes, and dialogs explicit names so you can target them by `name` instead of guessing numeric ids.

## Inspect tree and signals

For one widget, call `aivi_gtk_inspectWidget`:

```json
{ "sessionId": "<sessionId>", "name": "saveButton" }
```

For the whole mounted tree, call `aivi_gtk_dumpTree`.

When the problem is state rather than layout, inspect the reactive layer with `aivi_gtk_listSignals` and `aivi_gtk_inspectSignal`. This is especially useful for checking whether:

- an `onClick` or `onInput` handler wrote the expected signal,
- an `EventHandle` moved through `running`, `result`, or `error`,
- a derived signal is stale because the wrong source signal is feeding it.

## Reproduce user input

Use the mutation tools to drive the app the same way a user would:

- `aivi_gtk_click`
- `aivi_gtk_type`
- `aivi_gtk_focus`
- `aivi_gtk_moveFocus`
- `aivi_gtk_select`
- `aivi_gtk_scroll`
- `aivi_gtk_keyPress`

A typical flow is:

1. focus a known widget,
2. type or click,
3. re-read the widget with `inspectWidget`,
4. re-read the relevant signal with `inspectSignal`.

That lets you answer both halves of a UI bug: "did the host widget change?" and "did the reactive state graph change?"

## Practical checklist

When you build a GTK app that you expect to debug later, these habits help immediately:

- add `id="..."` to important widgets,
- keep event-handle names descriptive (`saveDraft`, `refreshMailbox`, `closeSettings`),
- derive display-only values with `->>` so they appear as first-class signals in inspection output,
- keep low-level raw-signal usage localized so the inspectable graph stays easy to read.
