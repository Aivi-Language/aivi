# Email Module

<!-- quick-info: {"kind":"module","name":"aivi.email"} -->
The `email` module provides IMAP inbox access and SMTP delivery. Use `imap` for one-shot message fetching, `imapOpen` for session-based IMAP operations (flags, move, delete, IDLE, mailbox management), and `smtpSend` to send messages. Both IMAP and SMTP support password and OAuth2 (XOAUTH2) authentication.

<!-- /quick-info -->
<div class="import-badge">use aivi.email</div>

## Types

### `EmailAuth`

Authentication method for IMAP and SMTP connections.

```aivi
EmailAuth = Password Text | OAuth2 Text
```

| Constructor | Explanation |
| --- | --- |
| `Password Text` | Plain password authentication. |
| `OAuth2 Text` | XOAUTH2 access token (e.g. from Gmail, Outlook). |

### `ImapConfig`

Connection and filter configuration for IMAP.

```aivi
ImapConfig = {
  host: Text
  user: Text
  auth: EmailAuth
  port: Option Int
  starttls: Option Bool
  mailbox: Option Text
  filter: Option Text
  limit: Option Int
}
```

| Field | Type | Explanation |
| --- | --- | --- |
| `host` | `Text` | IMAP server hostname. |
| `user` | `Text` | IMAP login username / email. |
| `auth` | `EmailAuth` | Authentication method (`Password` or `OAuth2`). |
| `port` | `Option Int` | IMAP port. Defaults to `993` (IMAPS) when `None`. |
| `starttls` | `Option Bool` | Use STARTTLS on port 143 instead of implicit TLS. Defaults to `False` when `None`. |
| `mailbox` | `Option Text` | Mailbox to open (e.g. `"INBOX"`). Defaults to `INBOX` when `None`. Used by the simple `imap` function. |
| `filter` | `Option Text` | IMAP search filter string (e.g. `"UNSEEN"`). Fetches all when `None`. Used by the simple `imap` function. |
| `limit` | `Option Int` | Maximum number of messages to return. No limit when `None`. Used by the simple `imap` function. |

### `SmtpConfig`

Configuration for sending a single email via SMTP.

```aivi
SmtpConfig = {
  host: Text
  user: Text
  auth: EmailAuth
  from: Text
  to: List Text
  cc: Option (List Text)
  bcc: Option (List Text)
  subject: Text
  body: Text
  port: Option Int
  starttls: Option Bool
}
```

| Field | Type | Explanation |
| --- | --- | --- |
| `host` | `Text` | SMTP server hostname. |
| `user` | `Text` | SMTP login username. |
| `auth` | `EmailAuth` | Authentication method (`Password` or `OAuth2`). |
| `from` | `Text` | Sender address. |
| `to` | `List Text` | Recipient addresses. |
| `cc` | `Option (List Text)` | CC recipient addresses. |
| `bcc` | `Option (List Text)` | BCC recipient addresses. |
| `subject` | `Text` | Email subject line. |
| `body` | `Text` | Plain-text email body. |
| `port` | `Option Int` | SMTP port. Defaults to `465` (SMTPS) when `None`. |
| `starttls` | `Option Bool` | Use STARTTLS (typically port 587) instead of implicit TLS. Defaults to `False` when `None`. |

### `MimePart`

A single decoded MIME part from a raw email message.

```aivi
MimePart = {
  contentType: Text
  body: Text
}
```

### `MailboxInfo`

Information about an IMAP mailbox, returned by `imapListMailboxes`.

```aivi
MailboxInfo = {
  name: Text
  separator: Option Text
  attributes: List Text
}
```

### `IdleResult`

Outcome of an `imapIdle` call.

```aivi
IdleResult = TimedOut | MailboxChanged
```

## Functions

### One-shot (convenience)

| Function | Explanation |
| --- | --- |
| **imap** config<br><code>ImapConfig -> Effect Text (List A)</code> | Connects, fetches messages matching `config.filter` from `config.mailbox`, and disconnects. Each message is decoded into the expected type `A`. |
| **smtpSend** config<br><code>SmtpConfig -> Effect Text Unit</code> | Sends an email using `config`. Supports multiple recipients, CC, BCC. |
| **mimeParts** raw<br><code>Text -> List MimePart</code> | Parses a raw MIME email string into a list of decoded parts. |
| **flattenBodies** parts<br><code>List MimePart -> Text</code> | Concatenates the `body` of each `MimePart` with newlines, producing a single text. |

