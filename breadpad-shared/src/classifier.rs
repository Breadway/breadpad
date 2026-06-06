use crate::ai::OllamaClient;
use crate::config::OllamaConfig;
use crate::parser::parse_rule_based;
use crate::types::{ClassificationResult, NoteType};
use std::path::PathBuf;

/// Minimum Tier 1 confidence needed to skip Tier 2 entirely.
const TIER1_SKIP_THRESHOLD: f32 = 0.82;

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionProvider {
    Gpu,
    Cpu,
}

impl ExecutionProvider {
    pub fn as_str(&self) -> &str {
        match self {
            ExecutionProvider::Gpu => "ROCm (iGPU)",
            ExecutionProvider::Cpu => "CPU",
        }
    }
}

pub struct Classifier {
    session: Option<ort::session::Session>,
    tokenizer: Option<tokenizers::Tokenizer>,
    pub active_provider: ExecutionProvider,
    pub model_path: PathBuf,
    pub default_morning: String,
    ollama: Option<OllamaConfig>,
}

fn model_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("breadpad")
        .join("model")
}

impl Classifier {
    /// Load with Tier 1 + optional Tier 2 (ONNX). Tier 3 disabled unless
    /// `.with_ollama()` is called on the returned value.
    pub fn load(default_morning: &str) -> Self {
        let dir = model_dir();
        let onnx_path = dir.join("classifier.onnx");
        let tok_path = dir.join("tokenizer.json");
        Self::load_with_paths(default_morning, onnx_path, tok_path)
    }

    pub fn load_with_paths(
        default_morning: &str,
        model_path: PathBuf,
        tokenizer_path: PathBuf,
    ) -> Self {
        let (session, active_provider) = if model_path.exists() {
            try_load_session(&model_path)
        } else {
            tracing::warn!("model not found at {:?}; Tier 2 disabled", model_path);
            (None, ExecutionProvider::Cpu)
        };

        let tokenizer = if tokenizer_path.exists() && session.is_some() {
            match tokenizers::Tokenizer::from_file(&tokenizer_path) {
                Ok(tok) => Some(tok),
                Err(e) => {
                    tracing::warn!("failed to load tokenizer: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Classifier {
            session,
            tokenizer,
            active_provider,
            model_path,
            default_morning: default_morning.to_string(),
            ollama: None,
        }
    }

    /// Enable Tier 3 (Ollama). Only has an effect if `cfg.enabled` is true.
    pub fn with_ollama(mut self, cfg: OllamaConfig) -> Self {
        self.ollama = if cfg.enabled { Some(cfg) } else { None };
        self
    }

    /// Three-tier classification pipeline:
    ///
    /// - **Tier 1** (rule-based parser): always runs; handles time/recurrence extraction
    ///   and obvious type signals. If confidence ≥ 0.82, result is returned immediately.
    /// - **Tier 2** (small ONNX model): runs when Tier 1 is uncertain about the type.
    ///   Responsible for type classification only; Tier 1's time/rrule/body are preserved.
    /// - **Tier 3** (Ollama LLM): runs when Tier 2 confidence is below the configured
    ///   threshold. Falls back to the Tier 2 result if Ollama is unreachable.
    pub fn classify(&mut self, text: &str) -> ClassificationResult {
        // ── Tier 1 ───────────────────────────────────────────────────────────
        let tier1 = parse_rule_based(text, &self.default_morning);
        tracing::debug!("Tier 1: {:?} conf={:.2}", tier1.note_type, tier1.confidence);

        if tier1.confidence >= TIER1_SKIP_THRESHOLD {
            return tier1;
        }

        // ── Tier 2 ───────────────────────────────────────────────────────────
        // ONNX model classifies the type only; Tier 1's time/rrule/body are kept.
        let tier2 = if let (Some(session), Some(tokenizer)) =
            (&mut self.session, &self.tokenizer)
        {
            match run_onnx(session, tokenizer, text) {
                Ok(r) => {
                    tracing::debug!("Tier 2: {:?} conf={:.2}", r.note_type, r.confidence);
                    ClassificationResult {
                        note_type: r.note_type,
                        confidence: r.confidence,
                        time: tier1.time,
                        rrule: tier1.rrule.clone(),
                        body: tier1.body.clone(),
                    }
                }
                Err(e) => {
                    tracing::warn!("Tier 2 inference failed: {}; using Tier 1 result", e);
                    tier1.clone()
                }
            }
        } else {
            tier1.clone()
        };

        // ── Tier 3 ───────────────────────────────────────────────────────────
        if let Some(ollama_cfg) = &self.ollama {
            if tier2.confidence < ollama_cfg.confidence_threshold {
                tracing::debug!(
                    "Tier 2 confidence {:.2} < threshold {:.2}; trying Tier 3",
                    tier2.confidence,
                    ollama_cfg.confidence_threshold
                );
                let client = OllamaClient::new(ollama_cfg);
                return client.classify(text, &tier2);
            }
        }

        tier2
    }

    pub fn model_available(&self) -> bool {
        self.session.is_some()
    }

    /// Run only the ONNX model (Tier 2) with no Tier 1 pre-processing or fallback.
    /// Returns `None` if no model is loaded.
    pub fn classify_tier2_only(&mut self, text: &str) -> Option<ClassificationResult> {
        let (session, tokenizer) = (self.session.as_mut()?, self.tokenizer.as_ref()?);
        run_onnx(session, tokenizer, text).ok()
    }
}

// NLI hypotheses paired with their note types. The model scores each as
// entailment (label 0) vs not_entailment (label 1); we pick the highest
// entailment score across all five passes.
const HYPOTHESES: [(&str, &str); 5] = [
    ("This note is a task or action item to complete.", "todo"),
    ("This note is a reminder with a specific time or deadline.", "reminder"),
    ("This note is an idea, suggestion, or creative thought.", "idea"),
    ("This note is a general observation or piece of information.", "note"),
    ("This note is a question that needs an answer.", "question"),
];

fn run_onnx(
    session: &mut ort::session::Session,
    tokenizer: &tokenizers::Tokenizer,
    text: &str,
) -> anyhow::Result<ClassificationResult> {
    const ENTAILMENT_IDX: usize = 0;

    let mut entailment_scores = [0.0f32; 5];

    for (i, (hypothesis, _)) in HYPOTHESES.iter().enumerate() {
        let encoding = tokenizer
            .encode((text, *hypothesis), true)
            .map_err(|e| anyhow::anyhow!("tokenize: {}", e))?;

        let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&x| x as i64).collect();
        let len = ids.len();

        let ids_tensor = ort::value::Tensor::<i64>::from_array(
            (vec![1i64, len as i64], ids)
        ).map_err(|e| anyhow::anyhow!("ids tensor: {}", e))?;
        let mask_tensor = ort::value::Tensor::<i64>::from_array(
            (vec![1i64, len as i64], mask)
        ).map_err(|e| anyhow::anyhow!("mask tensor: {}", e))?;

        let inputs = ort::inputs![
            "input_ids" => ids_tensor,
            "attention_mask" => mask_tensor,
        ];
        let outputs = session
            .run(inputs)
            .map_err(|e| anyhow::anyhow!("run: {}", e))?;

        let logits = outputs["logits"]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow::anyhow!("extract logits: {}", e))?;
        let (_, logits_slice) = logits;

        entailment_scores[i] = logits_slice
            .get(ENTAILMENT_IDX)
            .copied()
            .unwrap_or(0.0);
    }

    let best_idx = entailment_scores
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Less))
        .map(|(i, _)| i)
        .unwrap_or(3);

    let note_type = NoteType::from_str(HYPOTHESES[best_idx].1);
    let confidence = softmax_single(&entailment_scores, best_idx);

    Ok(ClassificationResult {
        note_type,
        confidence,
        // Time/rrule/body are merged by the caller from Tier 1's result.
        time: None,
        rrule: None,
        body: text.to_string(),
    })
}

