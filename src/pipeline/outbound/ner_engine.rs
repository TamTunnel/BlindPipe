use anyhow::Result;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;
use std::sync::Mutex;
use tokenizers::Tokenizer;

#[derive(Debug)]
pub struct NerEntity {
    pub label: String,
    pub text: String,
    pub start: usize,
    pub end: usize,
}

pub struct NerEngine {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
    id2label: Vec<String>,
    #[allow(dead_code)]
    threshold: f32,
}

impl NerEngine {
    pub fn new(model_dir: &str, threshold: f32) -> Result<Self> {
        let model_path = Path::new(model_dir).join("model.onnx");
        let tokenizer_path = Path::new(model_dir).join("tokenizer.json");

        if !model_path.exists() || !tokenizer_path.exists() {
            anyhow::bail!("Model or tokenizer not found in {}", model_dir);
        }

        let _ = ort::init().with_name("promptveil").commit();

        let session = Session::builder()
            .map_err(|e| anyhow::anyhow!("Session builder error: {:?}", e))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow::anyhow!("Optimization error: {:?}", e))?
            .with_intra_threads(1)
            .map_err(|e| anyhow::anyhow!("Thread config error: {:?}", e))?
            .commit_from_file(model_path)
            .map_err(|e| anyhow::anyhow!("Model load error: {:?}", e))?;

        let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| anyhow::anyhow!(e))?;

        // Typical BIO labels for NER models (e.g., dslim/bert-base-NER)
        let id2label = vec![
            "O".to_string(),
            "B-MISC".to_string(),
            "I-MISC".to_string(),
            "B-PER".to_string(),
            "I-PER".to_string(),
            "B-ORG".to_string(),
            "I-ORG".to_string(),
            "B-LOC".to_string(),
            "I-LOC".to_string(),
        ];

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
            id2label,
            threshold,
        })
    }

    pub fn extract(&self, text: &str) -> Result<Vec<NerEntity>> {
        if text.trim().is_empty() {
            return Ok(vec![]);
        }

        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!(e))?;

        let seq_len = encoding.get_ids().len();
        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let attention_mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&x| x as i64)
            .collect();
        let token_type_ids: Vec<i64> = vec![0i64; seq_len];

        let input_ids_val = Tensor::from_array(([1, seq_len], input_ids))
            .map_err(|e| anyhow::anyhow!("{:?}", e))?;
        let attention_mask_val = Tensor::from_array(([1, seq_len], attention_mask))
            .map_err(|e| anyhow::anyhow!("{:?}", e))?;
        let token_type_ids_val = Tensor::from_array(([1, seq_len], token_type_ids))
            .map_err(|e| anyhow::anyhow!("{:?}", e))?;

        let inputs = ort::inputs![
            "input_ids" => input_ids_val,
            "attention_mask" => attention_mask_val,
            "token_type_ids" => token_type_ids_val,
        ];

        let mut session_guard = self.session.lock().map_err(|e| anyhow::anyhow!("Mutex poison error: {:?}", e))?;
        let outputs = session_guard.run(inputs).map_err(|e| anyhow::anyhow!("{:?}", e))?;
        let (dims, logits_slice) = outputs["logits"].try_extract_tensor::<f32>()
            .map_err(|e| anyhow::anyhow!("{:?}", e))?;
        let num_classes = if dims.len() == 3 { dims[2] as usize } else { self.id2label.len() };

        let mut entities = Vec::new();
        let mut current_entity: Option<(String, usize, usize)> = None;

        let offsets = encoding.get_offsets();

        for i in 0..seq_len {
            if offsets[i] == (0, 0) {
                continue; // Skip special tokens like [CLS], [SEP]
            }

            let mut max_val = f32::NEG_INFINITY;
            let mut max_idx = 0;
            for c in 0..num_classes.min(self.id2label.len()) {
                let idx = i * num_classes + c;
                if idx < logits_slice.len() {
                    let val = logits_slice[idx];
                    if val > max_val {
                        max_val = val;
                        max_idx = c;
                    }
                }
            }

            let label = &self.id2label[max_idx];
            let (start_offset, end_offset) = offsets[i];

            if label.starts_with("B-") {
                if let Some((lbl, start, end)) = current_entity.take() {
                    entities.push(NerEntity {
                        label: lbl,
                        text: text[start..end].to_string(),
                        start,
                        end,
                    });
                }
                current_entity = Some((label[2..].to_string(), start_offset, end_offset));
            } else if label.starts_with("I-") {
                if let Some((lbl, start, _end)) = current_entity.take() {
                    if lbl == label[2..] {
                        current_entity = Some((lbl, start, end_offset));
                    } else {
                        current_entity = Some((label[2..].to_string(), start_offset, end_offset));
                    }
                }
            } else if let Some((lbl, start, end)) = current_entity.take() {
                entities.push(NerEntity {
                    label: lbl,
                    text: text[start..end].to_string(),
                    start,
                    end,
                });
            }
        }

        if let Some((lbl, start, end)) = current_entity.take() {
            entities.push(NerEntity {
                label: lbl,
                text: text[start..end].to_string(),
                start,
                end,
            });
        }

        for ent in &mut entities {
            ent.label = ent.label.to_uppercase().replace(' ', "_");
        }

        Ok(entities)
    }
}
