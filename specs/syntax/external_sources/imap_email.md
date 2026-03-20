# IMAP (Internet Message Access Protocol) Email Sources

<!-- quick-info: {"kind":"topic","name":"imap email sources"} -->
AIVI provides IMAP integration in two related forms: a typed one-shot source for “connect, fetch, disconnect” mailbox reads, and a longer-lived session API for workflows such as search, flag management, mailbox administration, and IDLE notifications. This page focuses on the IMAP-specific surface; for the convenience module that re-exports the session functions as everyday stdlib calls, see the [Email Module](../../stdlib/system/email.md).
<!-- /quick-info -->

IMAP support comes in two styles:

1. a **one-shot source** when you want to load messages as typed data,
2. a **session API** when you need a longer conversation with the mailbox.

Use the one-shot source for simple inbox ingestion. Use the session API when you need to select mailboxes, manage flags, move or append messages, or listen for changes with IDLE (an IMAP extension for push-style notifications of new mail).

## Start here

- Use the **one-shot source** when you want “load some messages as typed data”.
- Use the **session API** when you need a conversation with the mailbox over time.
- Most session workflows start with `imapOpen`, then `imapSelect` or `imapExamine`, and then `imapSearch` / `imapFetch`.
- All of these operations perform network I/O when loaded or executed.

The one-shot form is exposed directly as `email.imap`. Session operations exist at the lower-level runtime surface as `email.imapOpen`, `email.imapSelect`, and so on, and are re-exported by [`use aivi.email`](../../stdlib/system/email.md) as `imapOpen`, `imapSelect`, `imapFetch`, and related helpers. The examples below use `aivi.email` for the session API because that is the form most application code uses.

## APIs

### One-shot source

- `email.imap : ImapConfig -> Source Imap (List A)`

Load this source with [`load`](../effects.md#load) when you want the runtime to connect, fetch, decode messages as `A`, and disconnect in one step.

### Session lifecycle

- `imapOpen : ImapConfig -> Effect Text ImapSession` (lower-level name: `email.imapOpen`)
- `imapSelect : Text -> ImapSession -> Effect Text MailboxInfo`
- `imapExamine : Text -> ImapSession -> Effect Text MailboxInfo`
- `imapIdle : Int -> ImapSession -> Effect Text IdleResult`

### Search and fetch

- `imapSearch : Text -> ImapSession -> Effect Text (List Int)`
- `imapFetch : List Int -> ImapSession -> Effect Text (List A)`

`imapSearch` takes a raw IMAP search expression such as `"UNSEEN"` or `"UNSEEN FROM \"billing@example.com\""`. `imapFetch` decodes the matching messages with the same typed-message rules used by `email.imap`.

### Flags and message changes

- `imapSetFlags : List Int -> List Text -> ImapSession -> Effect Text Unit`
- `imapAddFlags : List Int -> List Text -> ImapSession -> Effect Text Unit`
- `imapRemoveFlags : List Int -> List Text -> ImapSession -> Effect Text Unit`
- `imapExpunge : ImapSession -> Effect Text Unit`
- `imapCopy : List Int -> Text -> ImapSession -> Effect Text Unit`
- `imapMove : List Int -> Text -> ImapSession -> Effect Text Unit`

Flags are plain IMAP flag strings such as `"\\Seen"` or `"\\Deleted"`.

### Mailbox administration

- `imapListMailboxes : ImapSession -> Effect Text (List MailboxInfo)`
- `imapCreateMailbox : Text -> ImapSession -> Effect Text Unit`
- `imapDeleteMailbox : Text -> ImapSession -> Effect Text Unit`
- `imapRenameMailbox : Text -> Text -> ImapSession -> Effect Text Unit`
- `imapAppend : Text -> Text -> ImapSession -> Effect Text Unit`

`imapAppend` appends a raw RFC822 message string to the destination mailbox.

## Authentication

Both password and OAuth2 (XOAUTH2) authentication are supported through `EmailAuth`:

<<< ../../snippets/from_md/syntax/external_sources/imap_email/block_01.aivi{aivi}


OAuth2 uses the XOAUTH2 (an OAuth2-based authentication mechanism for email servers) SASL mechanism, which is commonly supported by providers such as Gmail and Outlook.

On GNOME desktops, [`aivi.gnome.onlineAccounts`](../../stdlib/system/gnome_online_accounts.md) can supply the `host`, `user`, and `auth` fields for you.
That module resolves desktop-managed credentials, then `Goa.toImapConfig` adds mailbox-specific fields before you call `load (email.imap ...)` or `imapOpen`.

For the full record definitions of `ImapConfig`, `MailboxInfo`, and `IdleResult`, see the [Email Module](../../stdlib/system/email.md). In practice, the most important `ImapConfig` fields are:

- `host`, `user`, and `auth` for the connection itself,
- `mailbox`, `filter`, and `limit` for the one-shot `email.imap` / `imap` helpers,
- `port: None` to use the default implicit-TLS IMAP port `993`,
- `starttls: None` to keep STARTTLS disabled unless you explicitly want the cleartext-then-upgrade flow that is commonly paired with port `143`.

For session-oriented code, `mailbox` and `filter` are usually `None`, because you pick the mailbox with `imapSelect` or `imapExamine` and then provide explicit search text to `imapSearch`.

## Example — one-shot mailbox read

<<< ../../snippets/from_md/syntax/external_sources/imap_email/block_02.aivi{aivi}


This is a good fit for batch-style jobs such as importing unread support messages or extracting invoices from a mailbox. `myToken` stands for an OAuth2 access token that your program acquired elsewhere.

When you decode IMAP messages into a record, the built-in decoder can supply common transport and threading fields such as `uid`, `subject`, `from`, `to`, `cc`, `bcc`, `date`, `messageId`, `inReplyTo`, `references`, `textBody`, `htmlBody`, `body`, and `rawRfc822`. Ask for only the fields your application actually needs.

## Example — GNOME Online Accounts to IMAP

```aivi
use aivi.gnome.onlineAccounts as Goa
use aivi.email

InboxMessage = {
  subject: Option Text
  body: Text
}

loadUnread = accountId =>
  toSource = goaCfg =>
    email.imap (
      Goa.toImapConfig goaCfg (Some "INBOX") (Some "UNSEEN") (Some 20)
    )

  accountId
     |> Goa.imapConfig
     |> toSource
     |> load
```

This keeps the mailbox query in your application while letting GOA supply the desktop-managed transport credentials.

## Example — explicit search and fetch

If you need a little more control than the one-shot source gives you, open the session in a flow, bind it with `#session`, register the matching cleanup at the acquisition line, and then continue with `imapSelect`, `imapSearch`, and `imapFetch`.

## Example — session with IDLE

In a long-running application, keep the session acquisition in an outer flow scope and let the repeating `imapIdle` / fetch step live in a helper flow or anchored restart loop.


Use the session API when you need:

- mailbox lifecycle control,
- explicit searches and fetches,
- flag management,
- append, copy, move, or delete operations,
- push-style workflows through `imapIdle`.

## See also

- [External Sources](../external_sources.md) for the broader `Source K A` model,
- [Effects](../effects.md#load) for how `load` turns a `Source` into an `Effect`,
- [Cleanup & Lifetimes](../resources.md) for the acquisition and cleanup rules behind `imapOpen`,
- [Effects](../effects.md) for effectful runtime operations,
- [Email Module](../../stdlib/system/email.md) for the convenience wrappers and full type definitions.
