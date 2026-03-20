# GNOME Online Accounts Module

<!-- quick-info: {"kind":"module","name":"aivi.gnome.onlineAccounts"} -->
`aivi.gnome.onlineAccounts` lets AIVI programs discover GNOME Online Accounts mail accounts and turn desktop-managed credentials into typed mail configuration values that compose with `aivi.email`.
<!-- /quick-info -->

<div class="import-badge">use aivi.gnome.onlineAccounts</div>

## What this module is for

Use this module when your application runs in a GNOME desktop session and you want to reuse the accounts that the user already configured in **Settings → Online Accounts**.

The module does **not** implement IMAP or SMTP itself.
Instead, it discovers accounts, refreshes credentials through GOA, and resolves partial config records that you can pass into the existing [`aivi.email`](./email.md) helpers.

Typical uses include:

- showing the user a list of desktop-managed mail accounts,
- reusing a GOA-managed OAuth2 token or password for IMAP access,
- composing SMTP transport settings without storing a second copy of the credentials in your own application config.

These APIs are platform integrations.
On unsupported platforms, or when the GOA service is unavailable, they fail with `GoaError`.

## Types

### `GoaError`

Typed failures produced by the GOA integration layer.

```aivi
GoaError =
  | PlatformUnsupported
  | ServiceUnavailable Text
  | AttentionNeeded Text
  | AccountNotFound Text
  | MailUnsupported Text
  | UnsupportedAuth Text
  | Credentials Text
```

| Constructor | What it means |
| --- | --- |
| `PlatformUnsupported` | The program is running somewhere that does not expose GNOME Online Accounts. |
| `ServiceUnavailable Text` | The session bus or GOA service could not be reached. |
| `AttentionNeeded Text` | GOA says the user must reauthenticate or fix the account in the desktop UI first. |
| `AccountNotFound Text` | No GOA account with the requested account id exists. |
| `MailUnsupported Text` | The selected GOA account does not expose the requested mail capability. |
| `UnsupportedAuth Text` | The GOA account exposes a credential style that the current AIVI mail surface cannot use safely. |
| `Credentials Text` | GOA could not refresh or return usable credentials. |

### `GoaMailAccount`

Summary information for one GOA account that currently exposes the mail interface.

```aivi
GoaMailAccount = {
  id: Text
  providerType: Text
  providerName: Text
  presentationIdentity: Text
  emailAddress: Option Text
  attentionNeeded: Bool
  imapSupported: Bool
  smtpSupported: Bool
}
```

### `GoaImapConfig`

IMAP connection settings resolved from GOA.
This is the transport/auth part only; your application still chooses mailbox, filter, and message limit.

```aivi
GoaImapConfig = {
  host: Text
  user: Text
  auth: EmailAuth
  port: Option Int
  starttls: Option Bool
}
```

### `GoaSmtpConfig`

SMTP connection settings resolved from GOA.
This includes the sender identity, but your application still supplies recipients and message content.

```aivi
GoaSmtpConfig = {
  host: Text
  user: Text
  auth: EmailAuth
  from: Text
  port: Option Int
  starttls: Option Bool
}
```

## Core API

| Function | What it does |
| --- | --- |
| **listMailAccounts**<br><code>Effect GoaError (List GoaMailAccount)</code> | Lists GOA accounts that currently expose the mail interface. |
| **ensureCredentials** accountId<br><code>Text -> Effect GoaError Unit</code> | Asks GOA to refresh or validate credentials for one account id. |
| **imapConfig** accountId<br><code>Text -> Effect GoaError GoaImapConfig</code> | Resolves IMAP transport/auth settings for the selected account. |
| **smtpConfig** accountId<br><code>Text -> Effect GoaError GoaSmtpConfig</code> | Resolves SMTP transport/auth settings for the selected account. |
| **toImapConfig** cfg mailbox filter limit<br><code>GoaImapConfig -> Option Text -> Option Text -> Option Int -> ImapConfig</code> | Adds mailbox/filter/limit fields so the result can be used with `aivi.email.imap` or `imapOpen`. |
| **toSmtpConfig** cfg to cc bcc subject body<br><code>GoaSmtpConfig -> List Text -> Option (List Text) -> Option (List Text) -> Text -> Text -> SmtpConfig</code> | Adds recipients and message body fields so the result can be used with `aivi.email.smtpSend`. |

## Example

This example lists GOA mail accounts, picks one by id, then uses the resolved IMAP settings with the existing `aivi.email` surface:

```aivi
use aivi.email
use aivi.gnome.onlineAccounts as Goa

InboxMessage = {
  subject: Option Text
  body: Text
}

loadUnread = accountId =>
  toSource = accountCfg =>
    imap (
      Goa.toImapConfig accountCfg (Some "INBOX") (Some "UNSEEN") (Some 20)
    )

  accountId
     |> Goa.imapConfig #accountCfg
     |> toSource
```

And this is the corresponding SMTP composition pattern:

```aivi
use aivi.email
use aivi.gnome.onlineAccounts as Goa

sendHello = accountId =>
  sendWith = smtpCfg =>
    smtpSend (
      Goa.toSmtpConfig
        smtpCfg
        ["team@example.com"]
        None
        None
        "Hello"
        "Sent with GNOME Online Accounts credentials"
    )

  accountId
     |> Goa.smtpConfig #smtpCfg
     |> sendWith
```

## Notes and failure behavior

- `listMailAccounts` only reports accounts that currently expose GOA's mail interface. If the user disables mail for an account in GNOME Settings, it disappears from this list.
- `imapConfig` and `smtpConfig` refresh credentials through GOA before returning configuration values.
- `AttentionNeeded` means the user likely needs to reauthenticate in GNOME Settings before your application can continue.
- `smtpConfig` may fail with `UnsupportedAuth` when the GOA account exposes an SMTP setup that the current `aivi.email` transport cannot represent safely.
- `GoaImapConfig` and `GoaSmtpConfig` are intentionally partial: GOA knows the desktop account and its credentials, but it does not know your mailbox query or the message body you want to send.

## See also

- [Email Module](./email.md) for the actual IMAP and SMTP operations
- [IMAP Email Sources](../../syntax/external_sources/imap_email.md) for the source-oriented mailbox read surface