fn softmax_single(logits: &[f32], idx: usize) -> f32 {
    if logits.is_empty() || idx >= logits.len() {
        return 0.5;
    }
    let max = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f32> = logits.iter().map(|&x| (x - max).exp()).collect();
    let sum: f32 = exps.iter().sum();
    exps[idx] / sum
}

fn try_load_session(
    path: &std::path::Path,
) -> (Option<ort::session::Session>, ExecutionProvider) {
    // Try ROCm (iGPU) first, fall back to CPU.
    match build_onnx_session(path, ort::ep::ROCm::default().build()) {
        Ok(s) => {
            tracing::info!("ONNX session loaded (ROCm iGPU)");
            return (Some(s), ExecutionProvider::Gpu);
        }
        Err(e) => tracing::debug!("ROCm EP unavailable: {}; trying CPU", e),
    }
    match build_onnx_session(path, ort::ep::CPU::default().build()) {
        Ok(s) => {
            tracing::info!("ONNX session loaded (CPU)");
            (Some(s), ExecutionProvider::Cpu)
        }
        Err(e) => {
            tracing::warn!("failed to load ONNX session: {}; Tier 2 disabled", e);
            (None, ExecutionProvider::Cpu)
        }
    }
}

fn build_onnx_session(
    path: &std::path::Path,
    ep: ort::ep::ExecutionProviderDispatch,
) -> anyhow::Result<ort::session::Session> {
    let mut builder = ort::session::Session::builder()
        .map_err(|e| anyhow::anyhow!("builder: {}", e))?
        .with_execution_providers([ep])
        .map_err(|e| anyhow::anyhow!("ep: {}", e))?;
    builder.commit_from_file(path).map_err(|e| anyhow::anyhow!("load: {}", e))
}