### Session-based IMAP

Open a persistent IMAP connection as a `Resource` for multi-step mailbox interactions.

| Function | Explanation |
| --- | --- |
| **imapOpen** config<br><code>ImapConfig -> Resource Text ImapSession</code> | Opens a persistent IMAP connection. The session is automatically closed when the resource scope exits. |
| **imapSelect** mailbox session<br><code>Text -> ImapSession -> Effect Text MailboxInfo</code> | Selects a mailbox in read-write mode. |
| **imapExamine** mailbox session<br><code>Text -> ImapSession -> Effect Text MailboxInfo</code> | Opens a mailbox in read-only mode. |
| **imapSearch** query session<br><code>Text -> ImapSession -> Effect Text (List Int)</code> | Searches the selected mailbox. Returns a list of message UIDs matching the IMAP search query. |
| **imapFetch** uids session<br><code>List Int -> ImapSession -> Effect Text (List A)</code> | Fetches messages by UID. Each message is decoded into the expected type `A`. |
| **imapSetFlags** uids flags session<br><code>List Int -> List Text -> ImapSession -> Effect Text Unit</code> | Replaces all flags on messages (e.g. `["\\Seen", "\\Flagged"]`). |
| **imapAddFlags** uids flags session<br><code>List Int -> List Text -> ImapSession -> Effect Text Unit</code> | Adds flags to messages without removing existing ones. |
| **imapRemoveFlags** uids flags session<br><code>List Int -> List Text -> ImapSession -> Effect Text Unit</code> | Removes specific flags from messages. |
| **imapExpunge** session<br><code>ImapSession -> Effect Text Unit</code> | Permanently removes all messages marked `\\Deleted` from the selected mailbox. |
| **imapCopy** uids mailbox session<br><code>List Int -> Text -> ImapSession -> Effect Text Unit</code> | Copies messages by UID to another mailbox. |
| **imapMove** uids mailbox session<br><code>List Int -> Text -> ImapSession -> Effect Text Unit</code> | Moves messages by UID to another mailbox. |
| **imapListMailboxes** session<br><code>ImapSession -> Effect Text (List MailboxInfo)</code> | Lists all mailboxes on the server. |
| **imapCreateMailbox** name session<br><code>Text -> ImapSession -> Effect Text Unit</code> | Creates a new mailbox. |
| **imapDeleteMailbox** name session<br><code>Text -> ImapSession -> Effect Text Unit</code> | Deletes a mailbox. |
| **imapRenameMailbox** from to session<br><code>Text -> Text -> ImapSession -> Effect Text Unit</code> | Renames a mailbox. |
| **imapAppend** mailbox content session<br><code>Text -> Text -> ImapSession -> Effect Text Unit</code> | Appends a raw RFC822 message to a mailbox. |
| **imapIdle** timeoutSecs session<br><code>Int -> ImapSession -> Effect Text IdleResult</code> | Waits for mailbox changes using IMAP IDLE. Returns `MailboxChanged` if activity is detected, or `TimedOut` after `timeoutSecs` seconds. |

## Examples

### One-shot fetch with OAuth2

```aivi
use aivi.email

do Effect {
  msgs <- imap {
    host: "imap.gmail.com"
    user: "user@gmail.com"
    auth: OAuth2 myAccessToken
    mailbox: Some "INBOX"
    filter: Some "UNSEEN"
    limit: Some 20
    port: None
    starttls: None
  }
  pure msgs
}
```

### Session-based workflow

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
    uids <- imapSearch "UNSEEN" session
    msgs <- imapFetch uids session
    _ <- imapAddFlags uids ["\\Seen"] session
    pure msgs
  })
```

### IDLE notification loop

```aivi
use aivi.email

watchInbox = session => do Effect {
  _ <- imapSelect "INBOX" session
  result <- imapIdle 300 session
  result match
    | MailboxChanged => do Effect {
        uids <- imapSearch "UNSEEN" session
        msgs <- imapFetch uids session
        _ <- processMsgs msgs
        watchInbox session
      }
    | TimedOut => watchInbox session
}
```
