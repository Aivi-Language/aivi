use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct AiviEmailError {
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum EmailAuth {
    Password(String),
    OAuth2(String),
}

#[derive(Debug, Clone)]
pub struct ImapConfig {
    pub host: String,
    pub user: String,
    pub auth: EmailAuth,
    pub mailbox: String,
    pub filter: String,
    pub limit: i64,
    pub port: i64,
    pub starttls: bool,
}

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub user: String,
    pub auth: EmailAuth,
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub port: i64,
    pub starttls: bool,
}

#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub uid: Option<u32>,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub date: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct MimePart {
    pub content_type: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct MailboxInfo {
    pub name: String,
    pub separator: Option<String>,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdleResult {
    TimedOut,
    MailboxChanged,
}

// XOAUTH2 authenticator for the imap crate
struct XOAuth2 {
    token_response: Vec<u8>,
}

impl XOAuth2 {
    fn new(user: &str, access_token: &str) -> Self {
        // XOAUTH2 format: "user=<user>\x01auth=Bearer <token>\x01\x01"
        let auth_string = format!("user={}\x01auth=Bearer {}\x01\x01", user, access_token);
        Self {
            token_response: auth_string.into_bytes(),
        }
    }
}

impl imap::Authenticator for XOAuth2 {
    type Response = Vec<u8>;
    fn process(&self, _challenge: &[u8]) -> Self::Response {
        self.token_response.clone()
    }
}

pub type ImapSession = Arc<Mutex<imap::Session<Box<dyn imap::ImapConnection>>>>;

fn connect_and_auth(
    config: &ImapConfig,
) -> Result<imap::Session<Box<dyn imap::ImapConnection>>, AiviEmailError> {
    let mut builder = imap::ClientBuilder::new(&config.host, config.port as u16);
    if config.starttls {
        builder = builder.mode(imap::ConnectionMode::StartTls);
    }
    let client = builder.connect().map_err(|err| AiviEmailError {
        message: format!("email.imap transport error: {err}"),
    })?;

    match &config.auth {
        EmailAuth::Password(password) => {
            client
                .login(&config.user, password)
                .map_err(|(err, _)| AiviEmailError {
                    message: format!("email.imap auth error: {err}"),
                })
        }
        EmailAuth::OAuth2(token) => {
            let auth = XOAuth2::new(&config.user, token);
            client
                .authenticate("XOAUTH2", &auth)
                .map_err(|(err, _)| AiviEmailError {
                    message: format!("email.imap OAuth2 auth error: {err}"),
                })
        }
    }
}

pub fn load_imap_messages(config: ImapConfig) -> Result<Vec<EmailMessage>, AiviEmailError> {
    let mut session = connect_and_auth(&config)?;

    session
        .select(&config.mailbox)
        .map_err(|err| AiviEmailError {
            message: format!("email.imap mailbox error: {err}"),
        })?;

    let ids = session
        .search(&config.filter)
        .map_err(|err| AiviEmailError {
            message: format!("email.imap search error: {err}"),
        })?;
    if ids.is_empty() {
        let _ = session.logout();
        return Ok(Vec::new());
    }

    let mut selected = ids.into_iter().collect::<Vec<_>>();
    selected.sort_unstable();
    selected.reverse();
    selected.truncate(config.limit as usize);
    selected.sort_unstable();
    let sequence_set = selected
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let fetches = session
        .fetch(sequence_set, "UID RFC822")
        .map_err(|err| AiviEmailError {
            message: format!("email.imap fetch error: {err}"),
        })?;

    let mut out = Vec::new();
    for msg in fetches.iter() {
        let Some(raw) = msg.body() else {
            continue;
        };
        let parsed = mailparse::parse_mail(raw).map_err(|err| AiviEmailError {
            message: format!("email.imap decode error: {err}"),
        })?;
        out.push(EmailMessage {
            uid: msg.uid,
            subject: header_value(&parsed, "Subject"),
            from: header_value(&parsed, "From"),
            to: header_value(&parsed, "To"),
            date: header_value(&parsed, "Date"),
            body: parsed.get_body().unwrap_or_default(),
        });
    }

    let _ = session.logout();
    Ok(out)
}

// --- Session-based API ---

pub fn imap_open(config: &ImapConfig) -> Result<ImapSession, AiviEmailError> {
    let session = connect_and_auth(config)?;
    Ok(Arc::new(Mutex::new(session)))
}

pub fn imap_close(session: &ImapSession) -> Result<(), AiviEmailError> {
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    s.logout().map_err(|err| AiviEmailError {
        message: format!("email.imap logout error: {err}"),
    })?;
    Ok(())
}

pub fn imap_select(mailbox: &str, session: &ImapSession) -> Result<MailboxInfo, AiviEmailError> {
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    let mb = s.select(mailbox).map_err(|err| AiviEmailError {
        message: format!("email.imapSelect error: {err}"),
    })?;
    Ok(MailboxInfo {
        name: mailbox.to_string(),
        separator: None,
        attributes: mb.flags.iter().map(|f| format!("{f}")).collect(),
    })
}

pub fn imap_examine(mailbox: &str, session: &ImapSession) -> Result<MailboxInfo, AiviEmailError> {
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    let mb = s.examine(mailbox).map_err(|err| AiviEmailError {
        message: format!("email.imapExamine error: {err}"),
    })?;
    Ok(MailboxInfo {
        name: mailbox.to_string(),
        separator: None,
        attributes: mb.flags.iter().map(|f| format!("{f}")).collect(),
    })
}

pub fn imap_search(query: &str, session: &ImapSession) -> Result<Vec<u32>, AiviEmailError> {
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    let ids = s.uid_search(query).map_err(|err| AiviEmailError {
        message: format!("email.imapSearch error: {err}"),
    })?;
    let mut uids: Vec<u32> = ids.into_iter().collect();
    uids.sort_unstable();
    Ok(uids)
}

pub fn imap_fetch(
    uids: &[u32],
    session: &ImapSession,
) -> Result<Vec<EmailMessage>, AiviEmailError> {
    if uids.is_empty() {
        return Ok(Vec::new());
    }
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    let uid_set = uids
        .iter()
        .map(|u| u.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let fetches = s
        .uid_fetch(uid_set, "UID RFC822")
        .map_err(|err| AiviEmailError {
            message: format!("email.imapFetch error: {err}"),
        })?;
    let mut out = Vec::new();
    for msg in fetches.iter() {
        let Some(raw) = msg.body() else {
            continue;
        };
        let parsed = mailparse::parse_mail(raw).map_err(|err| AiviEmailError {
            message: format!("email.imapFetch decode error: {err}"),
        })?;
        out.push(EmailMessage {
            uid: msg.uid,
            subject: header_value(&parsed, "Subject"),
            from: header_value(&parsed, "From"),
            to: header_value(&parsed, "To"),
            date: header_value(&parsed, "Date"),
            body: parsed.get_body().unwrap_or_default(),
        });
    }
    Ok(out)
}

pub fn imap_set_flags(
    uids: &[u32],
    flags: &[String],
    session: &ImapSession,
) -> Result<(), AiviEmailError> {
    if uids.is_empty() {
        return Ok(());
    }
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    let uid_set = uid_set_string(uids);
    let flag_str = flags.join(" ");
    s.uid_store(&uid_set, format!("FLAGS ({flag_str})"))
        .map_err(|err| AiviEmailError {
            message: format!("email.imapSetFlags error: {err}"),
        })?;
    Ok(())
}

pub fn imap_add_flags(
    uids: &[u32],
    flags: &[String],
    session: &ImapSession,
) -> Result<(), AiviEmailError> {
    if uids.is_empty() {
        return Ok(());
    }
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    let uid_set = uid_set_string(uids);
    let flag_str = flags.join(" ");
    s.uid_store(&uid_set, format!("+FLAGS ({flag_str})"))
        .map_err(|err| AiviEmailError {
            message: format!("email.imapAddFlags error: {err}"),
        })?;
    Ok(())
}

pub fn imap_remove_flags(
    uids: &[u32],
    flags: &[String],
    session: &ImapSession,
) -> Result<(), AiviEmailError> {
    if uids.is_empty() {
        return Ok(());
    }
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    let uid_set = uid_set_string(uids);
    let flag_str = flags.join(" ");
    s.uid_store(&uid_set, format!("-FLAGS ({flag_str})"))
        .map_err(|err| AiviEmailError {
            message: format!("email.imapRemoveFlags error: {err}"),
        })?;
    Ok(())
}

pub fn imap_expunge(session: &ImapSession) -> Result<(), AiviEmailError> {
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    s.expunge().map_err(|err| AiviEmailError {
        message: format!("email.imapExpunge error: {err}"),
    })?;
    Ok(())
}

pub fn imap_copy(uids: &[u32], mailbox: &str, session: &ImapSession) -> Result<(), AiviEmailError> {
    if uids.is_empty() {
        return Ok(());
    }
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    let uid_set = uid_set_string(uids);
    s.uid_copy(&uid_set, mailbox)
        .map_err(|err| AiviEmailError {
            message: format!("email.imapCopy error: {err}"),
        })?;
    Ok(())
}

pub fn imap_move(uids: &[u32], mailbox: &str, session: &ImapSession) -> Result<(), AiviEmailError> {
    if uids.is_empty() {
        return Ok(());
    }
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    let uid_set = uid_set_string(uids);
    s.uid_mv(&uid_set, mailbox).map_err(|err| AiviEmailError {
        message: format!("email.imapMove error: {err}"),
    })?;
    Ok(())
}

pub fn imap_list_mailboxes(session: &ImapSession) -> Result<Vec<MailboxInfo>, AiviEmailError> {
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    let names = s.list(Some(""), Some("*")).map_err(|err| AiviEmailError {
        message: format!("email.imapListMailboxes error: {err}"),
    })?;
    let mut out = Vec::new();
    for name in names.iter() {
        out.push(MailboxInfo {
            name: name.name().to_string(),
            separator: name.delimiter().map(|c| c.to_string()),
            attributes: name.attributes().iter().map(|a| format!("{a:?}")).collect(),
        });
    }
    Ok(out)
}

pub fn imap_create_mailbox(name: &str, session: &ImapSession) -> Result<(), AiviEmailError> {
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    s.create(name).map_err(|err| AiviEmailError {
        message: format!("email.imapCreateMailbox error: {err}"),
    })?;
    Ok(())
}

pub fn imap_delete_mailbox(name: &str, session: &ImapSession) -> Result<(), AiviEmailError> {
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    s.delete(name).map_err(|err| AiviEmailError {
        message: format!("email.imapDeleteMailbox error: {err}"),
    })?;
    Ok(())
}

pub fn imap_rename_mailbox(
    from: &str,
    to: &str,
    session: &ImapSession,
) -> Result<(), AiviEmailError> {
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    s.rename(from, to).map_err(|err| AiviEmailError {
        message: format!("email.imapRenameMailbox error: {err}"),
    })?;
    Ok(())
}

pub fn imap_append(
    mailbox: &str,
    content: &str,
    session: &ImapSession,
) -> Result<(), AiviEmailError> {
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    s.append(mailbox, content.as_bytes())
        .finish()
        .map_err(|err| AiviEmailError {
            message: format!("email.imapAppend error: {err}"),
        })?;
    Ok(())
}

pub fn imap_idle(timeout_secs: u64, session: &ImapSession) -> Result<IdleResult, AiviEmailError> {
    let mut s = session.lock().map_err(|e| AiviEmailError {
        message: format!("email.imap lock error: {e}"),
    })?;
    let mut idle = s.idle();
    idle.timeout(Duration::from_secs(timeout_secs));
    idle.keepalive(false);
    let outcome = idle
        .wait_while(|_response| {
            // Return false to stop waiting on any server response
            false
        })
        .map_err(|err| AiviEmailError {
            message: format!("email.imapIdle error: {err}"),
        })?;
    match outcome {
        imap::extensions::idle::WaitOutcome::TimedOut => Ok(IdleResult::TimedOut),
        imap::extensions::idle::WaitOutcome::MailboxChanged => Ok(IdleResult::MailboxChanged),
    }
}

fn build_smtp_message(config: &SmtpConfig) -> Result<Message, AiviEmailError> {
    let mut builder = Message::builder().from(config.from.parse().map_err(|e| AiviEmailError {
        message: format!("Invalid from address: {e}"),
    })?);

    for addr in &config.to {
        builder = builder.to(addr.parse().map_err(|e| AiviEmailError {
            message: format!("Invalid to address '{addr}': {e}"),
        })?);
    }
    for addr in &config.cc {
        builder = builder.cc(addr.parse().map_err(|e| AiviEmailError {
            message: format!("Invalid cc address '{addr}': {e}"),
        })?);
    }
    for addr in &config.bcc {
        builder = builder.bcc(addr.parse().map_err(|e| AiviEmailError {
            message: format!("Invalid bcc address '{addr}': {e}"),
        })?);
    }

    let email = builder
        .subject(config.subject.clone())
        .body(config.body.clone())
        .map_err(|e| AiviEmailError {
            message: format!("Failed to build email: {e}"),
        })?;
    Ok(email)
}

fn smtp_credentials(config: &SmtpConfig) -> Credentials {
    match &config.auth {
        EmailAuth::Password(password) => Credentials::new(config.user.clone(), password.clone()),
        EmailAuth::OAuth2(token) => {
            // lettre doesn't natively support XOAUTH2, use the token as password
            // with the access_token mechanism
            Credentials::new(config.user.clone(), token.clone())
        }
    }
}

fn build_smtp_transport(config: &SmtpConfig) -> Result<SmtpTransport, AiviEmailError> {
    let creds = smtp_credentials(config);
    let mailer = if config.starttls {
        SmtpTransport::starttls_relay(&config.host)
            .map_err(|e| AiviEmailError {
                message: format!("Invalid SMTP host: {e}"),
            })?
            .port(config.port as u16)
            .credentials(creds)
            .build()
    } else {
        SmtpTransport::relay(&config.host)
            .map_err(|e| AiviEmailError {
                message: format!("Invalid SMTP host: {e}"),
            })?
            .port(config.port as u16)
            .credentials(creds)
            .build()
    };
    Ok(mailer)
}

fn send_smtp_message_with_transport<T>(
    config: &SmtpConfig,
    transport: &T,
) -> Result<(), AiviEmailError>
where
    T: Transport,
    T::Error: std::fmt::Display,
{
    let email = build_smtp_message(config)?;
    transport
        .send(&email)
        .map(|_| ())
        .map_err(|e| AiviEmailError {
            message: format!("SMTP send failed: {e}"),
        })
}

pub fn send_smtp_message(config: SmtpConfig) -> Result<(), AiviEmailError> {
    let mailer = build_smtp_transport(&config)?;
    send_smtp_message_with_transport(&config, &mailer)
}

pub fn parse_mime_parts(raw: &str) -> Result<Vec<MimePart>, AiviEmailError> {
    let parsed = mailparse::parse_mail(raw.as_bytes()).map_err(|err| AiviEmailError {
        message: format!("email.mimeParts decode error: {err}"),
    })?;
    let mut parts = Vec::new();
    collect_parts(&parsed, &mut parts)?;
    Ok(parts)
}

fn collect_parts(
    parsed: &mailparse::ParsedMail<'_>,
    out: &mut Vec<MimePart>,
) -> Result<(), AiviEmailError> {
    out.push(MimePart {
        content_type: parsed.ctype.mimetype.clone(),
        body: parsed.get_body().unwrap_or_default(),
    });
    for subpart in &parsed.subparts {
        collect_parts(subpart, out)?;
    }
    Ok(())
}

fn header_value(parsed: &mailparse::ParsedMail<'_>, name: &str) -> Option<String> {
    parsed
        .headers
        .iter()
        .find(|h| h.get_key_ref().eq_ignore_ascii_case(name))
        .map(|h| h.get_value())
}

fn uid_set_string(uids: &[u32]) -> String {
    uids.iter()
        .map(|u| u.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use imap::extensions::idle::SetReadTimeout;
    use lettre::transport::stub::StubTransport;
    use std::io::{self, Read, Write};
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Default)]
    struct RecordingState {
        read_buf: Vec<u8>,
        read_pos: usize,
        written_buf: Vec<u8>,
        read_timeouts: Vec<Option<Duration>>,
        on_done: Option<Vec<u8>>,
    }

    #[derive(Clone, Debug)]
    struct RecordingImapStream {
        state: Arc<Mutex<RecordingState>>,
    }

    impl RecordingImapStream {
        fn scripted(
            script: impl Into<Vec<u8>>,
            on_done: Option<Vec<u8>>,
        ) -> (Self, Arc<Mutex<RecordingState>>) {
            let state = Arc::new(Mutex::new(RecordingState {
                read_buf: script.into(),
                read_pos: 0,
                written_buf: Vec::new(),
                read_timeouts: Vec::new(),
                on_done,
            }));
            (
                Self {
                    state: state.clone(),
                },
                state,
            )
        }
    }

    impl Read for RecordingImapStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let mut state = self.state.lock().unwrap();
            if state.read_pos >= state.read_buf.len() {
                if state
                    .read_timeouts
                    .last()
                    .is_some_and(|timeout| timeout.is_some())
                {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "timed out"));
                }
                return Ok(0);
            }

            let count = (state.read_buf.len() - state.read_pos).min(buf.len());
            buf[..count].copy_from_slice(&state.read_buf[state.read_pos..state.read_pos + count]);
            state.read_pos += count;
            Ok(count)
        }
    }

    impl Write for RecordingImapStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut state = self.state.lock().unwrap();
            state.written_buf.extend_from_slice(buf);
            if state.written_buf.ends_with(b"DONE\r\n") {
                if let Some(extra) = state.on_done.take() {
                    state.read_buf.extend(extra);
                }
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl SetReadTimeout for RecordingImapStream {
        fn set_read_timeout(&mut self, timeout: Option<Duration>) -> imap::Result<()> {
            self.state.lock().unwrap().read_timeouts.push(timeout);
            Ok(())
        }
    }

    fn login_script(extra: &str) -> String {
        format!("* OK ready\r\na1 OK LOGIN completed\r\n{extra}")
    }

    fn scripted_session(
        extra: &str,
        on_done: Option<&str>,
    ) -> (ImapSession, Arc<Mutex<RecordingState>>) {
        let (stream, state) = RecordingImapStream::scripted(
            login_script(extra).into_bytes(),
            on_done.map(|value| value.as_bytes().to_vec()),
        );
        let mut client = imap::Client::new(Box::new(stream) as Box<dyn imap::ImapConnection>);
        client.read_greeting().expect("read greeting");
        let session = client.login("user", "pass").expect("login");
        (Arc::new(Mutex::new(session)), state)
    }

    fn written(state: &Arc<Mutex<RecordingState>>) -> String {
        String::from_utf8(state.lock().unwrap().written_buf.clone()).expect("utf8 commands")
    }

    fn smtp_config() -> SmtpConfig {
        SmtpConfig {
            host: "smtp.example.com".to_string(),
            user: "user".to_string(),
            auth: EmailAuth::Password("secret".to_string()),
            from: "from@example.com".to_string(),
            to: vec!["to@example.com".to_string()],
            cc: vec!["cc@example.com".to_string()],
            bcc: vec!["bcc@example.com".to_string()],
            subject: "hello".to_string(),
            body: "body text".to_string(),
            port: 465,
            starttls: false,
        }
    }

    #[test]
    fn smtp_send_with_stub_transport_logs_envelope_and_body() {
        let config = smtp_config();
        let transport = StubTransport::new_ok();

        send_smtp_message_with_transport(&config, &transport).expect("send succeeds");

        let messages = transport.messages();
        assert_eq!(messages.len(), 1);
        let (envelope, raw) = &messages[0];
        assert_eq!(
            envelope.from().map(ToString::to_string),
            Some(config.from.clone())
        );
        assert_eq!(
            envelope
                .to()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec![
                "to@example.com".to_string(),
                "cc@example.com".to_string(),
                "bcc@example.com".to_string(),
            ]
        );
        assert!(raw.contains("Subject: hello"));
        assert!(raw.contains("To: to@example.com"));
        assert!(raw.contains("Cc: cc@example.com"));
        assert!(!raw.contains("Bcc:"));
        assert!(raw.contains("\r\n\r\nbody text"));
    }

    #[test]
    fn smtp_send_with_stub_transport_surfaces_transport_errors() {
        let err = send_smtp_message_with_transport(&smtp_config(), &StubTransport::new_error())
            .expect_err("stub send should fail");
        assert!(err.message.contains("SMTP send failed"));
    }

    #[test]
    fn imap_select_and_examine_return_mailbox_info() {
        let (select_session, select_state) = scripted_session(
            "* FLAGS (\\Seen \\Deleted)\r\n* 2 EXISTS\r\n* 0 RECENT\r\na2 OK [READ-WRITE] SELECT completed\r\n",
            None,
        );
        let selected = imap_select("INBOX", &select_session).expect("select succeeds");
        assert_eq!(selected.name, "INBOX");
        assert!(!selected.attributes.is_empty());
        assert!(written(&select_state).contains("a2 SELECT \"INBOX\"\r\n"));

        let (examine_session, examine_state) = scripted_session(
            "* FLAGS (\\Seen)\r\n* 2 EXISTS\r\n* 0 RECENT\r\na2 OK [READ-ONLY] EXAMINE completed\r\n",
            None,
        );
        let examined = imap_examine("Archive", &examine_session).expect("examine succeeds");
        assert_eq!(examined.name, "Archive");
        assert!(!examined.attributes.is_empty());
        assert!(written(&examine_state).contains("a2 EXAMINE \"Archive\"\r\n"));
    }

    #[test]
    fn imap_search_and_fetch_decode_messages() {
        let raw_message = concat!(
            "Subject: hello\r\n",
            "From: from@example.com\r\n",
            "To: to@example.com\r\n",
            "Date: Tue, 1 Jan 2024 00:00:00 +0000\r\n",
            "\r\n",
            "Body text\r\n"
        );
        let fetch_response = format!(
            "* SEARCH 9 3 5\r\n\
             a2 OK SEARCH completed\r\n\
             * 1 FETCH (UID 5 RFC822 {{{}}}\r\n{}\r\n)\r\n\
             a3 OK FETCH completed\r\n",
            raw_message.len(),
            raw_message.trim_end_matches("\r\n")
        );
        let (session, state) = scripted_session(&fetch_response, None);

        let uids = imap_search("UNSEEN", &session).expect("search succeeds");
        assert_eq!(uids, vec![3, 5, 9]);

        let messages = imap_fetch(&[5], &session).expect("fetch succeeds");
        assert_eq!(messages.len(), 1);
        let message = &messages[0];
        assert_eq!(message.uid, Some(5));
        assert_eq!(message.subject.as_deref(), Some("hello"));
        assert_eq!(message.from.as_deref(), Some("from@example.com"));
        assert_eq!(message.to.as_deref(), Some("to@example.com"));
        assert_eq!(
            message.date.as_deref(),
            Some("Tue, 1 Jan 2024 00:00:00 +0000")
        );
        assert_eq!(message.body.trim(), "Body text");

        let commands = written(&state);
        assert!(commands.contains("a2 UID SEARCH UNSEEN\r\n"));
        assert!(commands.contains("a3 UID FETCH 5 UID RFC822\r\n"));
    }

    #[test]
    fn imap_flag_mutations_issue_distinct_uid_store_commands() {
        let (session, state) = scripted_session(
            "* 5 FETCH (FLAGS (\\Seen))\r\na2 OK STORE completed\r\n\
             * 5 FETCH (FLAGS (\\Seen \\Flagged))\r\na3 OK STORE completed\r\n\
             * 5 FETCH (FLAGS (\\Seen))\r\na4 OK STORE completed\r\n",
            None,
        );

        imap_set_flags(&[5], &[r"\Seen".to_string()], &session).expect("set flags");
        imap_add_flags(&[5], &[r"\Flagged".to_string()], &session).expect("add flags");
        imap_remove_flags(&[5], &[r"\Flagged".to_string()], &session).expect("remove flags");

        let commands = written(&state);
        assert!(commands.contains("a2 UID STORE 5 FLAGS (\\Seen)\r\n"));
        assert!(commands.contains("a3 UID STORE 5 +FLAGS (\\Flagged)\r\n"));
        assert!(commands.contains("a4 UID STORE 5 -FLAGS (\\Flagged)\r\n"));
    }

    #[test]
    fn imap_lists_and_manages_mailboxes() {
        let (session, state) = scripted_session(
            "* LIST (\\HasNoChildren) \"/\" \"INBOX\"\r\n\
             * LIST (\\HasNoChildren \\Sent) \"/\" \"Archive\"\r\n\
             a2 OK LIST completed\r\n\
             a3 OK CREATE completed\r\n\
             a4 OK RENAME completed\r\n\
             a5 OK DELETE completed\r\n",
            None,
        );

        let mailboxes = imap_list_mailboxes(&session).expect("list mailboxes");
        assert_eq!(mailboxes.len(), 2);
        assert_eq!(mailboxes[0].name, "INBOX");
        assert_eq!(mailboxes[0].separator.as_deref(), Some("/"));

        imap_create_mailbox("Projects", &session).expect("create");
        imap_rename_mailbox("Projects", "Projects-2024", &session).expect("rename");
        imap_delete_mailbox("Projects-2024", &session).expect("delete");

        let commands = written(&state);
        assert!(commands.contains("a2 LIST \"\" *\r\n"));
        assert!(commands.contains("a3 CREATE \"Projects\"\r\n"));
        assert!(commands.contains("a4 RENAME \"Projects\" \"Projects-2024\"\r\n"));
        assert!(commands.contains("a5 DELETE \"Projects-2024\"\r\n"));
    }

    #[test]
    fn imap_copy_move_expunge_append_and_close_issue_commands() {
        let raw_append = concat!(
            "Subject: appended\r\n",
            "From: from@example.com\r\n",
            "To: to@example.com\r\n",
            "\r\n",
            "New body"
        );
        let (session, state) = scripted_session(
            "a2 OK COPY completed\r\n\
             * OK [COPYUID 1 5 6] Moved UIDs.\r\n* 1 EXPUNGE\r\na3 OK Move completed\r\n\
             * 1 EXPUNGE\r\na4 OK EXPUNGE completed\r\n\
             + Ready for literal data\r\n\
             a5 OK APPEND completed\r\n\
             * BYE Logging out\r\na6 OK LOGOUT completed\r\n",
            None,
        );

        imap_copy(&[5], "Archive", &session).expect("copy");
        imap_move(&[5], "Processed", &session).expect("move");
        imap_expunge(&session).expect("expunge");
        imap_append("INBOX", raw_append, &session).expect("append");
        imap_close(&session).expect("close");

        let commands = written(&state);
        let append_prefix = format!("a5 APPEND \"INBOX\" () {{{}}}\r\n", raw_append.len());
        assert!(commands.contains("a2 UID COPY 5 Archive\r\n"));
        assert!(commands.contains("a3 UID MOVE 5 \"Processed\"\r\n"));
        assert!(commands.contains("a4 EXPUNGE\r\n"));
        assert!(commands.contains(&append_prefix));
        assert!(commands.contains(raw_append));
        assert!(commands.contains("a6 LOGOUT\r\n"));
    }

    #[test]
    fn imap_idle_reports_timeout_without_keepalive_loop() {
        let (session, state) = scripted_session("+ idling\r\n", Some("a2 OK IDLE terminated\r\n"));

        let result = imap_idle(1, &session).expect("idle succeeds");
        assert_eq!(result, IdleResult::TimedOut);

        let state = state.lock().unwrap();
        assert!(state.read_timeouts.contains(&Some(Duration::from_secs(1))));
        assert!(state.read_timeouts.contains(&None));
        let written = String::from_utf8(state.written_buf.clone()).expect("utf8");
        assert!(written.contains("a2 IDLE\r\n"));
        assert!(written.contains("DONE\r\n"));
    }

    #[test]
    fn imap_idle_reports_mailbox_changes() {
        let (session, state) = scripted_session(
            "+ idling\r\n* 4 EXISTS\r\n",
            Some("a2 OK IDLE terminated\r\n"),
        );

        let result = imap_idle(1, &session).expect("idle succeeds");
        assert_eq!(result, IdleResult::MailboxChanged);

        let written = written(&state);
        assert!(written.contains("a2 IDLE\r\n"));
        assert!(written.contains("DONE\r\n"));
    }
}
