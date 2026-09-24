//! OpenRouter API key in the OS credential store (macOS Keychain, Windows
//! Credential Manager, Secret Service on Linux). `OPENROUTER_API_KEY` in the
//! environment takes precedence, which is handy for development.

use std::sync::Mutex;

use anyhow::{Context, Result};
use thoughtrouter_core::secrets::{ENV_API_KEY, KeySource, SecretStore};

const SERVICE: &str = "com.sonoransolutions.thoughtrouter";
const ACCOUNT: &str = "openrouter-api-key";

#[derive(Default)]
pub struct KeychainSecrets {
    /// Avoids hitting the keychain on every worker tick.
    cache: Mutex<Option<Option<String>>>,
}

fn env_key() -> Option<String> {
    std::env::var(ENV_API_KEY)
        .ok()
        .filter(|k| !k.trim().is_empty())
}

fn entry() -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, ACCOUNT).context("OS keychain unavailable")
}

impl SecretStore for KeychainSecrets {
    fn api_key(&self) -> Result<Option<String>> {
        if let Some(k) = env_key() {
            return Ok(Some(k));
        }
        let mut cache = self.cache.lock().unwrap();
        if let Some(v) = cache.as_ref() {
            return Ok(v.clone());
        }
        let v = match entry()?.get_password() {
            Ok(k) => Some(k),
            Err(keyring::Error::NoEntry) => None,
            Err(e) => {
                return Err(anyhow::anyhow!(e)).context("the OS keychain is not available");
            }
        };
        *cache = Some(v.clone());
        Ok(v)
    }

    fn set_api_key(&self, key: &str) -> Result<()> {
        entry()?
            .set_password(key.trim())
            .context("saving API key to OS keychain (you can set OPENROUTER_API_KEY instead)")?;
        *self.cache.lock().unwrap() = Some(Some(key.trim().to_string()));
        Ok(())
    }

    fn clear_api_key(&self) -> Result<()> {
        match entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(e).context("removing API key from OS keychain"),
        }
        *self.cache.lock().unwrap() = Some(None);
        Ok(())
    }

    fn key_source(&self) -> KeySource {
        if env_key().is_some() {
            KeySource::Env
        } else if matches!(self.api_key(), Ok(Some(_))) {
            KeySource::Keychain
        } else {
            KeySource::None
        }
    }
}
