//! Read-only smoke tool. Reads two secret lines from piped stdin, never arguments.
use std::{io, sync::Mutex};

use aster_sms::{
    SecretReference, SecretValue, SmsCredentialResolver, SmsError, SmsReason, SmsbaoConfig,
    SmsbaoProvider,
};
use async_trait::async_trait;

struct StdinSecrets {
    username_reference: String,
    credential_reference: String,
    username: Mutex<Option<SecretValue>>,
    credential: Mutex<Option<SecretValue>>,
}
#[async_trait]
impl SmsCredentialResolver for StdinSecrets {
    async fn resolve(&self, reference: &SecretReference) -> Result<SecretValue, SmsError> {
        let slot = if reference.as_str() == self.username_reference {
            &self.username
        } else if reference.as_str() == self.credential_reference {
            &self.credential
        } else {
            return Err(refused());
        };
        slot.lock()
            .map_err(|_| refused())?
            .take()
            .ok_or_else(refused)
    }
}
fn refused() -> SmsError {
    SmsError::Refused {
        reason: SmsReason::AuthenticationFailed,
    }
}
fn read_secret() -> Result<SecretValue, Box<dyn std::error::Error>> {
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    while value.ends_with(['\r', '\n']) {
        value.pop();
    }
    Ok(SecretValue::new(value)?)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = std::env::args().nth(1) else {
        return Err("supply an ACL profile path; secrets must arrive on stdin".into());
    };
    let source = std::fs::read_to_string(path)?;
    let config = SmsbaoConfig::from_acl(&source)?;
    let document = aster_acl::parse(&source).map_err(|_| "invalid ACL profile")?;
    // Configuration values are validated by SmsbaoConfig; these are reference IDs only.
    let block = document.blocks.first().ok_or("missing provider block")?;
    let reference = |field: &str| -> Result<String, Box<dyn std::error::Error>> {
        Ok(block
            .attributes
            .get(field)
            .and_then(aster_acl::Value::as_str)
            .ok_or("missing secret reference")?
            .into())
    };
    let secrets = StdinSecrets {
        username_reference: reference("username_secret_ref")?,
        credential_reference: reference("credential_secret_ref")?,
        username: Mutex::new(Some(read_secret()?)),
        credential: Mutex::new(Some(read_secret()?)),
    };
    let provider = SmsbaoProvider::from_config(config, &secrets).await?;
    let _balance = provider.query_balance().await?;
    println!("authentication=ok; balance_query=ok; sent_messages=0");
    Ok(())
}
