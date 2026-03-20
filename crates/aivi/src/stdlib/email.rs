pub const MODULE_NAME: &str = "aivi.email";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.email
export EmailAuth, ImapConfig, SmtpConfig, MimePart, MailboxInfo, IdleResult
export imap, smtpSend, mimeParts, flattenBodies
export imapOpen, imapSelect, imapExamine, imapSearch, imapFetch
export imapSetFlags, imapAddFlags, imapRemoveFlags, imapExpunge
export imapCopy, imapMove
export imapListMailboxes, imapCreateMailbox, imapDeleteMailbox, imapRenameMailbox
export imapAppend, imapIdle

use aivi

EmailAuth = Password Text | OAuth2 Text

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

MimePart = {
  contentType: Text
  body: Text
}

MailboxInfo = {
  name: Text
  separator: Option Text
  attributes: List Text
}

IdleResult = TimedOut | MailboxChanged

imap : ImapConfig -> Effect Text (List A)
imap = config => load (email.imap config)

smtpSend : SmtpConfig -> Effect Text Unit
smtpSend = config => email.smtpSend config

mimeParts : Text -> List MimePart
mimeParts = raw => email.mimeParts raw

flattenBodies : List MimePart -> Text
flattenBodies = parts => parts match
  | [] => ""
  | [x] => x.body
  | [x, ...xs] => text.concat [x.body, "\n", flattenBodies xs]

imapOpen : ImapConfig -> Resource Text ImapSession
imapOpen = config =>
  config
     |> email.imapOpen @cleanup email.imapClose #session

imapSelect : Text -> ImapSession -> Effect Text MailboxInfo
imapSelect = mailbox => session => email.imapSelect mailbox session

imapExamine : Text -> ImapSession -> Effect Text MailboxInfo
imapExamine = mailbox => session => email.imapExamine mailbox session

imapSearch : Text -> ImapSession -> Effect Text (List Int)
imapSearch = query => session => email.imapSearch query session

imapFetch : List Int -> ImapSession -> Effect Text (List A)
imapFetch = uids => session => email.imapFetch uids session

imapSetFlags : List Int -> List Text -> ImapSession -> Effect Text Unit
imapSetFlags = uids => flags => session => email.imapSetFlags uids flags session

imapAddFlags : List Int -> List Text -> ImapSession -> Effect Text Unit
imapAddFlags = uids => flags => session => email.imapAddFlags uids flags session

imapRemoveFlags : List Int -> List Text -> ImapSession -> Effect Text Unit
imapRemoveFlags = uids => flags => session => email.imapRemoveFlags uids flags session

imapExpunge : ImapSession -> Effect Text Unit
imapExpunge = session => email.imapExpunge session

imapCopy : List Int -> Text -> ImapSession -> Effect Text Unit
imapCopy = uids => mailbox => session => email.imapCopy uids mailbox session

imapMove : List Int -> Text -> ImapSession -> Effect Text Unit
imapMove = uids => mailbox => session => email.imapMove uids mailbox session

imapListMailboxes : ImapSession -> Effect Text (List MailboxInfo)
imapListMailboxes = session => email.imapListMailboxes session

imapCreateMailbox : Text -> ImapSession -> Effect Text Unit
imapCreateMailbox = name => session => email.imapCreateMailbox name session

imapDeleteMailbox : Text -> ImapSession -> Effect Text Unit
imapDeleteMailbox = name => session => email.imapDeleteMailbox name session

imapRenameMailbox : Text -> Text -> ImapSession -> Effect Text Unit
imapRenameMailbox = from => to => session => email.imapRenameMailbox from to session

imapAppend : Text -> Text -> ImapSession -> Effect Text Unit
imapAppend = mailbox => content => session => email.imapAppend mailbox content session

imapIdle : Int -> ImapSession -> Effect Text IdleResult
imapIdle = timeoutSecs => session => email.imapIdle timeoutSecs session
"#;
