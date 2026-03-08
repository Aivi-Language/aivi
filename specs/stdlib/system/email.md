# Email Module

<!-- quick-info: {"kind":"module","name":"aivi.email"} -->
The `email` module lets AIVI programs read mail from IMAP servers and send mail through SMTP.
Use `imap` for simple one-shot reads, `imapOpen` when you need a longer-lived IMAP session, and `smtpSend` when your program needs to deliver mail.
This module is the convenient stdlib surface over the lower-level `email.*` runtime APIs.

<!-- /quick-info -->
<div class="import-badge">use aivi.email</div>

## What this module is for

`aivi.email` is useful when a program needs to:

- fetch incoming messages from an inbox,
- search and manage mailboxes,
- watch for changes with IMAP IDLE,
- or send plain-text email notifications.

All of these APIs perform network I/O when executed.
If you need the lower-level `load (email.imap ...)` source form, see [IMAP Email Sources](../../syntax/external_sources/imap_email.md).

Both IMAP and SMTP accept password credentials or OAuth2 access tokens through `EmailAuth`.
IMAP uses XOAUTH2 (an OAuth2-based authentication mechanism specifically for email servers) directly; SMTP token support depends on what the remote server accepts for authenticated SMTP.

## Types

### `EmailAuth`

Authentication method for IMAP and SMTP connections.

<<< ../../snippets/from_md/stdlib/system/email/block_01.aivi{aivi}


| Constructor | What it means |
| --- | --- |
| `Password Text` | Password-based authentication. |
| `OAuth2 Text` | OAuth2 access token. IMAP uses XOAUTH2 directly; SMTP support depends on the provider/server. |

### `ImapConfig`

Connection and filtering settings for IMAP.

<<< ../../snippets/from_md/stdlib/system/email/block_02.aivi{aivi}


| Field | Type | What it controls |
| --- | --- | --- |
| `host` | `Text` | IMAP server hostname. |
| `user` | `Text` | Login username, often the email address. |
| `auth` | `EmailAuth` | Password or OAuth2 token. |
| `port` | `Option Int` | IMAP port. When `None`, the default is `993` for implicit TLS. |
| `starttls` | `Option Bool` | Whether to use STARTTLS, commonly on port `143`. When `None`, the default is `False`. |
| `mailbox` | `Option Text` | Mailbox name such as `"INBOX"`. Used by the simple `imap` helper. |
| `filter` | `Option Text` | IMAP search expression such as `"UNSEEN"`. Used by the simple `imap` helper. |
| `limit` | `Option Int` | Maximum number of messages to return. |

### `SmtpConfig`

Configuration for sending one plain-text email.
Use this when you want the runtime to build and send a straightforward plain-text message for you.

<<< ../../snippets/from_md/stdlib/system/email/block_03.aivi{aivi}


| Field | Type | What it controls |
| --- | --- | --- |
| `host` | `Text` | SMTP server hostname. |
| `user` | `Text` | Login username. |
| `auth` | `EmailAuth` | Password or OAuth2 token. |
| `from` | `Text` | Sender address. |
| `to` | `List Text` | Primary recipients. |
| `cc` | `Option (List Text)` | Carbon-copy recipients. |
| `bcc` | `Option (List Text)` | Blind-carbon-copy recipients. |
| `subject` | `Text` | Subject line. |
| `body` | `Text` | Plain-text message body. |
| `port` | `Option Int` | SMTP port. When `None`, the default is `465` for implicit TLS. |
| `starttls` | `Option Bool` | Whether to use STARTTLS instead of implicit TLS. |

### `MimePart`

A decoded MIME part extracted from a raw email message.

<<< ../../snippets/from_md/stdlib/system/email/block_02.aivi{aivi}


### `MailboxInfo`

Information about an IMAP mailbox.

<<< ../../snippets/from_md/stdlib/system/email/block_05.aivi{aivi}


### `IdleResult`

Result of waiting with IMAP IDLE.

<<< ../../snippets/from_md/stdlib/system/email/block_03.aivi{aivi}


## Choose the right entry point

- Use **`imap`** when you want a simple “connect, fetch, disconnect” workflow.
- Use **`imapOpen`** when you need to search, flag, move, append, or watch messages over a longer session.
- Use **`smtpSend`** when you want to send a plain-text message.
- Use **`mimeParts`** and **`flattenBodies`** when you already have raw mail data and want to inspect or display it.

`imap` is a convenience wrapper around `load (email.imap config)`.
If you want the underlying source-oriented API or more detail about typed mailbox reads, see [IMAP Email Sources](../../syntax/external_sources/imap_email.md).

## Functions

### One-shot helpers

