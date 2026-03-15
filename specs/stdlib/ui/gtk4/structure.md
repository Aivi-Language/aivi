# Structure

Part of the [Writing Native Apps](../gtk4.md) guide.

Dynamic child structure uses mounted structural bindings rather than plain rerendering.

```aivi
use aivi.reactive
use aivi.ui.gtk4

rows = signal [
  { id: "1", title: "Inbox", visible: True },
  { id: "2", title: "Archive", visible: False }
]
sidebarOpen = signal True
visibleRows = rows ->> filter .visible

mailboxRow = row => ~<gtk>
  <GtkLabel label={row.title} />
</gtk>

view = ~<gtk>
  <GtkBox orientation="vertical" spacing="6">
    <show when={sidebarOpen}>
      <GtkLabel label="Sidebar" />
    </show>
    <each items={visibleRows} as={row} key={row => row.id}>
      <MailboxRow row />
    </each>
  </GtkBox>
</gtk>
```

## Public contract

- `<show>` mounts or disposes one child scope as its guard changes,
- `<each>` preserves one mounted child scope per key,
- keyed children move instead of being recreated when possible,
- inserts and removals go through the owning GTK container.

Next: [Lifecycle](./lifecycle.md)
