//! OpenRouter adapter (D-020): OpenAI-compatible chat completions with strict
//! JSON-schema output, and the embeddings endpoint.

use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::*;
use crate::models::ThreadSynthesis;

pub const PROVIDER: &str = "openrouter";
pub const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";
const EMBED_BATCH: usize = 64;

const ANALYZE_PROMPT: &str = include_str!("../../prompts/analyze_capture.v1.md");
const ANALYZE_SCHEMA: &str = include_str!("../../prompts/analyze_capture.v1.schema.json");
const LINK_PROMPT: &str = include_str!("../../prompts/link_projects.v1.md");
const LINK_SCHEMA: &str = include_str!("../../prompts/link_projects.v1.schema.json");
const SYNTH_PROMPT: &str = include_str!("../../prompts/synthesize_thread.v1.md");
const SYNTH_SCHEMA: &str = include_str!("../../prompts/synthesize_thread.v1.schema.json");

#[derive(Debug, Clone)]
pub struct OpenRouterConfig {
    pub base_url: String,
    pub api_key: String,
    pub analyzer_model: String,
    pub embedding_model: String,
    /// Only route to zero-data-retention endpoints.
    pub zdr: bool,
    pub timeout: Duration,
}

impl OpenRouterConfig {
    pub fn new(
        api_key: String,
        analyzer_model: String,
        embedding_model: String,
        zdr: bool,
    ) -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            api_key,
            analyzer_model,
            embedding_model,
            zdr,
            timeout: Duration::from_secs(90),
        }
    }
}

pub struct OpenRouter {
    http: reqwest::Client,
    cfg: OpenRouterConfig,
}

impl OpenRouter {
    pub fn new(cfg: OpenRouterConfig) -> Result<Self> {
        let mut builder = reqwest::Client::builder().timeout(cfg.timeout);
        if cfg.base_url.contains("://127.0.0.1") || cfg.base_url.contains("://localhost") {
            builder = builder.no_proxy();
        }
        Ok(Self {
            http: builder.build()?,
            cfg,
        })
    }

    /// Routing preferences: only providers that honour every request
    /// parameter (so the schema is enforced), never ones that train on or
    /// retain prompts.
    fn provider_prefs(&self) -> Value {
        let mut p = json!({ "require_parameters": true, "data_collection": "deny" });
        if self.cfg.zdr {
            p["zdr"] = json!(true);
        }
        p
    }

    async fn post(&self, path: &str, body: &Value) -> Result<Value> {
        let url = format!("{}{}", self.cfg.base_url.trim_end_matches('/'), path);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.cfg.api_key)
            .header(
                "HTTP-Referer",
                "https://github.com/Sonoran-Solutions/ThoughtRouter",
            )
            .header("X-Title", "ThoughtRouter")
            .json(body)
            .send()
            .await
            .with_context(|| format!("request to OpenRouter {path} failed"))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            let detail = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
                .unwrap_or_else(|| text.chars().take(300).collect());
            let hint = match status.as_u16() {
                401 => " (check the OpenRouter API key in Settings)",
                402 => " (OpenRouter account is out of credits)",
                404 => " (check the model id in Settings, and that it supports structured outputs)",
                429 => " (rate limited; will retry)",
                _ => "",
            };
            bail!("OpenRouter {path} returned {status}{hint}: {detail}");
        }
        let v: Value = serde_json::from_str(&text).context("OpenRouter returned invalid JSON")?;
        if let Some(msg) = v["error"]["message"].as_str() {
            bail!("OpenRouter error: {msg}");
        }
        Ok(v)
    }

    async fn chat_json<T: DeserializeOwned>(
        &self,
        schema_name: &str,
        schema: &str,
        system: &str,
        user: String,
    ) -> Result<(T, String)> {
        if self.cfg.analyzer_model.trim().is_empty() {
            bail!("no analyzer model configured");
        }
        let schema: Value = serde_json::from_str(schema).expect("bundled schema is valid JSON");
        let mut messages = vec![
            json!({ "role": "system", "content": system }),
            json!({ "role": "user", "content": user }),
        ];
        for attempt in 0..2 {
            let body = json!({
                "model": self.cfg.analyzer_model,
                "messages": messages,
                "temperature": 0.2,
                "response_format": {
                    "type": "json_schema",
                    "json_schema": { "name": schema_name, "strict": true, "schema": schema }
                },
                "provider": self.provider_prefs(),
            });
            let resp = self.post("/chat/completions", &body).await?;
            let served = resp["model"]
                .as_str()
                .unwrap_or(&self.cfg.analyzer_model)
                .to_string();
            let message = &resp["choices"][0]["message"];
            let content = message["content"].as_str().ok_or_else(|| {
                let refusal = message["refusal"].as_str().unwrap_or("no content");
                anyhow!("model returned no JSON content ({refusal})")
            })?;
            // C6/AI_PIPELINE: never trust routing blindly; validate locally.
            match serde_json::from_str::<T>(strip_code_fence(content)) {
                Ok(v) => return Ok((v, served)),
                Err(e) if attempt == 0 => {
                    messages.push(json!({ "role": "assistant", "content": content }));
                    messages.push(json!({
                        "role": "user",
                        "content": format!(
                            "That response did not match the required JSON schema ({e}). \
                             Reply again with only valid JSON that matches the schema."
                        )
                    }));
                }
                Err(e) => bail!("model output did not match schema after a repair attempt: {e}"),
            }
        }
        unreachable!()
    }

    fn meta(&self, served: String, prompt: &str) -> RunMeta {
        RunMeta {
            provider: PROVIDER.into(),
            model: served,
            prompt_version: Some(prompt.into()),
            schema_version: Some(SCHEMA_VERSION.into()),
        }
    }
}

