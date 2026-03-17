use std::collections::{BTreeSet, HashMap};

use aivi_email::EmailAuth;
use thiserror::Error;
use zbus::blocking::{fdo::ObjectManagerProxy, Connection, Proxy};
use zbus::names::OwnedInterfaceName;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

const GOA_SERVICE: &str = "org.gnome.OnlineAccounts";
const GOA_ROOT: &str = "/org/gnome/OnlineAccounts";
const GOA_ACCOUNT_IFACE: &str = "org.gnome.OnlineAccounts.Account";
const GOA_MAIL_IFACE: &str = "org.gnome.OnlineAccounts.Mail";
const GOA_OAUTH2_IFACE: &str = "org.gnome.OnlineAccounts.OAuth2Based";
const GOA_PASSWORD_IFACE: &str = "org.gnome.OnlineAccounts.PasswordBased";

type ManagedInterfaces = HashMap<OwnedInterfaceName, HashMap<String, OwnedValue>>;
type ManagedObjects = HashMap<OwnedObjectPath, ManagedInterfaces>;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GoaError {
    #[error("GNOME Online Accounts is only available on supported desktop platforms")]
    PlatformUnsupported,
    #[error("{0}")]
    ServiceUnavailable(String),
    #[error("{0}")]
    AttentionNeeded(String),
    #[error("{0}")]
    AccountNotFound(String),
    #[error("{0}")]
    MailUnsupported(String),
    #[error("{0}")]
    UnsupportedAuth(String),
    #[error("{0}")]
    Credentials(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoaMailAccount {
    pub id: String,
    pub provider_type: String,
    pub provider_name: String,
    pub presentation_identity: String,
    pub email_address: Option<String>,
    pub attention_needed: bool,
    pub imap_supported: bool,
    pub smtp_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoaImapConfig {
    pub host: String,
    pub user: String,
    pub auth: EmailAuth,
    pub port: Option<i64>,
    pub starttls: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoaSmtpConfig {
    pub host: String,
    pub user: String,
    pub auth: EmailAuth,
    pub from: String,
    pub port: Option<i64>,
    pub starttls: Option<bool>,
}

pub trait GoaBackend {
    fn list_mail_accounts(&self) -> Result<Vec<GoaMailAccount>, GoaError>;
    fn ensure_credentials(&self, account_id: &str) -> Result<(), GoaError>;
    fn imap_config(&self, account_id: &str) -> Result<GoaImapConfig, GoaError>;
    fn smtp_config(&self, account_id: &str) -> Result<GoaSmtpConfig, GoaError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RealGoaBackend;

pub fn list_mail_accounts() -> Result<Vec<GoaMailAccount>, GoaError> {
    RealGoaBackend.list_mail_accounts()
}

pub fn ensure_credentials(account_id: &str) -> Result<(), GoaError> {
    RealGoaBackend.ensure_credentials(account_id)
}

pub fn imap_config(account_id: &str) -> Result<GoaImapConfig, GoaError> {
    RealGoaBackend.imap_config(account_id)
}

pub fn smtp_config(account_id: &str) -> Result<GoaSmtpConfig, GoaError> {
    RealGoaBackend.smtp_config(account_id)
}

#[derive(Clone, Copy, Debug)]
enum AuthTarget {
    Imap,
    Smtp,
}

#[derive(Clone, Debug)]
struct GoaAccountSnapshot {
    path: String,
    interfaces: BTreeSet<String>,
    id: String,
    provider_type: String,
    provider_name: String,
    presentation_identity: String,
    attention_needed: bool,
    email_address: Option<String>,
    imap_supported: bool,
    smtp_supported: bool,
    imap_host: Option<String>,
    imap_user_name: Option<String>,
    imap_use_ssl: bool,
    imap_use_tls: bool,
    smtp_host: Option<String>,
    smtp_user_name: Option<String>,
    smtp_use_auth: bool,
    smtp_auth_xoauth2: bool,
    smtp_use_ssl: bool,
    smtp_use_tls: bool,
}

impl GoaBackend for RealGoaBackend {
    fn list_mail_accounts(&self) -> Result<Vec<GoaMailAccount>, GoaError> {
        ensure_platform_supported()?;
        let conn = session_connection()?;
        let mut accounts = self
            .load_snapshots(&conn)?
            .into_iter()
            .map(snapshot_to_mail_account)
            .collect::<Vec<_>>();
        accounts.sort_by(|left, right| {
            left.presentation_identity
                .cmp(&right.presentation_identity)
                .then_with(|| left.provider_name.cmp(&right.provider_name))
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(accounts)
    }

    fn ensure_credentials(&self, account_id: &str) -> Result<(), GoaError> {
        ensure_platform_supported()?;
        let conn = session_connection()?;
        let snapshot = self.find_snapshot_by_id(&conn, account_id)?;
        ensure_credentials_for_snapshot(&conn, &snapshot)
    }

    fn imap_config(&self, account_id: &str) -> Result<GoaImapConfig, GoaError> {
        ensure_platform_supported()?;
        let conn = session_connection()?;
        let snapshot = self.find_snapshot_by_id(&conn, account_id)?;
        ensure_credentials_for_snapshot(&conn, &snapshot)?;
        let auth = resolve_auth(&conn, &snapshot, AuthTarget::Imap)?;
        snapshot_to_imap_config(&snapshot, auth)
    }

    fn smtp_config(&self, account_id: &str) -> Result<GoaSmtpConfig, GoaError> {
        ensure_platform_supported()?;
        let conn = session_connection()?;
        let snapshot = self.find_snapshot_by_id(&conn, account_id)?;
        ensure_credentials_for_snapshot(&conn, &snapshot)?;
        let auth = resolve_auth(&conn, &snapshot, AuthTarget::Smtp)?;
        snapshot_to_smtp_config(&snapshot, auth)
    }
}

impl RealGoaBackend {
    fn load_snapshots(&self, conn: &Connection) -> Result<Vec<GoaAccountSnapshot>, GoaError> {
        let proxy = ObjectManagerProxy::new(conn, GOA_SERVICE, GOA_ROOT).map_err(|err| {
            GoaError::ServiceUnavailable(format!("could not connect to GOA object manager: {err}"))
        })?;
        let managed: ManagedObjects = proxy.get_managed_objects().map_err(|err| {
            GoaError::ServiceUnavailable(format!("could not enumerate GOA accounts: {err}"))
        })?;

        let mut out = Vec::new();
        for (path, interfaces) in managed {
            let has_account = interfaces
                .keys()
                .any(|interface| interface.as_str() == GOA_ACCOUNT_IFACE);
            let has_mail = interfaces
                .keys()
                .any(|interface| interface.as_str() == GOA_MAIL_IFACE);
            if !has_account || !has_mail {
                continue;
            }
            out.push(load_snapshot(conn, path.as_str(), &interfaces)?);
        }
        Ok(out)
    }

    fn find_snapshot_by_id(
        &self,
        conn: &Connection,
        account_id: &str,
    ) -> Result<GoaAccountSnapshot, GoaError> {
        self.load_snapshots(conn)?
            .into_iter()
            .find(|snapshot| snapshot.id == account_id)
            .ok_or_else(|| {
                GoaError::AccountNotFound(format!("GOA account `{account_id}` was not found"))
            })
    }
}

fn ensure_platform_supported() -> Result<(), GoaError> {
    if cfg!(target_os = "linux") {
        Ok(())
    } else {
        Err(GoaError::PlatformUnsupported)
    }
}

fn session_connection() -> Result<Connection, GoaError> {
    Connection::session().map_err(|err| {
        GoaError::ServiceUnavailable(format!("could not connect to the session bus: {err}"))
    })
}

fn load_snapshot(
    conn: &Connection,
    path: &str,
    interfaces: &ManagedInterfaces,
) -> Result<GoaAccountSnapshot, GoaError> {
    let account = proxy(conn, path, GOA_ACCOUNT_IFACE)?;
    let mail = proxy(conn, path, GOA_MAIL_IFACE)?;

    Ok(GoaAccountSnapshot {
        path: path.to_string(),
        interfaces: interfaces
            .keys()
            .map(|interface| interface.as_str().to_string())
            .collect(),
        id: required_string_property(&account, path, "Id")?,
        provider_type: required_string_property(&account, path, "ProviderType")?,
        provider_name: required_string_property(&account, path, "ProviderName")?,
        presentation_identity: required_string_property(&account, path, "PresentationIdentity")?,
        attention_needed: required_bool_property(&account, path, "AttentionNeeded")?,
        email_address: non_empty_string(required_string_property(&mail, path, "EmailAddress")?),
        imap_supported: required_bool_property(&mail, path, "ImapSupported")?,
        smtp_supported: required_bool_property(&mail, path, "SmtpSupported")?,
        imap_host: non_empty_string(required_string_property(&mail, path, "ImapHost")?),
        imap_user_name: non_empty_string(required_string_property(&mail, path, "ImapUserName")?),
        imap_use_ssl: required_bool_property(&mail, path, "ImapUseSsl")?,
        imap_use_tls: required_bool_property(&mail, path, "ImapUseTls")?,
        smtp_host: non_empty_string(required_string_property(&mail, path, "SmtpHost")?),
        smtp_user_name: non_empty_string(required_string_property(&mail, path, "SmtpUserName")?),
        smtp_use_auth: required_bool_property(&mail, path, "SmtpUseAuth")?,
        smtp_auth_xoauth2: bool_property_or(&mail, "SmtpAuthXoauth2", false),
        smtp_use_ssl: required_bool_property(&mail, path, "SmtpUseSsl")?,
        smtp_use_tls: required_bool_property(&mail, path, "SmtpUseTls")?,
    })
}

fn proxy<'a>(
    conn: &'a Connection,
    path: &'a str,
    interface: &'a str,
) -> Result<Proxy<'a>, GoaError> {
    Proxy::new(conn, GOA_SERVICE, path, interface).map_err(|err| {
        GoaError::ServiceUnavailable(format!(
            "could not create GOA proxy for {interface} at {path}: {err}"
        ))
    })
}

fn required_string_property(proxy: &Proxy<'_>, path: &str, name: &str) -> Result<String, GoaError> {
    proxy.get_property(name).map_err(|err| {
        GoaError::ServiceUnavailable(format!(
            "could not read GOA property {name} at {path}: {err}"
        ))
    })
}

fn required_bool_property(proxy: &Proxy<'_>, path: &str, name: &str) -> Result<bool, GoaError> {
    proxy.get_property(name).map_err(|err| {
        GoaError::ServiceUnavailable(format!(
            "could not read GOA property {name} at {path}: {err}"
        ))
    })
}

fn bool_property_or(proxy: &Proxy<'_>, name: &str, default: bool) -> bool {
    proxy.get_property(name).unwrap_or(default)
}

fn snapshot_to_mail_account(snapshot: GoaAccountSnapshot) -> GoaMailAccount {
    GoaMailAccount {
        id: snapshot.id,
        provider_type: snapshot.provider_type,
        provider_name: snapshot.provider_name,
        presentation_identity: snapshot.presentation_identity,
        email_address: snapshot.email_address,
        attention_needed: snapshot.attention_needed,
        imap_supported: snapshot.imap_supported,
        smtp_supported: snapshot.smtp_supported,
    }
}

fn ensure_credentials_for_snapshot(
    conn: &Connection,
    snapshot: &GoaAccountSnapshot,
) -> Result<(), GoaError> {
    let account = proxy(conn, &snapshot.path, GOA_ACCOUNT_IFACE)?;
    let ensure_result: zbus::Result<i32> = account.call("EnsureCredentials", &());
    match ensure_result {
        Ok(_) => Ok(()),
        Err(err) => {
            if required_bool_property(&account, &snapshot.path, "AttentionNeeded").unwrap_or(false)
            {
                Err(GoaError::AttentionNeeded(format!(
                    "GOA account `{}` needs user attention before credentials can be used",
                    snapshot.presentation_identity
                )))
            } else {
                Err(GoaError::Credentials(format!(
                    "could not refresh GOA credentials for `{}`: {err}",
                    snapshot.presentation_identity
                )))
            }
        }
    }
}

fn resolve_auth(
    conn: &Connection,
    snapshot: &GoaAccountSnapshot,
    target: AuthTarget,
) -> Result<EmailAuth, GoaError> {
    if snapshot.interfaces.contains(GOA_OAUTH2_IFACE) {
        if matches!(target, AuthTarget::Smtp) && !snapshot.smtp_auth_xoauth2 {
            return Err(GoaError::UnsupportedAuth(format!(
                "GOA account `{}` does not advertise XOAUTH2 for SMTP",
                snapshot.presentation_identity
            )));
        }
        let oauth = proxy(conn, &snapshot.path, GOA_OAUTH2_IFACE)?;
        let (token, _expires_in): (String, i32) =
            oauth.call("GetAccessToken", &()).map_err(|err| {
                GoaError::Credentials(format!(
                    "could not obtain an OAuth2 token for `{}`: {err}",
                    snapshot.presentation_identity
                ))
            })?;
        let token = token.trim().to_string();
        if token.is_empty() {
            return Err(GoaError::Credentials(format!(
                "GOA account `{}` returned an empty OAuth2 token",
                snapshot.presentation_identity
            )));
        }
        return Ok(EmailAuth::OAuth2(token));
    }

    if snapshot.interfaces.contains(GOA_PASSWORD_IFACE) {
        if matches!(target, AuthTarget::Smtp) && !snapshot.smtp_use_auth {
            return Err(GoaError::UnsupportedAuth(format!(
                "GOA account `{}` does not require authenticated SMTP, but aivi.email.smtpSend requires EmailAuth",
                snapshot.presentation_identity
            )));
        }
        let password_proxy = proxy(conn, &snapshot.path, GOA_PASSWORD_IFACE)?;
        let secret_id = match target {
            AuthTarget::Imap => "imap-password",
            AuthTarget::Smtp => "smtp-password",
        };
        let password: String =
            password_proxy
                .call("GetPassword", &(secret_id,))
                .map_err(|err| {
                    GoaError::Credentials(format!(
                        "could not obtain the {secret_id} secret for `{}`: {err}",
                        snapshot.presentation_identity
                    ))
                })?;
        if password.is_empty() {
            return Err(GoaError::Credentials(format!(
                "GOA account `{}` returned an empty password for {secret_id}",
                snapshot.presentation_identity
            )));
        }
        return Ok(EmailAuth::Password(password));
    }

    Err(GoaError::UnsupportedAuth(format!(
        "GOA account `{}` does not expose an OAuth2 or password credential interface",
        snapshot.presentation_identity
    )))
}

fn snapshot_to_imap_config(
    snapshot: &GoaAccountSnapshot,
    auth: EmailAuth,
) -> Result<GoaImapConfig, GoaError> {
    if !snapshot.imap_supported {
        return Err(GoaError::MailUnsupported(format!(
            "GOA account `{}` does not expose IMAP access",
            snapshot.presentation_identity
        )));
    }

    let host_raw = snapshot.imap_host.as_deref().ok_or_else(|| {
        GoaError::MailUnsupported(format!(
            "GOA account `{}` does not provide an IMAP host",
            snapshot.presentation_identity
        ))
    })?;
    let user = preferred_user(
        snapshot.imap_user_name.as_deref(),
        snapshot.email_address.as_deref(),
        Some(snapshot.presentation_identity.as_str()),
    )
    .ok_or_else(|| {
        GoaError::Credentials(format!(
            "GOA account `{}` did not provide an IMAP user name",
            snapshot.presentation_identity
        ))
    })?;
    let (host, port) = split_host_and_port(host_raw, if snapshot.imap_use_ssl { 993 } else { 143 });

    Ok(GoaImapConfig {
        host,
        user,
        auth,
        port: Some(port),
        starttls: Some(snapshot.imap_use_tls),
    })
}

fn snapshot_to_smtp_config(
    snapshot: &GoaAccountSnapshot,
    auth: EmailAuth,
) -> Result<GoaSmtpConfig, GoaError> {
    if !snapshot.smtp_supported {
        return Err(GoaError::MailUnsupported(format!(
            "GOA account `{}` does not expose SMTP access",
            snapshot.presentation_identity
        )));
    }

    let host_raw = snapshot.smtp_host.as_deref().ok_or_else(|| {
        GoaError::MailUnsupported(format!(
            "GOA account `{}` does not provide an SMTP host",
            snapshot.presentation_identity
        ))
    })?;
    let user = preferred_user(
        snapshot.smtp_user_name.as_deref(),
        snapshot.email_address.as_deref(),
        Some(snapshot.presentation_identity.as_str()),
    )
    .ok_or_else(|| {
        GoaError::Credentials(format!(
            "GOA account `{}` did not provide an SMTP user name",
            snapshot.presentation_identity
        ))
    })?;
    let from = preferred_user(
        snapshot.email_address.as_deref(),
        Some(snapshot.presentation_identity.as_str()),
        None,
    )
    .ok_or_else(|| {
        GoaError::Credentials(format!(
            "GOA account `{}` did not provide a sender identity",
            snapshot.presentation_identity
        ))
    })?;
    let (host, port) = split_host_and_port(host_raw, if snapshot.smtp_use_ssl { 465 } else { 587 });

    Ok(GoaSmtpConfig {
        host,
        user,
        auth,
        from,
        port: Some(port),
        starttls: Some(snapshot.smtp_use_tls),
    })
}

fn preferred_user(
    first: Option<&str>,
    second: Option<&str>,
    third: Option<&str>,
) -> Option<String> {
    [first, second, third]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn non_empty_string(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn split_host_and_port(raw: &str, default_port: i64) -> (String, i64) {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            let host = format!("[{}]", &rest[..end]);
            let suffix = &rest[end + 1..];
            if let Some(port_text) = suffix.strip_prefix(':') {
                if let Ok(port) = port_text.parse::<i64>() {
                    return (host, port);
                }
            }
            return (host, default_port);
        }
    }

    if raw.matches(':').count() == 1 {
        if let Some((host, port_text)) = raw.rsplit_once(':') {
            if let Ok(port) = port_text.parse::<i64>() {
                return (host.to_string(), port);
            }
        }
    }

    (raw.to_string(), default_port)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> GoaAccountSnapshot {
        GoaAccountSnapshot {
            path: "/org/gnome/OnlineAccounts/Accounts/1".to_string(),
            interfaces: BTreeSet::from([
                GOA_ACCOUNT_IFACE.to_string(),
                GOA_MAIL_IFACE.to_string(),
                GOA_OAUTH2_IFACE.to_string(),
            ]),
            id: "account-1".to_string(),
            provider_type: "google".to_string(),
            provider_name: "Google".to_string(),
            presentation_identity: "ada@example.com".to_string(),
            attention_needed: false,
            email_address: Some("ada@example.com".to_string()),
            imap_supported: true,
            smtp_supported: true,
            imap_host: Some("imap.example.com".to_string()),
            imap_user_name: None,
            imap_use_ssl: true,
            imap_use_tls: false,
            smtp_host: Some("smtp.example.com:587".to_string()),
            smtp_user_name: Some("smtp-user".to_string()),
            smtp_use_auth: true,
            smtp_auth_xoauth2: true,
            smtp_use_ssl: false,
            smtp_use_tls: true,
        }
    }

    #[test]
    fn split_host_and_port_extracts_numeric_suffix() {
        assert_eq!(
            split_host_and_port("smtp.example.com:2525", 587),
            ("smtp.example.com".to_string(), 2525)
        );
    }

    #[test]
    fn split_host_and_port_keeps_ipv6_literal() {
        assert_eq!(
            split_host_and_port("[2001:db8::1]:143", 993),
            ("[2001:db8::1]".to_string(), 143)
        );
    }

    #[test]
    fn snapshot_to_mail_account_preserves_metadata() {
        let snapshot = snapshot();
        let account = snapshot_to_mail_account(snapshot);
        assert_eq!(account.provider_name, "Google");
        assert_eq!(account.email_address.as_deref(), Some("ada@example.com"));
        assert!(account.imap_supported);
        assert!(account.smtp_supported);
    }

    #[test]
    fn snapshot_to_imap_config_uses_email_as_username_fallback() {
        let config =
            snapshot_to_imap_config(&snapshot(), EmailAuth::OAuth2("token".to_string())).unwrap();
        assert_eq!(config.host, "imap.example.com");
        assert_eq!(config.user, "ada@example.com");
        assert_eq!(config.port, Some(993));
        assert_eq!(config.starttls, Some(false));
        assert!(matches!(config.auth, EmailAuth::OAuth2(_)));
    }

    #[test]
    fn snapshot_to_smtp_config_requires_supported_transport() {
        let mut snapshot = snapshot();
        snapshot.smtp_supported = false;
        let err = snapshot_to_smtp_config(&snapshot, EmailAuth::Password("pw".to_string()))
            .expect_err("smtp should be rejected");
        assert!(matches!(err, GoaError::MailUnsupported(_)));
    }

    #[test]
    fn preferred_user_skips_blank_values() {
        assert_eq!(
            preferred_user(Some(""), Some("  "), Some("ada@example.com")),
            Some("ada@example.com".to_string())
        );
    }
}
