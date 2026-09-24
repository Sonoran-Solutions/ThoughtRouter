//! API key storage abstraction. The desktop app implements this with the OS
//! keychain; the `OPENROUTER_API_KEY` environment variable overrides it.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::processor::mock::{MockAnalyzer, MockEmbedder};
use crate::processor::openrouter::{OpenRouter, OpenRouterConfig};
use crate::processor::{Embedder, Processors};
use crate::settings::{ProviderKind, Settings};

pub const ENV_API_KEY: &str = "OPENROUTER_API_KEY";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum KeySource {
    Env,
    Keychain,
    None,
}

pub trait SecretStore: Send + Sync {
    fn api_key(&self) -> Result<Option<String>>;
    fn set_api_key(&self, key: &str) -> Result<()>;
    fn clear_api_key(&self) -> Result<()>;
    fn key_source(&self) -> KeySource;
}

/// In-memory store for tests and headless use.
#[derive(Default)]
pub struct MemorySecrets(Mutex<Option<String>>);

impl SecretStore for MemorySecrets {
    fn api_key(&self) -> Result<Option<String>> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn set_api_key(&self, key: &str) -> Result<()> {
        *self.0.lock().unwrap() = Some(key.to_string());
        Ok(())
    }
    fn clear_api_key(&self) -> Result<()> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
    fn key_source(&self) -> KeySource {
        if self.0.lock().unwrap().is_some() {
            KeySource::Keychain
        } else {
            KeySource::None
        }
    }
}

/// Builds processors from settings, or explains why processing is blocked.
pub fn build_processors(
    settings: &Settings,
    secrets: &dyn SecretStore,
) -> Result<Processors, String> {
    if !settings.processing_enabled {
        return Err("Processing is paused in Settings.".into());
    }
    match settings.provider {
        ProviderKind::Mock => Ok(Processors {
            analyzer: Arc::new(MockAnalyzer),
            embedder: Some(Arc::new(MockEmbedder)),
        }),
        ProviderKind::Openrouter => {
            let key = secrets
                .api_key()
                .map_err(|e| format!("Could not read the API key ({e:#}). Set OPENROUTER_API_KEY in the environment instead."))?
                .filter(|k| !k.trim().is_empty())
                .ok_or("Add your OpenRouter API key in Settings to process captures.")?;
            if settings.analyzer_model.trim().is_empty() {
                return Err("Choose an analyzer model in Settings to process captures.".into());
            }
            let or = Arc::new(
                OpenRouter::new(OpenRouterConfig::new(
                    key,
                    settings.analyzer_model.trim().into(),
                    settings.embedding_model.trim().into(),
                    settings.openrouter_zdr,
                ))
                .map_err(|e| e.to_string())?,
            );
            let embedder: Option<Arc<dyn Embedder>> = if settings.embedding_model.trim().is_empty()
            {
                None
            } else {
                Some(or.clone())
            };
            Ok(Processors {
                analyzer: or,
                embedder,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_missing_key_and_model() {
        let secrets = MemorySecrets::default();
        let mut s = Settings::default();
        let err = build_processors(&s, &secrets).err().unwrap();
        assert!(err.contains("API key"));
        secrets.set_api_key("sk-x").unwrap();
        let err = build_processors(&s, &secrets).err().unwrap();
        assert!(err.contains("analyzer model"));
        s.analyzer_model = "vendor/model".into();
        let p = build_processors(&s, &secrets).unwrap();
        assert!(p.embedder.is_none(), "no embedding model ⇒ lexical only");
        s.processing_enabled = false;
        assert!(
            build_processors(&s, &secrets)
                .err()
                .unwrap()
                .contains("paused")
        );
    }
}

/// Settings plus derived, read-only facts for the Settings screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SettingsView {
    pub settings: Settings,
    pub api_key_source: KeySource,
    /// Where embeddings are stored for the current settings.
    pub embedding_model_id: Option<String>,
    pub data_dir: String,
}
