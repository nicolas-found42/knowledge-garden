//! Shared Ollama Cloud adapter for generative helper work (ADR 0006). Jev stays primary for
//! choices, scores and yes/no judgments; this client only carries helper generation such as image
//! description. Rust-only: credentials, request bodies and image bytes never enter logs or errors.
use crate::semantic::ProviderError;
use base64::Engine;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::time::Duration;

pub const OLLAMA_CLOUD_ENDPOINT: &str = "https://ollama.com/api";
/// Exact model. The adapter never substitutes another model.
pub const OLLAMA_CLOUD_MODEL: &str = "deepseek-v4.1-flash";

#[derive(Debug, Clone)]
pub struct OllamaImage {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OllamaThink {
    Off,
    Low,
    High,
    Max,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OllamaReply {
    pub content: String,
    pub model: String,
}

/// Where the API key comes from. Read lazily on each request, never at construction.
#[derive(Clone)]
pub struct CredentialSource {
    fixed: Option<String>,
}

impl CredentialSource {
    pub fn fixed(secret: &str) -> Self {
        Self {
            fixed: Some(secret.to_owned()),
        }
    }

    fn resolve(&self) -> Result<String, ProviderError> {
        self.fixed.clone().ok_or_else(|| {
            ProviderError::recoverable("Ollama Cloud is waiting for an API key.".into())
        })
    }
}

pub struct OllamaCloudClient {
    endpoint: String,
    credentials: CredentialSource,
    think: OllamaThink,
    http: Client,
    retry_backoff: Duration,
}

impl OllamaCloudClient {
    pub fn with_endpoint(endpoint: &str, credentials: CredentialSource) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_owned(),
            credentials,
            think: OllamaThink::Off,
            http: Client::builder()
                .timeout(Duration::from_secs(45))
                .build()
                .unwrap_or_else(|_| Client::new()),
            retry_backoff: Duration::from_millis(500),
        }
    }

    pub fn with_think(mut self, think: OllamaThink) -> Self {
        self.think = think;
        self
    }

    pub fn with_retry_backoff(mut self, backoff: Duration) -> Self {
        self.retry_backoff = backoff;
        self
    }

    pub fn chat(
        &self,
        system: &str,
        user: &str,
        images: &[OllamaImage],
    ) -> Result<OllamaReply, ProviderError> {
        let key = self.credentials.resolve()?;
        let mut user_message = json!({"role": "user", "content": user});
        if !images.is_empty() {
            user_message["images"] = Value::Array(
                images
                    .iter()
                    .map(|image| {
                        Value::String(
                            base64::engine::general_purpose::STANDARD.encode(&image.bytes),
                        )
                    })
                    .collect(),
            );
        }
        let think = match self.think {
            OllamaThink::Off => json!(false),
            OllamaThink::Low => json!("low"),
            OllamaThink::High => json!("high"),
            OllamaThink::Max => json!("max"),
        };
        let request = json!({
            "model": OLLAMA_CLOUD_MODEL,
            "stream": false,
            "think": think,
            "messages": [{"role": "system", "content": system}, user_message],
        });
        let response = self
            .http
            .post(format!("{}/chat", self.endpoint))
            .bearer_auth(key)
            .json(&request)
            .send()
            .map_err(|_| {
                ProviderError::recoverable(
                    "Ollama Cloud could not be reached; work will retry automatically.".into(),
                )
            })?;
        let body: Value = response.json().map_err(|_| {
            ProviderError::recoverable("Ollama Cloud returned a malformed response.".into())
        })?;
        Ok(OllamaReply {
            content: body["message"]["content"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            model: body["model"].as_str().unwrap_or_default().to_owned(),
        })
    }
}