| Function | What it does |
| --- | --- |
| **imap** config<br><code>ImapConfig -> Effect Text (List A)</code> | Connects, fetches messages that match `config.filter` from `config.mailbox`, decodes them with the same rules as `load (email.imap config)`, and disconnects. |
| **smtpSend** config<br><code>SmtpConfig -> Effect Text Unit</code> | Sends one email using the supplied SMTP settings. |
| **mimeParts** raw<br><code>Text -> List MimePart</code> | Parses a raw MIME message into decoded parts. |
| **flattenBodies** parts<br><code>List MimePart -> Text</code> | Joins the body text of multiple MIME parts into one plain text value. |

### Session-based IMAP

Use `imapOpen` when you need several mailbox actions in one connection.
As a `Resource`, it automatically closes the session when the surrounding resource scope ends.

| Function | What it does |
| --- | --- |
| **imapOpen** config<br><code>ImapConfig -> Resource Text ImapSession</code> | Opens a persistent IMAP session. |
| **imapSelect** mailbox session<br><code>Text -> ImapSession -> Effect Text MailboxInfo</code> | Opens a mailbox in read-write mode. |
| **imapExamine** mailbox session<br><code>Text -> ImapSession -> Effect Text MailboxInfo</code> | Opens a mailbox in read-only mode. |
| **imapSearch** query session<br><code>Text -> ImapSession -> Effect Text (List Int)</code> | Searches the selected mailbox and returns matching UIDs. |
| **imapFetch** uids session<br><code>List Int -> ImapSession -> Effect Text (List A)</code> | Fetches the messages for the given UIDs and decodes them as `A`. |
| **imapSetFlags** uids flags session<br><code>List Int -> List Text -> ImapSession -> Effect Text Unit</code> | Replaces the flags on the given messages. |
| **imapAddFlags** uids flags session<br><code>List Int -> List Text -> ImapSession -> Effect Text Unit</code> | Adds flags such as `"\\Seen"` without removing the existing flags. |
| **imapRemoveFlags** uids flags session<br><code>List Int -> List Text -> ImapSession -> Effect Text Unit</code> | Removes only the named flags. |
| **imapExpunge** session<br><code>ImapSession -> Effect Text Unit</code> | Permanently removes messages marked `"\\Deleted"` from the selected mailbox. |
| **imapCopy** uids mailbox session<br><code>List Int -> Text -> ImapSession -> Effect Text Unit</code> | Copies messages to another mailbox. |
| **imapMove** uids mailbox session<br><code>List Int -> Text -> ImapSession -> Effect Text Unit</code> | Moves messages to another mailbox. |
| **imapListMailboxes** session<br><code>ImapSession -> Effect Text (List MailboxInfo)</code> | Lists available mailboxes on the server. |
| **imapCreateMailbox** name session<br><code>Text -> ImapSession -> Effect Text Unit</code> | Creates a mailbox. |
| **imapDeleteMailbox** name session<br><code>Text -> ImapSession -> Effect Text Unit</code> | Deletes a mailbox. |
| **imapRenameMailbox** from to session<br><code>Text -> Text -> ImapSession -> Effect Text Unit</code> | Renames a mailbox. |
| **imapAppend** mailbox content session<br><code>Text -> Text -> ImapSession -> Effect Text Unit</code> | Appends a raw RFC822 message to a mailbox. |
| **imapIdle** timeoutSecs session<br><code>Int -> ImapSession -> Effect Text IdleResult</code> | Waits for mailbox changes, returning `MailboxChanged` or `TimedOut`. |

## Examples

These examples use Gmail-style hostnames because they are familiar to many readers.
Replace the host, username, mailbox names, and token/password values with the settings from your own mail provider and deployment environment.

### One-shot fetch with OAuth2

<<< ../../snippets/from_md/stdlib/system/email/block_07.aivi{aivi}

This is the smallest useful flow when you just want “read a bounded batch of messages and return them”.
`myAccessToken` stands for an OAuth2 access token that your program acquired elsewhere.

### Session-based workflow

<<< ../../snippets/from_md/stdlib/system/email/block_08.aivi{aivi}

This keeps one IMAP connection open long enough to select a mailbox, search it, fetch matching messages, and then mark those messages as seen.
Use this pattern when several mailbox operations belong in one resource scope.

### Watching a mailbox with IMAP IDLE

<<< ../../snippets/from_md/stdlib/system/email/block_09.aivi{aivi}

This example assumes `processMsgs` is your own application function, for example `List InboxMessage -> Effect Text Unit`.
Wrap `watchInbox` in a resource scope that acquires the session with `imapOpen`, then let the recursive loop continue until your application decides to stop.
