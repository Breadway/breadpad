use crate::config::OllamaConfig;
use crate::types::{ClassificationResult, NoteType};
use serde::Deserialize;

pub struct OllamaClient {
    endpoint: String,
    model: String,
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
    #[allow(dead_code)]
    done: bool,
}

#[derive(Deserialize)]
struct OllamaClassification {
    #[serde(rename = "type")]
    note_type: Option<String>,
    body: Option<String>,
    confidence: Option<f32>,
}

impl OllamaClient {
    pub fn new(cfg: &OllamaConfig) -> Self {
        OllamaClient {
            endpoint: cfg.endpoint.trim_end_matches('/').to_string(),
            model: cfg.model.clone(),
        }
    }

    /// Run Tier 3 classification. Returns `fallback` if Ollama is unreachable or returns
    /// an unparseable response. Time, rrule, and body from Tier 1 are always preserved.
    pub fn classify(&self, text: &str, fallback: &ClassificationResult) -> ClassificationResult {
        match self.try_classify(text, fallback) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Tier 3 (Ollama) unavailable: {}; using Tier 2 result", e);
                fallback.clone()
            }
        }
    }

    fn try_classify(&self, text: &str, fallback: &ClassificationResult) -> anyhow::Result<ClassificationResult> {
        let url = format!("{}/api/generate", self.endpoint);

        let prompt = format!(
            "Classify the following note into exactly one type.\n\
             Valid types: todo, reminder, idea, note, question.\n\
             Note: \"{}\"\n\
             Respond with JSON only, using this exact format: \
             {{\"type\": \"TYPENAME\", \"body\": \"cleaned text\", \"confidence\": 0.0}}",
            text
        );

        let payload = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "format": "json",
            "stream": false
        });

        let response = ureq::post(&url)
            .set("Content-Type", "application/json")
            .send_json(payload)
            .map_err(|e| anyhow::anyhow!("Ollama HTTP error: {}", e))?;

        let ollama_resp: OllamaGenerateResponse = response
            .into_json()
            .map_err(|e| anyhow::anyhow!("deserialize Ollama envelope: {}", e))?;

        let classification: OllamaClassification = extract_json(&ollama_resp.response)
            .ok_or_else(|| anyhow::anyhow!(
                "no JSON object found in response — raw: {:?}",
                &ollama_resp.response
            ))?;

        let note_type = classification
            .note_type
            .as_deref()
            .map(NoteType::from_str)
            .unwrap_or_else(|| fallback.note_type.clone());

        let confidence = classification
            .confidence
            .unwrap_or(0.75)
            .clamp(0.0, 1.0);

        // Use Tier 1's time/rrule/body as the ground truth; optionally use the LLM's
        // cleaned body if it provided one and Tier 1 didn't already strip anything.
        let body = if let Some(llm_body) = classification.body.filter(|b| !b.trim().is_empty()) {
            if fallback.body == text {
                // Tier 1 didn't clean the body — accept LLM's version
                llm_body
            } else {
                // Tier 1 already stripped time phrases — keep its result
                fallback.body.clone()
            }
        } else {
            fallback.body.clone()
        };

        tracing::info!(
            "Tier 3: classified {:?} as {:?} (conf={:.2})",
            text, note_type, confidence
        );

        Ok(ClassificationResult {
            note_type,
            confidence,
            time: fallback.time,
            rrule: fallback.rrule.clone(),
            body,
        })
    }
}

// Some backends (e.g. FastFlowLM) ignore `"format": "json"` and may wrap the
// JSON in prose. Find the first `{...}` span and parse that.
fn extract_json<T: serde::de::DeserializeOwned>(s: &str) -> Option<T> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end < start { return None; }
    serde_json::from_str(&s[start..=end]).ok()
}
