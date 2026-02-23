# IMAP Email Sources

<!-- quick-info: {"kind":"topic","name":"imap email sources"} -->
Email integration is available through IMAP as a typed source for mailbox reads.
<!-- /quick-info -->

## APIs (v0.1)

- `email.imap : ImapConfig -> Source Imap (List A)`

## Example

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
    host: "imap.example.com"
    user: "bot@example.com"
    password: "..."
    mailbox: Some "INBOX"
    filter: Some "UNSEEN"
    limit: Some 50
    port: Some 993
  })
  pure msgs
}
```
