pub const MODULE_NAME: &str = "aivi.email";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.email
export ImapConfig, SmtpConfig, MimePart
export imap, smtpSend, mimeParts, flattenBodies

use aivi

ImapConfig = {
  host: Text
  user: Text
  password: Text
  mailbox: Option Text
  filter: Option Text
  limit: Option Int
  port: Option Int
}

SmtpConfig = {
  host: Text
  user: Text
  password: Text
  from: Text
  to: Text
  subject: Text
  body: Text
}

MimePart = {
  contentType: Text
  body: Text
}

imap : ImapConfig -> Effect Text (List A)
imap = config => load (email.imap config)

smtpSend : SmtpConfig -> Effect Text Unit
smtpSend = config => email.smtpSend config

mimeParts : Text -> Result Text (List MimePart)
mimeParts = raw =>
  attempt (email.mimeParts raw) match
    | Ok value => Ok value
    | Err err  => Err err

flattenBodies : List MimePart -> Text
flattenBodies = parts => parts match
  | [] => ""
  | [x] => x.body
  | [x, ...xs] => text.concat [x.body, "\n", flattenBodies xs]
"#;
