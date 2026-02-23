pub const MODULE_NAME: &str = "aivi.email";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.email
export ImapConfig, imap

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

imap : ImapConfig -> Effect Text (List A)
imap = config => load (email.imap config)
"#;
