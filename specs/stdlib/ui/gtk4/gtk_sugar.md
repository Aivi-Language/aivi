# GTK Sugar

Part of the [Writing Native Apps](../gtk4.md) guide.

The GTK sigil is the main authoring surface:

```aivi
~<gtk>
  <GtkBox spacing="24" marginTop="12">
    <GtkLabel label={title} />
    <GtkButton label="Save" onClick={saveDraft} />
  </GtkBox>
</gtk>
```

## Shorthand widget tags

Tags beginning with `Gtk`, `Adw`, or `Gsk` are sugar for `<object class="...">`.

```aivi
// Preferred shorthand
~<gtk>
  <GtkBox spacing="24" marginTop="12">
    <GtkLabel label={title} />
    <GtkButton label="Save" onClick={saveDraft} />
  </GtkBox>
</gtk>

// Equivalent verbose form
~<gtk>
  <object class="GtkBox" props={{ spacing: 24, marginTop: 12 }}>
    <object class="GtkLabel" props={{ label: title }} />
    <object class="GtkButton" props={{ label: "Save" }} onClick={saveDraft} />
  </object>
</gtk>
```

| Attribute | Meaning |
| --- | --- |
| `label="Save"` | static property |
| `title={windowTitle}` | bound property |
| `id="saveButton"` | widget name for inspection and event matching |
| `ref="saveRef"` | widget reference |
| `onClick={...}` | event binding |
| `onInput={...}` | event binding |
| `onActivate={...}` | activate binding |
| `onToggle={...}` | toggle binding |
| `onSelect={...}` | selection binding |
| `onClosed={...}` | dialog close binding |
| `onKeyPress={...}` | keyboard binding |
| `onValueChanged={...}` | range binding |
| `onFocusIn={...}` | focus-enter binding |
| `onFocusOut={...}` | focus-leave binding |
| `onShowSidebarChanged={...}` | overlay split-view sidebar binding |

## Component tags and function-call tags

Uppercase or dotted non-`Gtk*` / `Adw*` / `Gsk*` tags are component calls. Their attributes lower to a record-shaped argument.

```aivi
<ProjectRow row={row} selected={isSelected} />
<Mail.ProjectRow row={row} selected={isSelected} />
```

Function-call tags are the lighter helper form for simple self-closing tags with positional arguments.

```aivi
// Equivalent to: { navRailNode currentSection "sidebar" }
~<gtk>
  <NavRailNode currentSection "sidebar" />
</gtk>
```

Function-call tags:

- only apply to simple non-widget tags,
- use positional arguments instead of attributes,
- must stay self-closing,
- pass `Unit` automatically when there are no positional arguments, so `<DetailsPane />` lowers like `detailsPane Unit`.

## Object-valued properties and child slots

Use nested `<property name="...">` when GTK expects another object rather than plain text or numbers.

```aivi
~<gtk>
  <GtkListView>
    <property name="model">
      <GtkNoSelection>
        <property name="model">
          <GtkStringList id="items" />
        </property>
      </GtkNoSelection>
    </property>
    <property name="factory">
      <GtkSignalListItemFactory />
    </property>
  </GtkListView>
</gtk>
```

Use `<child type="...">` when the underlying GTK or libadwaita widget has named child slots.

Next: [Callbacks](./callbacks.md)
