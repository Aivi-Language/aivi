pub const MODULE_NAME: &str = "aivi.gnome.onlineAccounts";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.gnome.onlineAccounts
export GoaError, GoaMailAccount, GoaImapConfig, GoaSmtpConfig
export listMailAccounts, ensureCredentials, imapConfig, smtpConfig
export toImapConfig, toSmtpConfig

use aivi
use aivi.email (EmailAuth, ImapConfig, SmtpConfig)

GoaError =
  | PlatformUnsupported
  | ServiceUnavailable Text
  | AttentionNeeded Text
  | AccountNotFound Text
  | MailUnsupported Text
  | UnsupportedAuth Text
  | Credentials Text

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

GoaImapConfig = {
  host: Text
  user: Text
  auth: EmailAuth
  port: Option Int
  starttls: Option Bool
}

GoaSmtpConfig = {
  host: Text
  user: Text
  auth: EmailAuth
  from: Text
  port: Option Int
  starttls: Option Bool
}

listMailAccounts : Effect GoaError (List GoaMailAccount)
listMailAccounts = gnomeOnlineAccounts.listMailAccounts Unit

ensureCredentials : Text -> Effect GoaError Unit
ensureCredentials = accountId => gnomeOnlineAccounts.ensureCredentials accountId

imapConfig : Text -> Effect GoaError GoaImapConfig
imapConfig = accountId => gnomeOnlineAccounts.imapConfig accountId

smtpConfig : Text -> Effect GoaError GoaSmtpConfig
smtpConfig = accountId => gnomeOnlineAccounts.smtpConfig accountId

toImapConfig : GoaImapConfig -> Option Text -> Option Text -> Option Int -> ImapConfig
toImapConfig = cfg => mailbox => filter => limit => {
  host: cfg.host
  user: cfg.user
  auth: cfg.auth
  port: cfg.port
  starttls: cfg.starttls
  mailbox: mailbox
  filter: filter
  limit: limit
}

toSmtpConfig : GoaSmtpConfig -> List Text -> Option (List Text) -> Option (List Text) -> Text -> Text -> SmtpConfig
toSmtpConfig = cfg => to => cc => bcc => subject => body => {
  host: cfg.host
  user: cfg.user
  auth: cfg.auth
  from: cfg.from
  to: to
  cc: cc
  bcc: bcc
  subject: subject
  body: body
  port: cfg.port
  starttls: cfg.starttls
}
"#;
