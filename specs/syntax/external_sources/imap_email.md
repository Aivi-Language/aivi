# IMAP Email Sources

<!-- quick-info: {"kind":"topic","name":"imap email sources"} -->
AIVI provides email integration through IMAP, with a typed one-shot source for mailbox reads and a session API for longer-lived mailbox work such as search, flag management, and IDLE notifications.
<!-- /quick-info -->

IMAP support comes in two styles:

1. a **one-shot source** when you want to load messages as typed data,
2. a **session API** when you need a longer conversation with the mailbox.

Use the one-shot source for simple inbox ingestion. Use the session API when you need to select mailboxes, manage flags, move messages, or listen for changes with IDLE.

## Start here

- Use the **one-shot source** when you want “load some messages as typed data”.
- Use the **session API** when you need a conversation with the mailbox over time.
- Most session workflows start with `imapOpen`, `imapSelect`, then `imapSearch` / `imapFetch`.

## APIs

### One-shot source

- `email.imap : ImapConfig -> Source Imap (List A)`

### Session lifecycle

- `email.imapOpen : ImapConfig -> Resource Text ImapSession`
- `email.imapSelect : Text -> ImapSession -> Effect Text MailboxInfo`
- `email.imapExamine : Text -> ImapSession -> Effect Text MailboxInfo`
- `email.imapIdle : Int -> ImapSession -> Effect Text IdleResult`

### Search and fetch

- `email.imapSearch : Text -> ImapSession -> Effect Text (List Int)`
- `email.imapFetch : List Int -> ImapSession -> Effect Text (List A)`

### Flags and message changes

- `email.imapSetFlags : List Int -> List Text -> ImapSession -> Effect Text Unit`
- `email.imapAddFlags : List Int -> List Text -> ImapSession -> Effect Text Unit`
- `email.imapRemoveFlags : List Int -> List Text -> ImapSession -> Effect Text Unit`
- `email.imapExpunge : ImapSession -> Effect Text Unit`
- `email.imapCopy : List Int -> Text -> ImapSession -> Effect Text Unit`
- `email.imapMove : List Int -> Text -> ImapSession -> Effect Text Unit`

### Mailbox administration

- `email.imapListMailboxes : ImapSession -> Effect Text (List MailboxInfo)`
- `email.imapCreateMailbox : Text -> ImapSession -> Effect Text Unit`
- `email.imapDeleteMailbox : Text -> ImapSession -> Effect Text Unit`
- `email.imapRenameMailbox : Text -> Text -> ImapSession -> Effect Text Unit`
- `email.imapAppend : Text -> Text -> ImapSession -> Effect Text Unit`

## Authentication

Both password and OAuth2 (XOAUTH2) authentication are supported through `EmailAuth`:

```aivi
EmailAuth = Password Text | OAuth2 Text
```

OAuth2 uses the XOAUTH2 SASL mechanism, which is commonly supported by providers such as Gmail and Outlook.

## Example — one-shot mailbox read

```aivi
InboxMessage = {
  uid: Option Int
  subject: Option Text
  from: Option Text
  to: Option Text
  date: Option Text
  body: Text
}

do Effect {
  msgs <- load (email.imap {
    host: "imap.gmail.com"
    user: "user@gmail.com"
    auth: OAuth2 myToken
    mailbox: Some "INBOX"   -- read from the inbox
    filter: Some "UNSEEN"   -- only unread messages
    limit: Some 50          -- cap the batch size
    port: None
    starttls: None
  })
  pure msgs
}
```

This is a good fit for batch-style jobs such as importing unread support messages or extracting invoices from a mailbox.

## Example — explicit search and fetch

If you need a little more control than the one-shot source gives you, this is the smallest useful session flow:

```aivi
do Effect {
  session <- email.imapOpen config
  _ <- email.imapSelect "INBOX" session
  messageIds <- email.imapSearch "UNSEEN" session
  messages <- email.imapFetch messageIds session
  pure messages
}
```

This pattern is useful when you want custom search strings or you want to decide yourself where the mailbox session scope begins and ends.

## Example — session with IDLE

```aivi
use aivi.email

processInbox : Text -> Effect Text (List InboxMessage)
processInbox = token => do Effect {
  session <- imapOpen {
    host: "imap.gmail.com"
    user: "user@gmail.com"
    auth: OAuth2 token
    port: None
    starttls: None
    mailbox: None
    filter: None
    limit: None
  }
  _ <- imapSelect "INBOX" session
  result <- imapIdle 300 session         -- wait for mailbox changes for up to 300 seconds
  messages <- result match
    | MailboxChanged => do Effect {
        uids <- imapSearch "UNSEEN" session
        msgs <- imapFetch uids session
        _ <- imapAddFlags uids ["\\Seen"] session
        pure msgs
      }
    | TimedOut => pure []
  pure messages
}
```

Because `imapOpen` is acquired with `<-`, the session is released automatically when the surrounding `do Effect` block exits.

Use the session API when you need:

- mailbox lifecycle control,
- explicit searches and fetches,
- flag management,
- append, copy, move, or delete operations,
- push-style workflows through `imapIdle`.
