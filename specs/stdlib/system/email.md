# Email Module

<!-- quick-info: {"kind":"module","name":"aivi.email"} -->
The `email` module provides IMAP inbox access and SMTP delivery. Use `imap` to fetch messages and `smtpSend` to send them.

<!-- /quick-info -->
<div class="import-badge">use aivi.email</div>

## Types

### `ImapConfig`

Connection and filter configuration for fetching messages via IMAP.

```aivi
ImapConfig = {
  host: Text
  user: Text
  password: Text
  mailbox: Option Text
  filter: Option Text
  limit: Option Int
  port: Option Int
}
```

| Field | Type | Explanation |
| --- | --- | --- |
| `host` | `Text` | IMAP server hostname. |
| `user` | `Text` | IMAP login username. |
| `password` | `Text` | IMAP login password. |
| `mailbox` | `Option Text` | Mailbox to open (e.g. `"INBOX"`). Defaults to `INBOX` when `None`. |
| `filter` | `Option Text` | IMAP search filter string (e.g. `"UNSEEN"`). Fetches all when `None`. |
| `limit` | `Option Int` | Maximum number of messages to return. No limit when `None`. |
| `port` | `Option Int` | IMAP port. Defaults to `993` (IMAPS) when `None`. |

### `SmtpConfig`

Configuration for sending a single email via SMTP.

```aivi
SmtpConfig = {
  host: Text
  user: Text
  password: Text
  from: Text
  to: Text
  subject: Text
  body: Text
}
```

| Field | Type | Explanation |
| --- | --- | --- |
| `host` | `Text` | SMTP server hostname. |
| `user` | `Text` | SMTP login username. |
| `password` | `Text` | SMTP login password. |
| `from` | `Text` | Sender address. |
| `to` | `Text` | Recipient address. |
| `subject` | `Text` | Email subject line. |
| `body` | `Text` | Plain-text email body. |

### `MimePart`

A single decoded MIME part from a raw email message.

```aivi
MimePart = {
  contentType: Text
  body: Text
}
```

## Functions

| Function | Explanation |
| --- | --- |
| **imap** config<br><pre><code>`ImapConfig -> Effect Text (List A)`</code></pre> | Fetches messages from an IMAP mailbox using `config`. Each message is decoded into the expected type `A`. |
| **smtpSend** config<br><pre><code>`SmtpConfig -> Effect Text Unit`</code></pre> | Sends an email using `config`. |
| **mimeParts** raw<br><pre><code>`Text -> Result Text (List MimePart)`</code></pre> | Parses a raw MIME email string into a list of decoded parts. Returns `Err` on malformed input. |
| **flattenBodies** parts<br><pre><code>`List MimePart -> Text`</code></pre> | Concatenates the `body` of each `MimePart` with newlines, producing a single text. |
