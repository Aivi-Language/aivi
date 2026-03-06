# IMAP Email Sources

<!-- quick-info: {"kind":"topic","name":"imap email sources"} -->
Email integration is available through IMAP as a typed source for one-shot mailbox reads, and as a session-based resource for full IMAP interaction including OAuth2, message management, and IDLE push notifications.
<!-- /quick-info -->

## APIs (v0.1)

### One-shot source

- `email.imap : ImapConfig -> Source Imap (List A)`

### Session builtins

- `email.imapOpen : ImapConfig -> Effect Text ImapSessionHandle`
- `email.imapClose : ImapSessionHandle -> Effect Text Unit`
- `email.imapSelect : Text -> ImapSessionHandle -> Effect Text MailboxInfoRecord`
- `email.imapExamine : Text -> ImapSessionHandle -> Effect Text MailboxInfoRecord`
- `email.imapSearch : Text -> ImapSessionHandle -> Effect Text (List Int)`
- `email.imapFetch : List Int -> ImapSessionHandle -> Effect Text (List A)`
- `email.imapSetFlags : List Int -> List Text -> ImapSessionHandle -> Effect Text Unit`
- `email.imapAddFlags : List Int -> List Text -> ImapSessionHandle -> Effect Text Unit`
- `email.imapRemoveFlags : List Int -> List Text -> ImapSessionHandle -> Effect Text Unit`
- `email.imapExpunge : ImapSessionHandle -> Effect Text Unit`
- `email.imapCopy : List Int -> Text -> ImapSessionHandle -> Effect Text Unit`
- `email.imapMove : List Int -> Text -> ImapSessionHandle -> Effect Text Unit`
- `email.imapListMailboxes : ImapSessionHandle -> Effect Text (List MailboxInfoRecord)`
- `email.imapCreateMailbox : Text -> ImapSessionHandle -> Effect Text Unit`
- `email.imapDeleteMailbox : Text -> ImapSessionHandle -> Effect Text Unit`
- `email.imapRenameMailbox : Text -> Text -> ImapSessionHandle -> Effect Text Unit`
- `email.imapAppend : Text -> Text -> ImapSessionHandle -> Effect Text Unit`
- `email.imapIdle : Int -> ImapSessionHandle -> Effect Text IdleResultValue`

## Authentication

Both password and OAuth2 (XOAUTH2) authentication are supported via the `EmailAuth` type:

```aivi
EmailAuth = Password Text | OAuth2 Text
```

OAuth2 uses the XOAUTH2 SASL mechanism, compatible with Gmail, Outlook, and other providers that support it.

## Example — One-shot

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
    mailbox: Some "INBOX"
    filter: Some "UNSEEN"
    limit: Some 50
    port: None
    starttls: None
  })
  pure msgs
}
```

## Example — Session with IDLE

```aivi
use aivi.email

processInbox = token => resource {
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
  yield session
}
  |> withResource (session => do Effect {
    _ <- imapSelect "INBOX" session
    result <- imapIdle 300 session
    result match
      | MailboxChanged => do Effect {
          uids <- imapSearch "UNSEEN" session
          msgs <- imapFetch uids session
          _ <- imapAddFlags uids ["\\Seen"] session
          pure msgs
        }
      | TimedOut => pure []
  })
```