fn strip_code_fence(s: &str) -> &str {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("```") {
        let rest = rest.strip_prefix("json").unwrap_or(rest);
        return rest.strip_suffix("```").unwrap_or(rest).trim();
    }
    t
}

pub fn analyze_user_message(input: &AnalyzeInput) -> String {
    let projects = if input.known_projects.is_empty() {
        "(none yet)".to_string()
    } else {
        input.known_projects.join("; ")
    };
    format!(
        "Known project names (spelling reference only): {projects}\n\n\
         Capture (verbatim, between the markers):\n<<<CAPTURE\n{}\nCAPTURE>>>",
        input.text
    )
}

#[async_trait]
impl Analyzer for OpenRouter {
    fn provider(&self) -> &str {
        PROVIDER
    }

    async fn analyze_capture(&self, input: &AnalyzeInput) -> Result<(CaptureAnalysis, RunMeta)> {
        let (analysis, served) = self
            .chat_json::<CaptureAnalysis>(
                "capture_analysis",
                ANALYZE_SCHEMA,
                ANALYZE_PROMPT,
                analyze_user_message(input),
            )
            .await?;
        Ok((analysis, self.meta(served, ANALYZE_PROMPT_VERSION)))
    }

    async fn link_projects(&self, input: &LinkInput) -> Result<(LinkOutput, RunMeta)> {
        let rejected: Vec<Value> = input
            .rejected
            .iter()
            .map(|(a, p)| json!({ "atom": a, "project": p }))
            .collect();
        let user = serde_json::to_string_pretty(&json!({
            "atoms": input.atoms,
            "candidates": input.candidates,
            "rejected": rejected,
        }))?;
        let (out, served) = self
            .chat_json::<LinkOutput>("project_links", LINK_SCHEMA, LINK_PROMPT, user)
            .await?;
        Ok((out, self.meta(served, LINK_PROMPT_VERSION)))
    }

    async fn synthesize(&self, input: &SynthesisInput) -> Result<(ThreadSynthesis, RunMeta)> {
        let mut user = format!(
            "Project: {}\nDescription: {}\n\nNotes, oldest first:\n",
            input.project_name, input.project_description
        );
        for e in &input.entries {
            user.push_str(&format!("\n[{}]\n{}\n", e.captured_at, e.text));
        }
        let (out, served) = self
            .chat_json::<ThreadSynthesis>("thread_synthesis", SYNTH_SCHEMA, SYNTH_PROMPT, user)
            .await?;
        Ok((out, self.meta(served, SYNTHESIZE_PROMPT_VERSION)))
    }
}

