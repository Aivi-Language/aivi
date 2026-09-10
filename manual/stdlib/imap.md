# aivi.imap

Types for mailbox syncing, folder summaries, and mail events.

IMAP is the protocol many mail servers use for listing folders and syncing messages. This module defines the shapes an IMAP-backed feature can return. The current stdlib file does not connect to a server on its own.

Current status: this module now names the records used by the built-in IMAP source providers
(`imap.connect`, `imap.idle`, and `imap.fetchBody`) while keeping the task alias for background
IMAP work.

## Import

```aivi
use aivi.imap (
    ImapError
    ImapAuthFailed
    ImapConnectionFailed
    FolderNotFound
    ImapProtocolError
    SyncState
    ImapFolder
    ImapFlag
    Seen
    Answered
    Flagged
    Draft
    ImapEvent
    NewMessage
    MessageFlagChanged
    FolderChanged
    SyncCompleted
    ImapHeader
    ImapSnapshot
    ImapLiveEvent
    ImapBody
    ImapTask
)
```

## Overview

| Type | Purpose |
|------|---------|
| `SyncState` | Current progress and last error for a sync run |
| `ImapFolder` | Folder name plus message counts |
| `ImapFlag` | Common message flags |
| `ImapEvent` | Events emitted while mail changes |
| `ImapHeader` | One synced message header |
| `ImapSnapshot` | One mailbox snapshot for one account |
| `ImapLiveEvent` | One account-scoped mailbox change event |
| `ImapBody` | Full fetched message body |
| `ImapError` | Structured connection or sync failures |
| `ImapTask A` | Background IMAP work returning `A` |

---

## `SyncState`

```aivi
use aivi.imap (ImapError)

type SyncState = {
    lastSyncedAt: Option Int,
    inProgress: Bool,
    error: Option ImapError
}
```

Tracks the current state of a mailbox sync.

- `lastSyncedAt` — optional time of the last completed sync
- `inProgress` — `True` while a sync is running
- `error` — the last sync error, if there was one

```aivi
use aivi.imap (SyncState)

value idleSync : SyncState = {
    lastSyncedAt: None,
    inProgress: False,
    error: None
}
```

---

## `ImapFolder`

```aivi
type ImapFolder = {
    name: Text,
    messageCount: Int,
    unreadCount: Int
}
```

Summary information for one folder.

```aivi
use aivi.imap (ImapFolder)

value inbox : ImapFolder = {
    name: "INBOX",
    messageCount: 120,
    unreadCount: 4
}
```

---

## `ImapFlag`

```aivi
type ImapFlag =
  | Seen
  | Answered
  | Flagged
  | Draft
```

Common message flags used by mail servers.

- `Seen` — the message has been read
- `Answered` — a reply was sent
- `Flagged` — the message is starred or flagged
- `Draft` — the message is a draft

---

## `ImapEvent`

```aivi
use aivi.imap (ImapFlag)

type ImapEvent =
  | NewMessage Int
  | MessageFlagChanged Int ImapFlag
  | FolderChanged Text
  | SyncCompleted
```

Mailbox events you can react to.

- `NewMessage Int` — a new message arrived, identified by an integer from the backend
- `MessageFlagChanged Int ImapFlag` — a message flag changed
- `FolderChanged Text` — a folder changed by name
- `SyncCompleted` — the current sync run finished

```aivi
use aivi.imap (
    ImapEvent
    NewMessage
    MessageFlagChanged
    FolderChanged
    SyncCompleted
)

type ImapEvent -> Text
func describeEvent = event => event
 ||> NewMessage _           -> "new message"
 ||> MessageFlagChanged _ _ -> "message flag changed"
 ||> FolderChanged name     -> "folder changed: {name}"
 ||> SyncCompleted          -> "sync completed"
```

---

## `ImapHeader`

```aivi
use aivi.imap (ImapFlag)

type ImapHeader = {
    uid: Int,
    subject: Text,
    from: Text,
    date: Text,
    messageId: Text,
    flags: List ImapFlag,
    preview: Text,
    rawHeader: Text
}
```

Best-effort header slice fetched during mailbox sync.

## `ImapSnapshot`

```aivi
use aivi.imap (ImapHeader)

type ImapSnapshot = {
    accountId: Text,
    mailbox: Text,
    highestUid: Option Int,
    messages: List ImapHeader
}
```

Mailbox snapshot returned by `imap.connect`.

## `ImapLiveEvent`

```aivi
use aivi.imap (ImapEvent)

type ImapLiveEvent = {
    accountId: Text,
    mailbox: Text,
    event: ImapEvent
}
```

Account-scoped mailbox change event returned by `imap.idle`.

## `ImapBody`

```aivi
type ImapBody = {
    accountId: Text,
    mailbox: Text,
    uid: Int,
    text: Text,
    html: Text,
    raw: Text
}
```

Fetched body payload returned by `imap.fetchBody`.

---

## `ImapError`

```aivi
type ImapError =
  | ImapAuthFailed
  | ImapConnectionFailed Text
  | FolderNotFound Text
  | ImapProtocolError Text
```

Structured failure reasons for IMAP work.

- `ImapAuthFailed` — login failed
- `ImapConnectionFailed Text` — the server could not be reached or the connection dropped
- `FolderNotFound Text` — a requested folder does not exist
- `ImapProtocolError Text` — another protocol-level failure occurred

```aivi
use aivi.imap (
    ImapError
    ImapAuthFailed
    ImapConnectionFailed
    FolderNotFound
    ImapProtocolError
)

type ImapError -> Text
func describeImapError = error => error
 ||> ImapAuthFailed           -> "authentication failed"
 ||> ImapConnectionFailed msg -> "connection failed: {msg}"
 ||> FolderNotFound name      -> "folder not found: {name}"
 ||> ImapProtocolError msg    -> "IMAP protocol error: {msg}"
```

---

## `ImapTask`

```aivi
use aivi.imap (ImapError)

type ImapTask A =
  Task ImapError A
```

Alias for background IMAP work that either returns `A` or fails with `ImapError`.

## Source providers

Use actual `GoaMailAccount` values from `goa.mailAccounts` in an application. The empty
list below only illustrates the declarations; it does not connect to a mail account.
Direct TLS is supported, but STARTTLS-only GOA accounts are currently rejected.

```aivi
use aivi.imap (
    ImapBody
    ImapError
    ImapLiveEvent
    ImapSnapshot
)

use aivi.gnome.onlineAccounts (GoaMailAccount)

value accounts : List GoaMailAccount = []

signal syncPulse : Signal Unit

@source imap.connect accounts with {
    mailbox: "INBOX",
    limit: 25,
    refreshOn: syncPulse
}
signal snapshots : Signal (Result ImapError (List ImapSnapshot))

@source imap.idle accounts with {
    mailbox: "INBOX"
}
signal liveEvent : Signal (Result ImapError ImapLiveEvent)
```

`imap.fetchBody` requires a record with the account's connection and authentication
fields plus `mailbox: Text` and `uid: Int`. An `accountId` alone is not sufficient.
See the [source catalog](/guide/source-catalog#imap-fetchbody) for the request contract.

## Additional error and event constructors

`ImapConnectionFailed message`, `FolderNotFound folder`, and `ImapProtocolError message` preserve IMAP
failure details. `NewMessage uid`, `MessageFlagChanged uid flag`, and `FolderChanged folder` describe
incremental mailbox events.
