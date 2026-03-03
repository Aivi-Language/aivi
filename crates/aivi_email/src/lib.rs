use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct AiviEmailError {
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ImapConfig {
    pub host: String,
    pub user: String,
    pub password: String,
    pub mailbox: String,
    pub filter: String,
    pub limit: i64,
    pub port: i64,
}

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub user: String,
    pub password: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body: String,
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

pub fn load_imap_messages(config: ImapConfig) -> Result<Vec<EmailMessage>, AiviEmailError> {
    let client = imap::ClientBuilder::new(&config.host, config.port as u16)
        .connect()
        .map_err(|err| AiviEmailError {
            message: format!("email.imap transport error: {err}"),
        })?;
    let mut session = client
        .login(config.user, config.password)
        .map_err(|(err, _)| AiviEmailError {
            message: format!("email.imap auth error: {err}"),
        })?;

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

pub fn send_smtp_message(config: SmtpConfig) -> Result<(), AiviEmailError> {
    let email = Message::builder()
        .from(config.from.parse().map_err(|e| AiviEmailError {
            message: format!("Invalid from address: {e}"),
        })?)
        .to(config.to.parse().map_err(|e| AiviEmailError {
            message: format!("Invalid to address: {e}"),
        })?)
        .subject(config.subject)
        .body(config.body)
        .map_err(|e| AiviEmailError {
            message: format!("Failed to build email: {e}"),
        })?;

    let creds = Credentials::new(config.user, config.password);

    let mailer = SmtpTransport::relay(&config.host)
        .map_err(|e| AiviEmailError {
            message: format!("Invalid SMTP host: {e}"),
        })?
        .credentials(creds)
        .build();

    mailer.send(&email).map_err(|e| AiviEmailError {
        message: format!("SMTP send failed: {e}"),
    })?;

    Ok(())
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
