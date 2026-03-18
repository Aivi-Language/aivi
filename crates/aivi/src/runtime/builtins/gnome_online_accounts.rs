use std::collections::HashMap;
use std::sync::Arc;

use aivi_email::EmailAuth;
use aivi_goa::{GoaError, GoaImapConfig, GoaMailAccount, GoaSmtpConfig};

use super::util::{builtin, expect_text, make_none, make_some};
use crate::runtime::{EffectValue, RuntimeError, Value};

pub(super) fn build_gnome_online_accounts_record() -> Value {
    let mut fields = HashMap::new();

    fields.insert(
        "listMailAccounts".to_string(),
        builtin("gnomeOnlineAccounts.listMailAccounts", 1, |mut args, _| {
            let _unit = args.remove(0);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let accounts = aivi_goa::list_mail_accounts().map_err(goa_err_to_runtime)?;
                    Ok(Value::List(Arc::new(
                        accounts
                            .into_iter()
                            .map(goa_mail_account_to_value)
                            .collect(),
                    )))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "ensureCredentials".to_string(),
        builtin("gnomeOnlineAccounts.ensureCredentials", 1, |mut args, _| {
            let account_id = expect_text(args.remove(0), "gnomeOnlineAccounts.ensureCredentials")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    aivi_goa::ensure_credentials(&account_id).map_err(goa_err_to_runtime)?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapConfig".to_string(),
        builtin("gnomeOnlineAccounts.imapConfig", 1, |mut args, _| {
            let account_id = expect_text(args.remove(0), "gnomeOnlineAccounts.imapConfig")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let config = aivi_goa::imap_config(&account_id).map_err(goa_err_to_runtime)?;
                    Ok(goa_imap_config_to_value(config))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "smtpConfig".to_string(),
        builtin("gnomeOnlineAccounts.smtpConfig", 1, |mut args, _| {
            let account_id = expect_text(args.remove(0), "gnomeOnlineAccounts.smtpConfig")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let config = aivi_goa::smtp_config(&account_id).map_err(goa_err_to_runtime)?;
                    Ok(goa_smtp_config_to_value(config))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    Value::Record(Arc::new(fields))
}

fn goa_err_to_runtime(err: GoaError) -> RuntimeError {
    RuntimeError::Error(goa_error_to_value(err))
}

fn goa_error_to_value(err: GoaError) -> Value {
    match err {
        GoaError::PlatformUnsupported => Value::Constructor {
            name: "PlatformUnsupported".to_string(),
            args: vec![],
        },
        GoaError::ServiceUnavailable(message) => unary_constructor("ServiceUnavailable", message),
        GoaError::AttentionNeeded(message) => unary_constructor("AttentionNeeded", message),
        GoaError::AccountNotFound(message) => unary_constructor("AccountNotFound", message),
        GoaError::MailUnsupported(message) => unary_constructor("MailUnsupported", message),
        GoaError::UnsupportedAuth(message) => unary_constructor("UnsupportedAuth", message),
        GoaError::Credentials(message) => unary_constructor("Credentials", message),
    }
}

fn unary_constructor(name: &str, message: String) -> Value {
    Value::Constructor {
        name: name.to_string(),
        args: vec![Value::Text(message)],
    }
}

fn goa_mail_account_to_value(account: GoaMailAccount) -> Value {
    let mut fields = HashMap::new();
    fields.insert("id".to_string(), Value::Text(account.id));
    fields.insert(
        "providerType".to_string(),
        Value::Text(account.provider_type),
    );
    fields.insert(
        "providerName".to_string(),
        Value::Text(account.provider_name),
    );
    fields.insert(
        "presentationIdentity".to_string(),
        Value::Text(account.presentation_identity),
    );
    fields.insert(
        "emailAddress".to_string(),
        account
            .email_address
            .map(Value::Text)
            .map(make_some)
            .unwrap_or_else(make_none),
    );
    fields.insert(
        "attentionNeeded".to_string(),
        Value::Bool(account.attention_needed),
    );
    fields.insert(
        "imapSupported".to_string(),
        Value::Bool(account.imap_supported),
    );
    fields.insert(
        "smtpSupported".to_string(),
        Value::Bool(account.smtp_supported),
    );
    Value::Record(Arc::new(fields))
}

fn goa_imap_config_to_value(config: GoaImapConfig) -> Value {
    let mut fields = HashMap::new();
    fields.insert("host".to_string(), Value::Text(config.host));
    fields.insert("user".to_string(), Value::Text(config.user));
    fields.insert("auth".to_string(), email_auth_to_value(config.auth));
    fields.insert(
        "port".to_string(),
        config
            .port
            .map(|value| make_some(Value::Int(value)))
            .unwrap_or_else(make_none),
    );
    fields.insert(
        "starttls".to_string(),
        config
            .starttls
            .map(|value| make_some(Value::Bool(value)))
            .unwrap_or_else(make_none),
    );
    Value::Record(Arc::new(fields))
}

fn goa_smtp_config_to_value(config: GoaSmtpConfig) -> Value {
    let mut fields = HashMap::new();
    fields.insert("host".to_string(), Value::Text(config.host));
    fields.insert("user".to_string(), Value::Text(config.user));
    fields.insert("auth".to_string(), email_auth_to_value(config.auth));
    fields.insert("from".to_string(), Value::Text(config.from));
    fields.insert(
        "port".to_string(),
        config
            .port
            .map(|value| make_some(Value::Int(value)))
            .unwrap_or_else(make_none),
    );
    fields.insert(
        "starttls".to_string(),
        config
            .starttls
            .map(|value| make_some(Value::Bool(value)))
            .unwrap_or_else(make_none),
    );
    Value::Record(Arc::new(fields))
}

fn email_auth_to_value(auth: EmailAuth) -> Value {
    match auth {
        EmailAuth::Password(password) => Value::Constructor {
            name: "Password".to_string(),
            args: vec![Value::Text(password)],
        },
        EmailAuth::OAuth2(token) => Value::Constructor {
            name: "OAuth2".to_string(),
            args: vec![Value::Text(token)],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goa_error_value_preserves_variant_names() {
        let value = goa_error_to_value(GoaError::AttentionNeeded("reauth".to_string()));
        assert!(matches!(
            value,
            Value::Constructor { ref name, ref args }
            if name == "AttentionNeeded"
                && matches!(args.as_slice(), [Value::Text(text)] if text == "reauth")
        ));
    }

    #[test]
    fn goa_smtp_config_value_uses_email_auth_shape() {
        let value = goa_smtp_config_to_value(GoaSmtpConfig {
            host: "smtp.example.com".to_string(),
            user: "ada@example.com".to_string(),
            auth: EmailAuth::OAuth2("token".to_string()),
            from: "ada@example.com".to_string(),
            port: Some(587),
            starttls: Some(true),
        });

        let Value::Record(fields) = value else {
            panic!("expected record");
        };
        assert!(matches!(
            fields.get("auth"),
            Some(Value::Constructor { name, .. }) if name == "OAuth2"
        ));
        assert!(matches!(
            fields.get("starttls"),
            Some(Value::Constructor { name, .. }) if name == "Some"
        ));
    }
}