#[async_trait]
impl Embedder for OpenRouter {
    fn model_id(&self) -> String {
        format!("{PROVIDER}:{}", self.cfg.embedding_model)
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if self.cfg.embedding_model.trim().is_empty() {
            bail!("no embedding model configured");
        }
        let mut out = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(EMBED_BATCH) {
            let body = json!({ "model": self.cfg.embedding_model, "input": chunk });
            let resp = self.post("/embeddings", &body).await?;
            let mut data: Vec<(usize, Vec<f32>)> = resp["data"]
                .as_array()
                .ok_or_else(|| anyhow!("embeddings response has no data"))?
                .iter()
                .enumerate()
                .map(|(i, d)| {
                    let idx = d["index"].as_u64().map(|x| x as usize).unwrap_or(i);
                    let v = d["embedding"]
                        .as_array()
                        .ok_or_else(|| anyhow!("embedding is not an array"))?
                        .iter()
                        .map(|x| {
                            x.as_f64()
                                .map(|f| f as f32)
                                .ok_or_else(|| anyhow!("non-numeric embedding"))
                        })
                        .collect::<Result<Vec<f32>>>()?;
                    Ok((idx, v))
                })
                .collect::<Result<_>>()?;
            if data.len() != chunk.len() {
                bail!("expected {} embeddings, got {}", chunk.len(), data.len());
            }
            data.sort_by_key(|(i, _)| *i);
            out.extend(data.into_iter().map(|(_, v)| v));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AtomType;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(server: &MockServer, zdr: bool) -> OpenRouter {
        let mut cfg = OpenRouterConfig::new(
            "sk-test".into(),
            "vendor/model".into(),
            "vendor/embed".into(),
            zdr,
        );
        cfg.base_url = server.uri();
        OpenRouter::new(cfg).unwrap()
    }

    fn completion(content: &str) -> Value {
        json!({ "model": "vendor/model-2026", "choices": [{ "message": { "role": "assistant", "content": content } }] })
    }

    #[test]
    fn bundled_schemas_are_strict_objects() {
        for s in [ANALYZE_SCHEMA, LINK_SCHEMA, SYNTH_SCHEMA] {
            let v: Value = serde_json::from_str(s).unwrap();
            assert_eq!(v["additionalProperties"], json!(false));
            // Strict mode requires every property to be listed as required.
            let props = v["properties"].as_object().unwrap().len();
            assert_eq!(v["required"].as_array().unwrap().len(), props);
        }
    }

    #[tokio::test]
    async fn analyze_sends_strict_schema_and_privacy_routing() {
        let server = MockServer::start().await;
        let content = json!({
            "atoms": [{ "text": "Fix SSD", "type": "problem", "confidence": 0.9, "quote": "SSD" }],
            "needs_clarification": false, "notes": []
        })
        .to_string();
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("authorization", "Bearer sk-test"))
            .and(body_partial_json(json!({
                "model": "vendor/model",
                "response_format": { "type": "json_schema", "json_schema": { "name": "capture_analysis", "strict": true } },
                "provider": { "require_parameters": true, "data_collection": "deny", "zdr": true }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(completion(&content)))
            .expect(1)
            .mount(&server)
            .await;
        let or = client(&server, true);
        let (a, meta) = or
            .analyze_capture(&AnalyzeInput {
                text: "SSD".into(),
                known_projects: vec![],
            })
            .await
            .unwrap();
        assert_eq!(a.atoms[0].atom_type, AtomType::Problem);
        assert_eq!(
            meta.model, "vendor/model-2026",
            "records the model actually served"
        );
        assert_eq!(meta.prompt_version.as_deref(), Some(ANALYZE_PROMPT_VERSION));
    }

    #[tokio::test]
    async fn invalid_output_gets_one_repair_attempt() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(completion("{\"atoms\": \"nope\"}")),
            )
            .expect(2)
            .mount(&server)
            .await;
        let err = client(&server, false)
            .analyze_capture(&AnalyzeInput {
                text: "x".into(),
                known_projects: vec![],
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("repair"), "{err}");
    }

    #[tokio::test]
    async fn http_errors_carry_actionable_hints() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_json(json!({ "error": { "message": "No auth" } })),
            )
            .mount(&server)
            .await;
        let err = client(&server, false)
            .analyze_capture(&AnalyzeInput {
                text: "x".into(),
                known_projects: vec![],
            })
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("API key") && err.contains("No auth"), "{err}");
    }

    #[tokio::test]
    async fn embeddings_are_reordered_by_index() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .and(body_partial_json(json!({ "model": "vendor/embed" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [ { "index": 1, "embedding": [0.0, 1.0] }, { "index": 0, "embedding": [1.0, 0.0] } ]
            })))
            .mount(&server)
            .await;
        let v = client(&server, false)
            .embed(&["a".into(), "b".into()])
            .await
            .unwrap();
        assert_eq!(v, vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
    }

    #[test]
    fn strips_markdown_fences() {
        assert_eq!(strip_code_fence("```json\n{}\n```"), "{}");
        assert_eq!(strip_code_fence(" {} "), "{}");
    }
}
